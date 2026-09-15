use std::env;

use arc_core::agent::agent_spec;
use arc_core::detect::DetectCache;
use arc_core::error::ArcError;
use arc_core::paths::{ArcPaths, expand_user_path};
use arc_core::project::skills::{
    ProjectSkillOptions, reconcile_project_skills, recorded_project_agents,
};
use arc_core::project::{
    ProjectMarketEvent, ProjectProviderSwitch, execute_project_apply_with_options,
    find_project_config, prepare_project_apply_with_mode,
};
use arc_core::skill::install::{InstallReport, InstallScope};
use arc_tui::select_agents;
use serde::Serialize;

use crate::cli::{OutputFormat, ProjectApplyArgs, ProjectCleanArgs};
use crate::commands::common::{is_interactive, render_install_report, render_install_report_text};
use crate::format::{SCHEMA_VERSION, print_json};

#[derive(Serialize)]
struct ProjectOutput<'a> {
    schema_version: &'static str,
    ok: bool,
    message: &'static str,
    has_changes: bool,
    #[serde(flatten)]
    installs: &'a InstallReport,
    market_events: &'a [ProjectMarketEvent],
    provider_switch: Option<&'a ProjectProviderSwitch>,
    planned_provider: Option<&'a str>,
}

pub fn run(
    paths: &ArcPaths,
    cache: &DetectCache,
    fmt: &OutputFormat,
    args: &ProjectApplyArgs,
) -> Result<(), ArcError> {
    let cwd = env::current_dir().map_err(|err| ArcError::new(err.to_string()))?;
    let plan = match prepare_project_apply_with_mode(paths, cache, &cwd, args.dry_run) {
        Ok(plan) => plan,
        Err(err) => return fail(fmt, args.dry_run, err),
    };
    let mut options = ProjectSkillOptions {
        agents: args.agent.clone(),
        all_agents: args.all_agents,
        dry_run: args.dry_run,
        adopt_existing: args.adopt_existing,
    };
    if options.agents.is_empty()
        && !options.all_agents
        && !plan.effective.required_skills.is_empty()
    {
        let recorded =
            match recorded_project_agents(paths, plan.effective.project_root.as_deref().unwrap()) {
                Ok(recorded) => recorded,
                Err(err) => return fail(fmt, args.dry_run, err),
            };
        if recorded.is_empty() {
            if !is_interactive(fmt) || args.dry_run {
                return fail(
                    fmt,
                    args.dry_run,
                    ArcError::new(
                        "Choose target agent(s): pass --agent <name> (repeatable) or --all-agents.",
                    ),
                );
            }
            let candidates: Vec<String> = cache
                .agents_for_project_skill_install(&arc_core::models::ResourceKind::Skill)
                .into_iter()
                .filter(|agent| {
                    cache.get_agent(agent).is_some()
                        && agent_spec(agent).is_some_and(|spec| spec.supports_project_skills)
                })
                .collect();
            options.agents = select_agents(&candidates, &[])
                .map_err(|err| ArcError::new(format!("agent selection failed: {err}")))?;
            if options.agents.is_empty() {
                return Err(ArcError::new("No agents selected; canceled."));
            }
        }
    }
    let execution = match execute_project_apply_with_options(paths, cache, &plan, &options) {
        Ok(execution) => execution,
        Err(err) => return fail(fmt, args.dry_run, err),
    };
    let ok = execution.installs.ok();
    if *fmt == OutputFormat::Json {
        print_json(&ProjectOutput {
            schema_version: SCHEMA_VERSION,
            ok,
            message: if ok {
                if args.dry_run { "Plan ready." } else { "Done." }
            } else {
                "Completed with issues."
            },
            has_changes: execution.installs.has_changes()
                || !plan.market_events.is_empty()
                || plan.provider_to_switch.is_some(),
            installs: &execution.installs,
            market_events: &plan.market_events,
            provider_switch: execution.provider_switch.as_ref(),
            planned_provider: if args.dry_run {
                plan.provider_to_switch.as_deref()
            } else {
                None
            },
        })?;
    } else {
        println!(
            "Project: {}",
            plan.effective.project_root.as_ref().unwrap().display()
        );
        for market in &plan.market_events {
            println!("  market {:?}: {}", market.status, market.url);
        }
        if let Some(provider) = &execution.provider_switch {
            println!("  provider switched: {}", provider.name);
        }
        if args.dry_run
            && let Some(provider) = &plan.provider_to_switch
        {
            println!("  provider planned: {provider}");
        }
        render_install_report_text(&execution.installs);
        println!(
            "{}",
            if ok {
                if args.dry_run {
                    "Plan ready."
                } else {
                    "Ready."
                }
            } else {
                "Completed with issues."
            }
        );
    }
    if ok {
        Ok(())
    } else {
        Err(ArcError::new("Project apply completed with issues."))
    }
}

pub fn clean(
    paths: &ArcPaths,
    cache: &DetectCache,
    fmt: &OutputFormat,
    args: &ProjectCleanArgs,
) -> Result<(), ArcError> {
    let cwd = env::current_dir().map_err(|err| ArcError::new(err.to_string()))?;
    let root = if let Some(root) = &args.project_root {
        expand_user_path(root)
    } else {
        match find_project_config(&cwd)
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        {
            Some(root) => root,
            None => {
                return fail(
                    fmt,
                    args.dry_run,
                    ArcError::new(
                        "No arc.toml found; pass --project-root <path> to clean a recorded project.",
                    ),
                );
            }
        }
    };
    let options = ProjectSkillOptions {
        agents: args.agent.clone(),
        all_agents: args.all_agents,
        dry_run: args.dry_run,
        adopt_existing: false,
    };
    let report = reconcile_project_skills(paths, cache, &root, &[], &options, true, &[]);
    render_install_report(&report, fmt)
}

fn fail(fmt: &OutputFormat, dry_run: bool, error: ArcError) -> Result<(), ArcError> {
    let mut report = InstallReport::new(InstallScope::Project, None, dry_run);
    report.errors.push(error.message.clone());
    if *fmt == OutputFormat::Json {
        print_json(&crate::commands::common::InstallCommandOutput {
            schema_version: SCHEMA_VERSION,
            ok: false,
            message: &error.message,
            has_changes: false,
            report: &report,
        })?;
    }
    Err(error)
}
