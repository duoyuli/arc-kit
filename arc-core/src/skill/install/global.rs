use std::collections::BTreeSet;
use std::path::Path;

use super::*;
use crate::agent::agent_spec;
use crate::detect::DetectCache;
use crate::paths::ArcPaths;

pub fn install_global_skill(
    paths: &ArcPaths,
    cache: &DetectCache,
    skill: &str,
    source: &Path,
    agents: &[String],
) -> InstallReport {
    install_scoped_skill(
        paths,
        cache,
        InstallScope::Global,
        None,
        skill,
        source,
        agents,
    )
}

/// 安装一项已声明的项目技能，不清理其他声明。
pub fn install_project_skill(
    paths: &ArcPaths,
    cache: &DetectCache,
    root: &Path,
    skill: &str,
    source: &Path,
    agents: &[String],
) -> InstallReport {
    match normalize_root(root) {
        Ok(root) => install_scoped_skill(
            paths,
            cache,
            InstallScope::Project,
            Some(&root),
            skill,
            source,
            agents,
        ),
        Err(err) => {
            let mut report =
                InstallReport::new(InstallScope::Project, Some(root.to_path_buf()), false);
            report.errors.push(err.message);
            report
        }
    }
}

fn install_scoped_skill(
    paths: &ArcPaths,
    cache: &DetectCache,
    scope: InstallScope,
    root: Option<&Path>,
    skill: &str,
    source: &Path,
    agents: &[String],
) -> InstallReport {
    let mut report = InstallReport::new(scope, root.map(Path::to_path_buf), false);
    let mut store = match InstallStore::open(paths) {
        Ok(store) => store,
        Err(err) => {
            report.errors.push(err.message);
            return report;
        }
    };
    if let Err(err) = store.recover(scope, root, agents) {
        report.errors.push(err.message);
        return report;
    }
    let mut stopped = false;
    for agent in agents.iter().collect::<BTreeSet<_>>() {
        let mut item = InstallItem {
            scope,
            project_root: root.map(Path::to_path_buf),
            agent: agent.clone(),
            skill: skill.to_string(),
            source_path: Some(source.to_path_buf()),
            target_path: None,
            action: InstallAction::Install,
            status: InstallStatus::Planned,
            message: None,
        };
        if stopped {
            item.status = InstallStatus::NotExecuted;
            report.items.push(item);
            continue;
        }
        let result = (|| -> Result<()> {
            let target = candidate_destinations(paths, scope, root, agent, skill)?[0].clone();
            item.target_path = Some(target.clone());
            let strategy = agent_spec(agent).unwrap().skill_install_strategy;
            let record = InstallRecord::new(scope, root, agent, skill, source, &target, strategy)?;
            let previous = store.ledger.record(&target).cloned();
            if let Some(before) = &previous {
                if !before.same_owner(&record) {
                    return Err(ArcError::new(
                        "global installation collides with another scope",
                    ));
                }
                let present = inspect_owned(paths, before)?;
                if present
                    && before.source_path == record.source_path
                    && before.strategy == record.strategy
                    && (strategy == SkillInstallStrategy::Symlink
                        || before.source_fingerprint == record.source_fingerprint)
                {
                    item.action = InstallAction::Keep;
                    item.status = InstallStatus::Kept;
                    return Ok(());
                }
                item.action = if present {
                    InstallAction::Refresh
                } else {
                    InstallAction::Install
                };
            } else if NodeState::capture(&target)?.is_some() {
                if scope == InstallScope::Project {
                    return Err(ArcError::new(
                        "existing project target is unmanaged; use project apply --adopt-existing",
                    ));
                }
                item.action = InstallAction::Keep;
                item.status = InstallStatus::Kept;
                item.message =
                    Some("Existing unmanaged global installation left in place.".to_string());
                return Ok(());
            } else if cache.get_agent(agent).is_none() {
                return Err(ArcError::new(format!("Agent '{agent}' not detected")));
            }
            store.execute(InstallOperation::put(record, previous, false))?;
            item.status = if item.action == InstallAction::Refresh {
                InstallStatus::Refreshed
            } else {
                InstallStatus::Installed
            };
            Ok(())
        })();
        if let Err(err) = result {
            item.status = if item
                .target_path
                .as_ref()
                .is_some_and(|target| store.committed_pending(target))
            {
                InstallStatus::CleanupPending
            } else {
                InstallStatus::Failed
            };
            item.message = Some(err.message);
            stopped = true;
        }
        report.items.push(item);
    }
    report
}

pub fn uninstall_global_skill(
    paths: &ArcPaths,
    cache: &DetectCache,
    skill: &str,
    selected: Option<&[String]>,
) -> InstallReport {
    let mut report = InstallReport::new(InstallScope::Global, None, false);
    let mut store = match InstallStore::open(paths) {
        Ok(store) => store,
        Err(err) => {
            report.errors.push(err.message);
            return report;
        }
    };
    if let Err(err) = validate_name(skill) {
        report.errors.push(err.message);
        return report;
    }
    let agents: Vec<String> = match selected {
        Some(agents) => agents
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        None => cache
            .detected_agents()
            .keys()
            .cloned()
            .chain(
                store
                    .ledger
                    .in_scope(InstallScope::Global, None)
                    .filter(|record| record.skill == skill)
                    .map(|record| record.agent.clone()),
            )
            .chain(
                store
                    .ledger
                    .unresolved_legacy
                    .iter()
                    .filter(|record| {
                        record.get("skill").and_then(serde_json::Value::as_str) == Some(skill)
                    })
                    .filter_map(|record| {
                        record
                            .get("agent")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    }),
            )
            .chain(
                store
                    .journals
                    .iter()
                    .filter(|journal| {
                        journal.context.scope == InstallScope::Global
                            && journal.context.skill == skill
                    })
                    .map(|journal| journal.context.agent.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    };
    if let Err(err) = store.recover(InstallScope::Global, None, &agents) {
        report.errors.push(err.message);
        return report;
    }
    let mut operations = Vec::new();
    let mut seen = BTreeSet::new();
    for legacy in &store.ledger.unresolved_legacy {
        if legacy.get("skill").and_then(serde_json::Value::as_str) != Some(skill) {
            continue;
        }
        let agent = legacy
            .get("agent")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        if !agents.iter().any(|selected| selected == agent) {
            continue;
        }
        report.items.push(InstallItem {
            scope: InstallScope::Global,
            project_root: None,
            agent: agent.to_string(),
            skill: skill.to_string(),
            source_path: legacy
                .get("source_path")
                .and_then(serde_json::Value::as_str)
                .map(std::path::PathBuf::from),
            target_path: None,
            action: InstallAction::Inspect,
            status: InstallStatus::Unresolved,
            message: Some(
                "legacy destination or ownership is unresolved; original record preserved"
                    .to_string(),
            ),
        });
    }
    for record in store
        .ledger
        .in_scope(InstallScope::Global, None)
        .filter(|record| record.skill == skill && agents.contains(&record.agent))
    {
        let mut item =
            InstallItem::from_record(record, InstallAction::Remove, InstallStatus::Planned);
        seen.insert(record.target_path.clone());
        match inspect_owned(paths, record) {
            Ok(present) => {
                let mut operation = InstallOperation::remove(record.clone());
                if !present {
                    item.action = InstallAction::Forget;
                    operation.metadata_only = true;
                }
                operations.push((item, Some(operation)));
            }
            Err(err) => {
                item.status = InstallStatus::Conflict;
                item.message = Some(err.message);
                operations.push((item, None));
            }
        }
    }
    for agent in &agents {
        if cache.get_agent(agent).is_none() {
            continue;
        }
        let targets = match candidate_destinations(paths, InstallScope::Global, None, agent, skill)
        {
            Ok(targets) => targets,
            Err(err) => {
                report.errors.push(err.message);
                continue;
            }
        };
        for target in targets {
            if !seen.insert(target.clone()) {
                continue;
            }
            if store.ledger.record(&target).is_some() {
                report.errors.push(format!(
                    "global uninstall target belongs to another scope: {}",
                    target.display()
                ));
                continue;
            }
            let state = match NodeState::capture(&target) {
                Ok(state) => state,
                Err(err) => {
                    report.errors.push(err.message);
                    continue;
                }
            };
            if state.is_none() {
                continue;
            }
            // 显式全局卸载继续支持手工安装的条目。
            let context = InstallRecord {
                scope: InstallScope::Global,
                project_root: None,
                agent: agent.clone(),
                skill: skill.to_string(),
                target_path: target,
                source_path: paths.user_home().to_path_buf(),
                strategy: agent_spec(agent).unwrap().skill_install_strategy,
                source_fingerprint: String::new(),
                target_fingerprint: None,
            };
            let item =
                InstallItem::from_record(&context, InstallAction::Remove, InstallStatus::Planned);
            operations.push((
                item,
                Some(InstallOperation {
                    context,
                    before: None,
                    after: None,
                    metadata_only: false,
                    allow_unmanaged_remove: true,
                }),
            ));
        }
    }
    let mut blocked = !report.ok() || operations.iter().any(|(item, _)| item.status.is_issue());
    for (mut item, operation) in operations {
        if let Some(operation) = operation {
            if blocked {
                item.status = InstallStatus::NotExecuted;
            } else {
                match store.execute(operation) {
                    Ok(()) => {
                        item.status = if item.action == InstallAction::Forget {
                            InstallStatus::Absent
                        } else {
                            InstallStatus::Removed
                        }
                    }
                    Err(err) => {
                        item.status = if item
                            .target_path
                            .as_ref()
                            .is_some_and(|target| store.committed_pending(target))
                        {
                            InstallStatus::CleanupPending
                        } else {
                            InstallStatus::Failed
                        };
                        item.message = Some(err.message);
                        blocked = true;
                    }
                }
            }
        }
        report.items.push(item);
    }
    report
}
