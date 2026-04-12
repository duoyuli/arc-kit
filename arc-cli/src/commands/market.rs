use std::time::{SystemTime, UNIX_EPOCH};

use arc_core::ArcPaths;
use arc_core::error::ArcError;
use arc_core::git::validate_git_url;
use arc_core::market::bootstrap::sync_market_source_resources;
use arc_core::market::catalog::CatalogManager;
use arc_core::market::sources::MarketSourceRegistry;
use console::style;

use crate::cli::{MarketCommand, OutputFormat};
use crate::format::{
    MarketItem, MarketListOutput, SCHEMA_VERSION, WriteResult, WriteResultItem, print_json,
};

pub fn run(paths: &ArcPaths, command: MarketCommand, fmt: &OutputFormat) -> Result<(), ArcError> {
    match command {
        MarketCommand::Add { git_url } => add(paths, &git_url, fmt),
        MarketCommand::List => list(paths, fmt),
        MarketCommand::Remove { git_url } => remove(paths, &git_url, fmt),
        MarketCommand::Update => update(paths, fmt),
    }
}

fn add(paths: &ArcPaths, git_url: &str, fmt: &OutputFormat) -> Result<(), ArcError> {
    if !validate_git_url(git_url) {
        return Err(ArcError::with_hint(
            format!("Invalid Git URL: {git_url}"),
            "URL must start with https://, git://, ssh://, git@, or file://",
        ));
    }
    paths
        .ensure_arc_home()
        .map_err(|err| ArcError::new(err.to_string()))?;
    let registry = MarketSourceRegistry::new(paths.clone());
    if registry
        .list_all()
        .iter()
        .any(|source| source.git_url == git_url)
    {
        if *fmt == OutputFormat::Json {
            print_json(&WriteResult {
                schema_version: SCHEMA_VERSION,
                ok: true,
                message: format!("Market source already exists: {git_url}"),
                items: Vec::new(),
            })?;
            return Ok(());
        }
        println!(
            "  {} {}",
            style("Market source already exists:").yellow(),
            git_url
        );
        return Ok(());
    }

    let source = registry
        .add(git_url, "auto")
        .map_err(|err| ArcError::new(format!("failed to register market source: {err}")))?;

    let resource_count = sync_market_source_resources(paths, &source).map_err(|err| {
        ArcError::new(format!(
            "failed to sync market source '{}': {}",
            source.id, err.message
        ))
    })?;

    if *fmt == OutputFormat::Json {
        print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: format!(
                "Added market source '{}' with {} resources.",
                source.id, resource_count
            ),
            items: Vec::new(),
        })?;
        return Ok(());
    }

    println!(
        "  {} {}  {}",
        style("✓").green(),
        style(&source.id).bold(),
        style(format!("{} resources", resource_count)).dim()
    );
    Ok(())
}

fn list(paths: &ArcPaths, fmt: &OutputFormat) -> Result<(), ArcError> {
    let registry = MarketSourceRegistry::new(paths.clone());
    let sources = registry.list_all();

    if *fmt == OutputFormat::Json {
        let items: Vec<MarketItem> = sources
            .iter()
            .map(|s| MarketItem {
                id: s.id.clone(),
                git_url: s.git_url.clone(),
                status: s.status.clone(),
                resource_count: s.resource_count,
                last_updated_at: s.last_updated_at.clone(),
            })
            .collect();
        print_json(&MarketListOutput {
            schema_version: SCHEMA_VERSION,
            markets: items,
        })?;
        return Ok(());
    }

    if sources.is_empty() {
        println!("  {}", style("No market sources added.").yellow());
        return Ok(());
    }

    let id_width = sources.iter().map(|s| s.id.len()).max().unwrap_or(0);
    let res_width = sources
        .iter()
        .map(|s| format!("{}", s.resource_count).len())
        .max()
        .unwrap_or(1);

    println!();
    for source in &sources {
        println!(
            "  {:<id_w$}  {:>res_w$} resources   {}",
            style(&source.id).bold(),
            source.resource_count,
            style(format_relative_time(&source.last_updated_at)).dim(),
            id_w = id_width,
            res_w = res_width,
        );
    }
    println!();
    Ok(())
}

fn update(paths: &ArcPaths, fmt: &OutputFormat) -> Result<(), ArcError> {
    use arc_core::market::bootstrap::refresh_and_sync_market_sources;

    let report = refresh_and_sync_market_sources(paths)?;

    if *fmt == OutputFormat::Json {
        let mut items: Vec<WriteResultItem> = Vec::new();
        if let Some(gs) = &report.global_skills {
            for f in &gs.sync.failures {
                items.push(WriteResultItem {
                    resource_kind: None,
                    name: f.skill.clone(),
                    agent: f.agent.clone().unwrap_or_default(),
                    status: format!("failed: {}", f.message),
                    desired_scope: None,
                    applied_scope: None,
                    reason: None,
                });
            }
        }
        let sync_ok = report
            .global_skills
            .as_ref()
            .map(|g| g.sync.failures.is_empty())
            .unwrap_or(true);
        let gs_msg = report.global_skills.as_ref().map(|g| {
            format!(
                " Removed {} stale install(s), refreshed {} skill target(s).",
                g.cleanup.removed, g.sync.refreshed
            )
        });
        print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: sync_ok,
            message: format!(
                "Updated {} sources, {} resources.{}",
                report.source_count,
                report.resource_count,
                gs_msg.unwrap_or_default()
            ),
            items,
        })?;
        return Ok(());
    }

    if let Some(warning) = &report.refresh_warning {
        println!(
            "  {} {}",
            style("!").yellow(),
            style(format!(
                "built-in manifest unavailable, using cache: {warning}"
            ))
            .dim()
        );
    }

    if report.source_count == 0 {
        println!("  {}", style("No market sources added.").yellow());
    } else {
        let id_width = report
            .sources
            .iter()
            .map(|s| s.source_id.len())
            .max()
            .unwrap_or(0);

        println!();
        for detail in &report.sources {
            println!(
                "  {} {:<id_w$}  {} resources",
                style("✓").green(),
                detail.source_id,
                detail.resource_count,
                id_w = id_width,
            );
        }

        println!();
        println!(
            "  {}",
            style(format!(
                "{} sources · {} resources",
                report.source_count, report.resource_count
            ))
            .dim()
        );
    }

    if let Some(gs) = &report.global_skills {
        println!();
        println!(
            "  {}",
            style(format!(
                "Global skills: {} stale install(s) removed · {} target(s) refreshed",
                gs.cleanup.removed, gs.sync.refreshed
            ))
            .dim()
        );
        if !gs.sync.failures.is_empty() {
            for f in &gs.sync.failures {
                eprintln!("  {} {} {}", style("!").yellow(), f.skill, f.message);
            }
        }
    }
    println!();
    Ok(())
}

fn remove(paths: &ArcPaths, target: &str, fmt: &OutputFormat) -> Result<(), ArcError> {
    let registry = MarketSourceRegistry::new(paths.clone());
    let sources = registry.list_all();
    let Some(source) = sources
        .into_iter()
        .find(|source| source.git_url == target || source.id == target)
    else {
        return Err(ArcError::new(format!("Market source not found: {target}")));
    };
    if registry.is_builtin(&source.id) {
        return Err(ArcError::with_hint(
            format!("Built-in market source cannot be removed: {}", source.id),
            "Built-in market sources come from built-in/market/index.toml",
        ));
    }

    registry
        .remove(&source.id)
        .map_err(|err| ArcError::new(format!("failed to remove source: {err}")))?;
    let repo_dir = paths.market_checkout(&source);
    if repo_dir.exists() {
        std::fs::remove_dir_all(&repo_dir)
            .map_err(|err| ArcError::new(format!("failed to remove market checkout: {err}")))?;
    }
    CatalogManager::new(paths.clone())
        .remove_source_resources(&source.id)
        .map_err(|err| ArcError::new(format!("failed to prune catalog resources: {err}")))?;

    if *fmt == OutputFormat::Json {
        print_json(&WriteResult {
            schema_version: SCHEMA_VERSION,
            ok: true,
            message: format!("Removed market source: {}", source.id),
            items: Vec::new(),
        })?;
        return Ok(());
    }

    println!(
        "  {} Removed market source: {}",
        style("✓").green(),
        source.id
    );
    println!(
        "  {}",
        style("Note: Installed resources are not affected.").dim()
    );
    Ok(())
}

fn format_relative_time(timestamp_str: &str) -> String {
    let Ok(ts) = timestamp_str.parse::<u64>() else {
        return "never".to_string();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if ts == 0 || ts > now {
        return "never".to_string();
    }
    let diff = now - ts;
    match diff {
        0..60 => "just now".to_string(),
        60..3600 => {
            let m = diff / 60;
            format!("{m} min ago")
        }
        3600..86400 => {
            let h = diff / 3600;
            if h == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{h} hours ago")
            }
        }
        86400..2592000 => {
            let d = diff / 86400;
            if d == 1 {
                "1 day ago".to_string()
            } else {
                format!("{d} days ago")
            }
        }
        _ => {
            let months = diff / 2592000;
            if months == 1 {
                "1 month ago".to_string()
            } else {
                format!("{months} months ago")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_core::models::MarketSource;

    fn format_market_source_line(source: &MarketSource) -> String {
        let updated = if source.last_updated_at.is_empty() {
            "-"
        } else {
            &source.last_updated_at
        };
        format!(
            "- {} [{}] resources={} updated={} {}",
            source.id, source.status, source.resource_count, updated, source.git_url
        )
    }

    #[test]
    fn formats_market_source_on_single_line() {
        let source = MarketSource {
            id: "jimliu-baoyu-skills".to_string(),
            git_url: "https://github.com/jimliu/baoyu-skills".to_string(),
            parser: "auto".to_string(),
            owner: "jimliu".to_string(),
            repo: "baoyu-skills".to_string(),
            status: "ok".to_string(),
            last_updated_at: "1774501978".to_string(),
            resource_count: 20,
        };

        assert_eq!(
            format_market_source_line(&source),
            "- jimliu-baoyu-skills [ok] resources=20 updated=1774501978 https://github.com/jimliu/baoyu-skills"
        );
    }

    #[test]
    fn relative_time_just_now() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time(&now.to_string()), "just now");
    }

    #[test]
    fn relative_time_minutes() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 300;
        assert_eq!(format_relative_time(&ts.to_string()), "5 min ago");
    }

    #[test]
    fn relative_time_hours() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 7200;
        assert_eq!(format_relative_time(&ts.to_string()), "2 hours ago");
    }

    #[test]
    fn relative_time_invalid() {
        assert_eq!(format_relative_time(""), "never");
        assert_eq!(format_relative_time("not-a-number"), "never");
    }
}
