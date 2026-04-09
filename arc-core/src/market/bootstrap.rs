use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::git::{GitRepo, clone};
use crate::market::catalog::CatalogManager;
use crate::market::index::MarketIndexStore;
use crate::market::scanner::scan_repo;
use crate::market::sources::{MarketSourcePatch, MarketSourceRegistry};
use crate::models::MarketSource;
use crate::paths::ArcPaths;
use crate::skill::GlobalSkillMaintenanceReport;

/// Clone checkout if missing, scan resources, update registry metadata, merge into catalog.
/// Used by `arc market add` and `arc project apply` after registering a market so the source
/// is not left at `0 resources` / `never` until a later `arc market update`.
pub fn sync_market_source_resources(paths: &ArcPaths, source: &MarketSource) -> Result<usize> {
    paths
        .ensure_arc_home()
        .map_err(|err| ArcError::new(err.to_string()))?;
    let repo_dir = paths.market_checkout(source);
    if !repo_dir.exists() {
        clone(&source.git_url, &repo_dir, None)?;
    }
    let resources = scan_repo(&repo_dir, &source.parser, Some(&source.id));
    let registry = MarketSourceRegistry::new(paths.clone());
    registry
        .update_source(
            &source.id,
            MarketSourcePatch {
                status: Some("ok".to_string()),
                resource_count: Some(resources.len()),
                last_updated_at: Some(crate::io::now_unix_secs()),
                ..Default::default()
            },
        )
        .map_err(|err| ArcError::new(err.to_string()))?;
    CatalogManager::new(paths.clone())
        .rebuild_source(&source.id, &resources)
        .map_err(|err| ArcError::new(err.to_string()))?;
    Ok(resources.len())
}

#[derive(Debug, Clone, Default)]
pub struct MarketSyncReport {
    pub refreshed_index: bool,
    pub refresh_warning: Option<String>,
    pub source_count: usize,
    pub cloned_count: usize,
    pub pulled_count: usize,
    pub resource_count: usize,
    pub sources: Vec<SourceSyncDetail>,
    /// Present after catalog rebuild: prune unknown names, then refresh installs from registry.
    pub global_skills: Option<GlobalSkillMaintenanceReport>,
}

#[derive(Debug, Clone)]
pub struct SourceSyncDetail {
    pub source_id: String,
    pub resource_count: usize,
    pub cloned: bool,
}

pub fn ensure_local_catalog(paths: &ArcPaths) -> Result<MarketSyncReport> {
    let catalog_empty = CatalogManager::new(paths.clone())
        .load()
        .resources
        .is_empty();
    let registry = MarketSourceRegistry::new(paths.clone());
    let sources = registry.list_all();
    // New markets (e.g. from `arc project apply` or `market add`) have empty
    // `last_updated_at` until the first successful sync; we must not skip sync
    // just because the catalog already contains entries from other sources.
    let pending_source_sync = sources
        .iter()
        .any(|source| source.last_updated_at.is_empty());

    if !catalog_empty && !pending_source_sync {
        return Ok(MarketSyncReport::default());
    }
    let index_store = MarketIndexStore::new(paths.clone());
    let refreshed_index = if index_store.load_cached().is_err() {
        index_store.refresh().is_ok()
    } else {
        false
    };
    let mut report = sync_market_sources(paths, false)?;
    report.refreshed_index = refreshed_index;
    Ok(report)
}

pub fn refresh_and_sync_market_sources(paths: &ArcPaths) -> Result<MarketSyncReport> {
    sync_market_sources(paths, true)
}

fn sync_market_sources(paths: &ArcPaths, refresh_index: bool) -> Result<MarketSyncReport> {
    paths
        .ensure_arc_home()
        .map_err(|err| ArcError::new(err.to_string()))?;

    let mut report = MarketSyncReport::default();
    if refresh_index {
        match MarketIndexStore::new(paths.clone()).refresh() {
            Ok(_) => {
                report.refreshed_index = true;
            }
            Err(err) => {
                report.refresh_warning = Some(err.message);
            }
        }
    }

    let registry = MarketSourceRegistry::new(paths.clone());
    let sources = registry.list_all();
    report.source_count = sources.len();
    if sources.is_empty() {
        CatalogManager::new(paths.clone())
            .rebuild(&[])
            .map_err(|err| ArcError::new(format!("failed to rebuild catalog: {err}")))?;
        return Ok(report);
    }

    struct SourceResult {
        source_id: String,
        resources: Vec<crate::models::ResourceInfo>,
        cloned: bool,
    }

    let results: Vec<Result<SourceResult>> = std::thread::scope(|s| {
        let handles: Vec<_> = sources
            .iter()
            .map(|source| {
                let repo_dir = paths.market_checkout(source);
                let git_url = &source.git_url;
                let parser = &source.parser;
                let source_id = &source.id;
                s.spawn(move || {
                    let cloned = if repo_dir.exists() {
                        if refresh_index {
                            GitRepo::new(&repo_dir).pull_default_branch("origin")?;
                        }
                        false
                    } else {
                        clone(git_url, &repo_dir, None)?;
                        true
                    };
                    let resources = scan_repo(&repo_dir, parser, Some(source_id));
                    Ok(SourceResult {
                        source_id: source_id.clone(),
                        resources,
                        cloned,
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("market sync panicked"))
            .collect()
    });

    let mut all_resources = Vec::new();
    for result in results {
        let sr = result?;
        if sr.cloned {
            report.cloned_count += 1;
        } else if refresh_index {
            report.pulled_count += 1;
        }
        report.sources.push(SourceSyncDetail {
            source_id: sr.source_id.clone(),
            resource_count: sr.resources.len(),
            cloned: sr.cloned,
        });
        registry
            .update_source(
                &sr.source_id,
                MarketSourcePatch {
                    status: Some("ok".to_string()),
                    resource_count: Some(sr.resources.len()),
                    last_updated_at: Some(crate::io::now_unix_secs()),
                    ..Default::default()
                },
            )
            .map_err(|err| ArcError::new(format!("failed to update source metadata: {err}")))?;
        all_resources.extend(sr.resources);
    }

    report.resource_count = all_resources.len();
    CatalogManager::new(paths.clone())
        .rebuild(&all_resources)
        .map_err(|err| ArcError::new(format!("failed to rebuild catalog: {err}")))?;
    if refresh_index {
        let cache = DetectCache::new(paths);
        report.global_skills = Some(crate::skill::run_global_skill_maintenance(paths, &cache)?);
    }
    Ok(report)
}
