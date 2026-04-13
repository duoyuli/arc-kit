use super::*;

pub(super) fn collect_capabilities(
    paths: &ArcPaths,
    cwd: &Path,
    cache: &DetectCache,
) -> (Vec<CapabilityStatusEntry>, Vec<CapabilityStatusEntry>) {
    let project_config =
        find_project_config(cwd).and_then(|path| match load_project_config(&path) {
            Ok(config) => path_with_root(cwd, config),
            Err(_) => None,
        });

    let project_mcp_entries = project_config
        .as_ref()
        .map(|(project_root, cfg)| collect_project_mcp_entries(paths, cache, project_root, cfg))
        .unwrap_or_default();
    let project_subagent_entries = project_config
        .as_ref()
        .map(|(project_root, cfg)| {
            collect_project_subagent_entries(paths, cache, project_root, cfg)
        })
        .unwrap_or_default();

    let mut mcps = Vec::new();
    match load_user_registry_mcps(paths) {
        Ok(global_mcps) => {
            for definition in global_mcps {
                let plan = McpApplyPlan {
                    definition: definition.clone(),
                    source_scope: SourceScope::Global,
                };
                let targets = observe_capability_targets(
                    paths,
                    ResourceKind::Mcp,
                    &definition.name,
                    SourceScope::Global,
                    None,
                    preview_mcp_plan(paths, cache, &plan, None).unwrap_or_default(),
                );
                mcps.push(CapabilityStatusEntry {
                    name: definition.name.clone(),
                    kind: ResourceKind::Mcp,
                    source_scope: SourceScope::Global,
                    managed_by_arc: true,
                    declared_targets: definition.targets.clone(),
                    resolution: if is_fully_shadowed_by_project_targets(
                        &definition.name,
                        &targets,
                        &project_mcp_entries,
                    ) {
                        ResourceResolution::Shadowed
                    } else {
                        ResourceResolution::Active
                    },
                    targets,
                });
            }
        }
        Err(err) => mcps.push(invalid_global_capability_entry(
            ResourceKind::Mcp,
            "invalid-global-mcp-config",
            err.message,
        )),
    }
    mcps.extend(project_mcp_entries);

    let mut subagents = Vec::new();
    match load_global_subagents(paths) {
        Ok(global_subagents) => {
            for definition in global_subagents {
                let plan = SubagentApplyPlan {
                    prompt_path: Some(PathBuf::from(&definition.prompt_file)),
                    prompt_body: None,
                    definition: definition.clone(),
                    source_scope: SourceScope::Global,
                };
                let targets = observe_capability_targets(
                    paths,
                    ResourceKind::SubAgent,
                    &definition.name,
                    SourceScope::Global,
                    None,
                    preview_subagent_plan(paths, cache, &plan, None).unwrap_or_default(),
                );
                subagents.push(CapabilityStatusEntry {
                    name: definition.name.clone(),
                    kind: ResourceKind::SubAgent,
                    source_scope: SourceScope::Global,
                    managed_by_arc: true,
                    declared_targets: definition.targets.clone(),
                    resolution: if is_fully_shadowed_by_project_targets(
                        &definition.name,
                        &targets,
                        &project_subagent_entries,
                    ) {
                        ResourceResolution::Shadowed
                    } else {
                        ResourceResolution::Active
                    },
                    targets,
                });
            }
        }
        Err(err) => subagents.push(invalid_global_capability_entry(
            ResourceKind::SubAgent,
            "invalid-global-subagent-config",
            err.message,
        )),
    }
    subagents.extend(project_subagent_entries);

    mcps.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.source_scope.cmp(&b.source_scope))
    });
    subagents.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.source_scope.cmp(&b.source_scope))
    });
    (mcps, subagents)
}

fn path_with_root(
    cwd: &Path,
    config: crate::project::ProjectConfig,
) -> Option<(PathBuf, crate::project::ProjectConfig)> {
    let config_path = find_project_config(cwd)?;
    let root = config_path.parent()?.to_path_buf();
    Some((root, config))
}

fn invalid_capability_targets(
    cache: &DetectCache,
    declared_targets: Option<&Vec<String>>,
    desired_scope: DesiredScope,
    reason: String,
) -> Vec<CapabilityTargetStatus> {
    let targets = resolve_declared_targets(cache, declared_targets);
    if targets.is_empty() {
        return vec![CapabilityTargetStatus {
            agent: "-".to_string(),
            status: CapabilityTargetState::Failed,
            desired_scope,
            applied_scope: AppliedScope::None,
            reason: Some(reason),
        }];
    }

    targets
        .into_iter()
        .map(|agent| CapabilityTargetStatus {
            agent,
            status: CapabilityTargetState::Failed,
            desired_scope,
            applied_scope: AppliedScope::None,
            reason: Some(reason.clone()),
        })
        .collect()
}

fn collect_project_mcp_entries(
    paths: &ArcPaths,
    cache: &DetectCache,
    project_root: &Path,
    cfg: &crate::project::ProjectConfig,
) -> Vec<CapabilityStatusEntry> {
    let requirements = match resolve_project_capability_requirements(paths, cfg) {
        Ok(requirements) => requirements,
        Err(err) => {
            return cfg
                .mcps
                .require
                .iter()
                .map(|name| {
                    invalid_project_capability_entry(
                        ResourceKind::Mcp,
                        name,
                        DesiredScope::Project,
                        err.message.clone(),
                    )
                })
                .collect();
        }
    };
    let mut entries = Vec::new();
    for definition in &requirements.mcps {
        let mut definition = definition.clone();
        let targets = match validate_mcp_definition(&mut definition) {
            Ok(()) => {
                let plan = McpApplyPlan {
                    definition: definition.clone(),
                    source_scope: SourceScope::Project,
                };
                let preview = preview_mcp_plan(paths, cache, &plan, Some(project_root))
                    .unwrap_or_else(|err| {
                        invalid_capability_targets(
                            cache,
                            definition.targets.as_ref(),
                            DesiredScope::Project,
                            err.message,
                        )
                    });
                observe_capability_targets(
                    paths,
                    ResourceKind::Mcp,
                    &definition.name,
                    SourceScope::Project,
                    Some(project_root),
                    preview,
                )
            }
            Err(err) => invalid_capability_targets(
                cache,
                definition.targets.as_ref(),
                DesiredScope::Project,
                err.message,
            ),
        };
        entries.push(CapabilityStatusEntry {
            name: definition.name.clone(),
            kind: ResourceKind::Mcp,
            source_scope: SourceScope::Project,
            managed_by_arc: true,
            declared_targets: definition.targets.clone(),
            resolution: ResourceResolution::Active,
            targets,
        });
    }
    entries.extend(requirements.unavailable_mcps.into_iter().map(|name| {
        invalid_project_capability_entry(
            ResourceKind::Mcp,
            &name,
            DesiredScope::Project,
            "not_in_catalog".to_string(),
        )
    }));
    entries
}

fn collect_project_subagent_entries(
    paths: &ArcPaths,
    cache: &DetectCache,
    project_root: &Path,
    cfg: &crate::project::ProjectConfig,
) -> Vec<CapabilityStatusEntry> {
    let requirements = match resolve_project_capability_requirements(paths, cfg) {
        Ok(requirements) => requirements,
        Err(err) => {
            return cfg
                .subagents
                .require
                .iter()
                .map(|name| {
                    invalid_project_capability_entry(
                        ResourceKind::SubAgent,
                        name,
                        DesiredScope::Project,
                        err.message.clone(),
                    )
                })
                .collect();
        }
    };
    let mut entries = Vec::new();
    for entry in &requirements.subagents {
        let definition = &entry.definition;
        let targets = match validate_subagent_targets(cache, definition) {
            Ok(_) => {
                let plan = SubagentApplyPlan {
                    prompt_path: None,
                    prompt_body: Some(entry.prompt_body.clone()),
                    definition: definition.clone(),
                    source_scope: SourceScope::Project,
                };
                let preview = preview_subagent_plan(paths, cache, &plan, Some(project_root))
                    .unwrap_or_else(|err| {
                        invalid_capability_targets(
                            cache,
                            definition.targets.as_ref(),
                            DesiredScope::Project,
                            err.message,
                        )
                    });
                observe_capability_targets(
                    paths,
                    ResourceKind::SubAgent,
                    &definition.name,
                    SourceScope::Project,
                    Some(project_root),
                    preview,
                )
            }
            Err(err) => invalid_capability_targets(
                cache,
                definition.targets.as_ref(),
                DesiredScope::Project,
                err.message,
            ),
        };
        entries.push(CapabilityStatusEntry {
            name: definition.name.clone(),
            kind: ResourceKind::SubAgent,
            source_scope: SourceScope::Project,
            managed_by_arc: true,
            declared_targets: definition.targets.clone(),
            resolution: ResourceResolution::Active,
            targets,
        });
    }
    entries.extend(requirements.unavailable_subagents.into_iter().map(|name| {
        invalid_project_capability_entry(
            ResourceKind::SubAgent,
            &name,
            DesiredScope::Project,
            "not_in_catalog".to_string(),
        )
    }));
    entries
}

fn invalid_project_capability_entry(
    kind: ResourceKind,
    name: &str,
    desired_scope: DesiredScope,
    reason: String,
) -> CapabilityStatusEntry {
    CapabilityStatusEntry {
        name: name.to_string(),
        kind,
        source_scope: SourceScope::Project,
        managed_by_arc: true,
        declared_targets: None,
        resolution: ResourceResolution::Active,
        targets: vec![CapabilityTargetStatus {
            agent: "-".to_string(),
            status: CapabilityTargetState::Failed,
            desired_scope,
            applied_scope: AppliedScope::None,
            reason: Some(reason),
        }],
    }
}

fn observe_capability_targets(
    paths: &ArcPaths,
    kind: ResourceKind,
    name: &str,
    source_scope: SourceScope,
    project_root: Option<&Path>,
    targets: Vec<CapabilityTargetStatus>,
) -> Vec<CapabilityTargetStatus> {
    targets
        .into_iter()
        .map(|mut target| {
            let Some(record) =
                tracking_record_for_target(kind.clone(), name, source_scope, &target, project_root)
            else {
                return target;
            };
            match capability_install_present(paths, &record) {
                Ok(true) => target,
                Ok(false) => {
                    target.status = CapabilityTargetState::Failed;
                    target.reason = Some("drift_missing_from_disk".to_string());
                    target
                }
                Err(err) => {
                    target.status = CapabilityTargetState::Failed;
                    target.reason = Some(err.message);
                    target
                }
            }
        })
        .collect()
}

fn invalid_global_capability_entry(
    kind: ResourceKind,
    name: &str,
    reason: String,
) -> CapabilityStatusEntry {
    CapabilityStatusEntry {
        name: name.to_string(),
        kind,
        source_scope: SourceScope::Global,
        managed_by_arc: true,
        declared_targets: None,
        resolution: ResourceResolution::Active,
        targets: vec![CapabilityTargetStatus {
            agent: "-".to_string(),
            status: CapabilityTargetState::Failed,
            desired_scope: DesiredScope::Global,
            applied_scope: AppliedScope::None,
            reason: Some(reason),
        }],
    }
}

fn is_fully_shadowed_by_project_targets(
    name: &str,
    global_targets: &[CapabilityTargetStatus],
    project_entries: &[CapabilityStatusEntry],
) -> bool {
    let global_agents: BTreeSet<String> = global_targets
        .iter()
        .filter(|target| target.status == CapabilityTargetState::Applied)
        .map(|target| target.agent.clone())
        .collect();
    if global_agents.is_empty() {
        return false;
    }
    let project_agents: BTreeSet<String> = project_entries
        .iter()
        .filter(|entry| entry.name == name)
        .flat_map(|entry| entry.targets.iter())
        .filter(|target| target.status != CapabilityTargetState::Skipped)
        .map(|target| target.agent.clone())
        .collect();
    global_agents.is_subset(&project_agents)
}
