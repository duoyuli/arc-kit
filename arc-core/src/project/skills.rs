//! 仅协调所选项目中归属验证通过的安装落点。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::agent::{SkillInstallStrategy, agent_spec};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::paths::ArcPaths;
use crate::skill::SkillRegistry;
use crate::skill::install::{
    InstallAction, InstallItem, InstallLedger, InstallOperation, InstallRecord, InstallReport,
    InstallScope, InstallStatus, InstallStore, Journal, NodeState, candidate_destinations,
    inspect_owned, normalize_root, validate_destination, validate_name, with_snapshot,
};
use crate::skill::tracking::fingerprint_path;

#[derive(Debug, Clone, Default)]
pub struct ProjectSkillOptions {
    pub agents: Vec<String>,
    pub all_agents: bool,
    pub dry_run: bool,
    pub adopt_existing: bool,
}

struct PlannedInstall {
    item: InstallItem,
    operation: Option<InstallOperation>,
}

fn selected_agents(
    cache: &DetectCache,
    root: &Path,
    options: &ProjectSkillOptions,
    ledger: &InstallLedger,
    journals: &[Journal],
) -> Result<Vec<String>> {
    if options.all_agents && !options.agents.is_empty() {
        return Err(ArcError::new(
            "--agent and --all-agents are mutually exclusive",
        ));
    }
    for agent in &options.agents {
        let recorded = ledger
            .in_scope(InstallScope::Project, Some(root))
            .any(|record| record.agent == *agent)
            || journals.iter().any(|journal| {
                journal.context.scope == InstallScope::Project
                    && journal.context.project_root.as_deref() == Some(root)
                    && journal.context.agent == *agent
            });
        if !recorded && !agent_spec(agent).is_some_and(|spec| spec.supports_project_skills) {
            return Err(ArcError::new(format!(
                "unsupported project agent '{agent}'"
            )));
        }
    }
    let mut agents: BTreeSet<String> = options.agents.iter().cloned().collect();
    if options.agents.is_empty() {
        agents.extend(
            ledger
                .in_scope(InstallScope::Project, Some(root))
                .map(|record| record.agent.clone()),
        );
        agents.extend(
            journals
                .iter()
                .filter(|journal| {
                    journal.context.scope == InstallScope::Project
                        && journal.context.project_root.as_deref() == Some(root)
                })
                .map(|journal| journal.context.agent.clone()),
        );
    }
    if options.all_agents {
        agents.extend(
            cache
                .detected_agents()
                .keys()
                .filter(|agent| agent_spec(agent).is_some_and(|spec| spec.supports_project_skills))
                .cloned(),
        );
    }
    Ok(agents.into_iter().collect())
}

pub fn recorded_project_agents(paths: &ArcPaths, project_root: &Path) -> Result<Vec<String>> {
    let root = normalize_root(project_root)?;
    with_snapshot(paths, |ledger, journals| {
        selected_agents(
            &DetectCache::from_map(BTreeMap::new()),
            &root,
            &ProjectSkillOptions::default(),
            ledger,
            journals,
        )
    })
}

pub fn reconcile_project_skills(
    paths: &ArcPaths,
    cache: &DetectCache,
    project_root: &Path,
    required: &[String],
    options: &ProjectSkillOptions,
    clean: bool,
    preparation_errors: &[String],
) -> InstallReport {
    let mut report = InstallReport::new(
        InstallScope::Project,
        Some(project_root.to_path_buf()),
        options.dry_run,
    );
    let root = match normalize_root(project_root) {
        Ok(root) => root,
        Err(err) => {
            report.errors.push(err.message);
            return report;
        }
    };
    report.project_root = Some(root.clone());
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let mut sources = BTreeMap::new();
    if !clean {
        for name in required.iter().collect::<BTreeSet<_>>() {
            let source = validate_name(name).and_then(|_| {
                let skill = registry.find(name).ok_or_else(|| {
                    ArcError::new(format!("required skill '{name}' is unavailable"))
                })?;
                if options.dry_run {
                    registry.resolve_source_path_readonly(&skill)
                } else {
                    registry.resolve_source_path(&skill)
                }
            });
            sources.insert(name.clone(), source);
        }
    }
    if options.dry_run {
        let result = with_snapshot(paths, |ledger, journals| {
            let agents = selected_agents(cache, &root, options, ledger, journals)?;
            let mut plan = build_plan(
                paths, cache, &root, &agents, &sources, options, clean, ledger, journals,
            );
            let mut snapshot = InstallReport::new(InstallScope::Project, Some(root.clone()), true);
            snapshot.errors.extend_from_slice(preparation_errors);
            if agents.is_empty() && !sources.is_empty() {
                snapshot.errors.push(
                    "Choose target agent(s): pass --agent <name> or --all-agents.".to_string(),
                );
            }
            snapshot
                .items
                .extend(plan.drain(..).map(|entry| entry.item));
            for journal in journals
                .iter()
                .filter(|journal| journal.selected(InstallScope::Project, Some(&root), &agents))
            {
                let mut item = InstallItem::from_record(
                    &journal.context,
                    InstallAction::Recover,
                    InstallStatus::Unresolved,
                );
                item.message = Some(
                    "pending installation requires recovery by a normal scoped command".to_string(),
                );
                snapshot.items.push(item);
            }
            Ok(snapshot)
        });
        return match result {
            Ok(report) => report,
            Err(err) => {
                report.errors.push(err.message);
                report
            }
        };
    }

    let mut store = match InstallStore::open(paths) {
        Ok(store) => store,
        Err(err) => {
            report.errors.push(err.message);
            return report;
        }
    };
    let agents = match selected_agents(cache, &root, options, &store.ledger, &store.journals) {
        Ok(agents) => agents,
        Err(err) => {
            report.errors.push(err.message);
            return report;
        }
    };
    let recovered: Vec<_> = store
        .journals
        .iter()
        .filter(|journal| journal.selected(InstallScope::Project, Some(&root), &agents))
        .map(|journal| journal.context.clone())
        .collect();
    if let Err(err) = store.recover(InstallScope::Project, Some(&root), &agents) {
        report.errors.push(err.message);
        for context in recovered {
            report.items.push(InstallItem::from_record(
                &context,
                InstallAction::Recover,
                InstallStatus::Unresolved,
            ));
        }
        return report;
    }
    report.items.extend(recovered.iter().map(|record| {
        InstallItem::from_record(record, InstallAction::Recover, InstallStatus::Recovered)
    }));
    report.errors.extend_from_slice(preparation_errors);
    if agents.is_empty() && !sources.is_empty() {
        report
            .errors
            .push("Choose target agent(s): pass --agent <name> or --all-agents.".to_string());
    }
    let plan = build_plan(
        paths,
        cache,
        &root,
        &agents,
        &sources,
        options,
        clean,
        &store.ledger,
        &store.journals,
    );
    let mut blocked =
        !report.errors.is_empty() || plan.iter().any(|entry| entry.item.status.is_issue());
    for mut entry in plan {
        if let Some(operation) = entry.operation {
            if blocked {
                entry.item.status = InstallStatus::NotExecuted;
            } else {
                match store.execute(operation) {
                    Ok(()) => {
                        entry.item.status = match entry.item.action {
                            InstallAction::Install => InstallStatus::Installed,
                            InstallAction::Refresh => InstallStatus::Refreshed,
                            InstallAction::Adopt => InstallStatus::Adopted,
                            InstallAction::Remove => InstallStatus::Removed,
                            InstallAction::Forget => InstallStatus::Absent,
                            _ => InstallStatus::Kept,
                        }
                    }
                    Err(err) => {
                        entry.item.status = if entry
                            .item
                            .target_path
                            .as_ref()
                            .is_some_and(|target| store.committed_pending(target))
                        {
                            InstallStatus::CleanupPending
                        } else {
                            InstallStatus::Failed
                        };
                        entry.item.message = Some(err.message);
                        blocked = true;
                    }
                }
            }
        }
        report.items.push(entry.item);
    }
    report
}

#[allow(clippy::too_many_arguments)]
fn build_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    root: &Path,
    agents: &[String],
    sources: &BTreeMap<String, Result<PathBuf>>,
    options: &ProjectSkillOptions,
    clean: bool,
    ledger: &InstallLedger,
    journals: &[Journal],
) -> Vec<PlannedInstall> {
    let mut result = Vec::new();
    let mut desired = BTreeSet::new();
    if !clean {
        for (skill, source) in sources {
            for agent in agents {
                let mut item = InstallItem {
                    scope: InstallScope::Project,
                    project_root: Some(root.to_path_buf()),
                    agent: agent.clone(),
                    skill: skill.clone(),
                    source_path: source.as_ref().ok().cloned(),
                    target_path: None,
                    action: InstallAction::Inspect,
                    status: InstallStatus::Planned,
                    message: None,
                };
                let target = match candidate_destinations(
                    paths,
                    InstallScope::Project,
                    Some(root),
                    agent,
                    skill,
                ) {
                    Ok(targets) => targets[0].clone(),
                    Err(err) => {
                        result.push(issue(item, InstallStatus::Conflict, err.message));
                        continue;
                    }
                };
                item.target_path = Some(target.clone());
                if !desired.insert(target.clone()) {
                    result.push(issue(
                        item,
                        InstallStatus::Conflict,
                        "multiple agents or skills claim the same physical target".to_string(),
                    ));
                    continue;
                }
                if journals
                    .iter()
                    .any(|journal| journal.context.target_path == target)
                {
                    result.push(issue(
                        item,
                        InstallStatus::Unresolved,
                        "target has a pending installation operation".to_string(),
                    ));
                    continue;
                }
                let source = match source {
                    Ok(source) => source,
                    Err(err) => {
                        result.push(issue(
                            item,
                            if options.dry_run {
                                InstallStatus::Unresolved
                            } else {
                                InstallStatus::Unavailable
                            },
                            err.message.clone(),
                        ));
                        continue;
                    }
                };
                let strategy = agent_spec(agent).unwrap().skill_install_strategy;
                let mut after = match InstallRecord::new(
                    InstallScope::Project,
                    Some(root),
                    agent,
                    skill,
                    source,
                    &target,
                    strategy,
                ) {
                    Ok(record) => record,
                    Err(err) => {
                        result.push(issue(item, InstallStatus::Unavailable, err.message));
                        continue;
                    }
                };
                if let Some(before) = ledger.record(&target) {
                    if !before.same_owner(&after) {
                        result.push(issue(
                            item,
                            InstallStatus::Conflict,
                            "target is owned by another agent or scope".to_string(),
                        ));
                        continue;
                    }
                    match inspect_owned(paths, before) {
                        Err(err) => result.push(issue(item, InstallStatus::Conflict, err.message)),
                        Ok(present) => {
                            let refresh = before.source_path != after.source_path
                                || before.strategy != after.strategy
                                || (strategy == SkillInstallStrategy::Copy
                                    && before.source_fingerprint != after.source_fingerprint);
                            if !present || refresh {
                                item.action = if present {
                                    InstallAction::Refresh
                                } else {
                                    InstallAction::Install
                                };
                                result.push(PlannedInstall {
                                    item,
                                    operation: Some(InstallOperation::put(
                                        after,
                                        Some(before.clone()),
                                        false,
                                    )),
                                });
                            } else {
                                item.action = InstallAction::Keep;
                                item.status = InstallStatus::Kept;
                                result.push(PlannedInstall {
                                    item,
                                    operation: None,
                                });
                            }
                        }
                    }
                } else {
                    match NodeState::capture(&target) {
                        Err(err) => result.push(issue(item, InstallStatus::Unavailable, err.message)),
                        Ok(None) => {
                            if cache.get_agent(agent).is_none() {
                                result.push(issue(item, InstallStatus::Unavailable, format!("Agent '{agent}' not detected; a new destination requires a detected agent")));
                            } else {
                                item.action = InstallAction::Install;
                                result.push(PlannedInstall { item, operation: Some(InstallOperation::put(after, None, false)) });
                            }
                        }
                        Ok(Some(_)) if options.adopt_existing => {
                            if strategy == SkillInstallStrategy::Copy {
                                match fingerprint_path(&target) {
                                    Ok(fingerprint) if fingerprint == after.source_fingerprint => after.target_fingerprint = Some(fingerprint),
                                    _ => { result.push(issue(item, InstallStatus::Conflict, "existing copy does not match the requested source".to_string())); continue; }
                                }
                            }
                            match inspect_owned(paths, &after) {
                                Ok(true) => {
                                    item.action = InstallAction::Adopt;
                                    result.push(PlannedInstall { item, operation: Some(InstallOperation::put(after, None, true)) });
                                }
                                _ => result.push(issue(item, InstallStatus::Conflict, "existing target does not match the requested source; cannot adopt".to_string())),
                            }
                        }
                        Ok(Some(_)) => result.push(issue(item, InstallStatus::Unmanaged, "existing target is unmanaged; use --adopt-existing to adopt a matching required skill".to_string())),
                    }
                }
            }
        }
    }
    for record in ledger
        .in_scope(InstallScope::Project, Some(root))
        .filter(|record| agents.contains(&record.agent))
    {
        if desired.contains(&record.target_path) {
            continue;
        }
        let mut item =
            InstallItem::from_record(record, InstallAction::Remove, InstallStatus::Planned);
        if journals
            .iter()
            .any(|journal| journal.context.target_path == record.target_path)
        {
            result.push(issue(
                item,
                InstallStatus::Unresolved,
                "target requires pending recovery".to_string(),
            ));
            continue;
        }
        if let Err(err) = validate_destination(paths, record) {
            result.push(issue(item, InstallStatus::Conflict, err.message));
            continue;
        }
        match inspect_owned(paths, record) {
            Ok(present) => {
                if !present {
                    item.action = InstallAction::Forget;
                }
                let mut operation = InstallOperation::remove(record.clone());
                operation.metadata_only = !present;
                result.push(PlannedInstall {
                    item,
                    operation: Some(operation),
                });
            }
            Err(err) => result.push(issue(item, InstallStatus::Conflict, err.message)),
        }
    }
    // 先完成全部期望落点的安装，再开始删除多余目标。
    result.sort_by_key(|entry| {
        matches!(
            entry.item.action,
            InstallAction::Remove | InstallAction::Forget
        )
    });
    result
}

fn issue(mut item: InstallItem, status: InstallStatus, message: String) -> PlannedInstall {
    item.status = status;
    item.message = Some(message);
    PlannedInstall {
        item,
        operation: None,
    }
}
