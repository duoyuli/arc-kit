use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use crate::agent::agent_spec;
use crate::detect::DetectCache;
use crate::error::Result;
use crate::market::bootstrap::{MarketSyncReport, ensure_local_catalog};
use crate::market::catalog::CatalogManager;
use crate::market::scanner::find_skill_directory;
use crate::market::sources::MarketSourceRegistry;
use crate::models::{ResourceKind, SkillEntry, SkillOrigin};
use crate::paths::ArcPaths;
use crate::skill::builtin;
use crate::skill::local;
use crate::skill::merge;
use crate::skill::tracking::is_arc_tracking_file_name;

pub struct SkillRegistry {
    paths: ArcPaths,
    cache: DetectCache,
}

impl SkillRegistry {
    pub fn new(paths: ArcPaths, cache: DetectCache) -> Self {
        Self { paths, cache }
    }

    pub(crate) fn arc_paths(&self) -> &ArcPaths {
        &self.paths
    }

    pub(crate) fn detect_cache(&self) -> &DetectCache {
        &self.cache
    }

    /// Merge all three sources, deduplicate by name using priority (local > built-in > market).
    pub fn list_all(&self) -> Vec<SkillEntry> {
        let map =
            merge::merge_by_priority(self.scan_market(), self.scan_builtin(), self.scan_local());
        let mut entries: Vec<SkillEntry> = map.into_values().collect();
        self.fill_installed_targets(&mut entries);
        entries
    }

    /// Find a single skill by name, respecting priority.
    pub fn find(&self, name: &str) -> Option<SkillEntry> {
        self.list_all().into_iter().find(|e| e.name == name)
    }

    /// Ensure the source_path is materialized on disk (matters for built-in skills).
    pub fn resolve_source_path(&self, entry: &SkillEntry) -> Result<PathBuf> {
        match &entry.origin {
            SkillOrigin::BuiltIn => {
                let cache_dir = self.paths.builtin_cache_dir();
                builtin::materialize(&cache_dir, &entry.name)
                    .map_err(|err| crate::error::ArcError::new(err.to_string()))
            }
            SkillOrigin::Market { source_id } => {
                self.resolve_market_source_path(source_id, &entry.name)
            }
            SkillOrigin::Local => Ok(entry.source_path.clone()),
        }
    }

    /// Bootstrap catalog if needed (safe to call multiple times).
    /// Returns a sync report so callers can display bootstrap progress.
    pub fn bootstrap_catalog(&self) -> Result<MarketSyncReport> {
        ensure_local_catalog(&self.paths)
    }

    fn scan_market(&self) -> Vec<SkillEntry> {
        let catalog = CatalogManager::new(self.paths.clone());
        let market_registry = MarketSourceRegistry::new(self.paths.clone());
        let market_sources = market_registry.load();
        catalog
            .get_resources(Some(ResourceKind::Skill))
            .into_iter()
            .map(|r| {
                let market_repo = market_sources.get(&r.source_id).and_then(|s| {
                    if s.owner.is_empty() || s.repo.is_empty() {
                        None
                    } else {
                        Some(format!("{}/{}", s.owner, s.repo))
                    }
                });
                SkillEntry {
                    name: r.name,
                    origin: SkillOrigin::Market {
                        source_id: r.source_id,
                    },
                    summary: r.summary,
                    source_path: PathBuf::new(),
                    installed_targets: Vec::new(),
                    market_repo,
                }
            })
            .collect()
    }

    fn scan_builtin(&self) -> Vec<SkillEntry> {
        builtin::list_builtin_skills(&self.paths.builtin_cache_dir())
    }

    fn scan_local(&self) -> Vec<SkillEntry> {
        local::scan_local_skills(&self.paths.local_skills_dir())
    }

    fn fill_installed_targets(&self, entries: &mut [SkillEntry]) {
        let agents = self.cache.detected_agents();
        let mut installed_per_agent: BTreeMap<&str, HashSet<String>> = BTreeMap::new();
        for (agent_id, info) in agents {
            let Some(root) = &info.root else {
                continue;
            };
            let subdir = skill_subdir(agent_id);
            let Ok(rd) = std::fs::read_dir(root.join(subdir)) else {
                continue;
            };
            let names: HashSet<String> = rd
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') || is_arc_tracking_file_name(&name) {
                        return None;
                    }
                    let path = entry.path();
                    if !(path.is_symlink() || path.is_dir()) {
                        return None;
                    }
                    Some(name)
                })
                .collect();
            installed_per_agent.insert(agent_id.as_str(), names);
        }
        for entry in entries.iter_mut() {
            entry.installed_targets = installed_per_agent
                .iter()
                .filter(|(_, names)| names.contains(&entry.name))
                .map(|(id, _)| (*id).to_string())
                .collect();
        }
    }

    fn resolve_market_source_path(&self, source_id: &str, name: &str) -> Result<PathBuf> {
        let registry = MarketSourceRegistry::new(self.paths.clone());
        let source = registry.get(source_id).ok_or_else(|| {
            crate::error::ArcError::new(format!("market source '{source_id}' not found"))
        })?;
        let repo_dir = self.paths.market_checkout(&source);
        if !repo_dir.exists() {
            self.paths
                .ensure_arc_home()
                .map_err(|err| crate::error::ArcError::new(err.to_string()))?;
            crate::git::clone(&source.git_url, &repo_dir, None)?;
        }
        find_skill_directory(&repo_dir, name).ok_or_else(|| {
            crate::error::ArcError::new(format!(
                "skill '{name}' not found in market source '{source_id}'"
            ))
        })
    }
}

pub fn skill_subdir(agent_id: &str) -> &str {
    agent_spec(agent_id)
        .map(|spec| spec.skills_subdir)
        .unwrap_or("skills")
}
