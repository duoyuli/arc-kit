use std::fs;

use crate::adapters::base::{AgentContext, ApplyResult, Change, ResourceAdapter, Snapshot};
use crate::agent::{SkillInstallStrategy, agent_specs};
use crate::models::ResourceKind;

pub fn all_resource_adapters() -> Vec<Box<dyn ResourceAdapter>> {
    let mut out: Vec<Box<dyn ResourceAdapter>> = Vec::new();
    for agent in agent_specs() {
        if !agent.supports_skills {
            continue;
        }
        match agent.skill_install_strategy {
            SkillInstallStrategy::Symlink => out.push(Box::new(SymlinkSkillAdapter::new(
                agent.id,
                agent.skills_subdir,
            ))),
            SkillInstallStrategy::Copy => out.push(Box::new(CopySkillAdapter::new(
                agent.id,
                agent.skills_subdir,
            ))),
        }
    }
    out
}

#[derive(Debug)]
struct SymlinkSkillAdapter {
    agent_name: &'static str,
    subdir: &'static str,
}

impl SymlinkSkillAdapter {
    fn new(agent_name: &'static str, subdir: &'static str) -> Self {
        Self { agent_name, subdir }
    }
}

impl ResourceAdapter for SymlinkSkillAdapter {
    fn supports(&self, snapshot: &Snapshot, agent: &AgentContext) -> bool {
        agent.name == self.agent_name && snapshot.kind == ResourceKind::Skill
    }

    fn apply(&self, snapshot: &Snapshot, agent: &AgentContext) -> ApplyResult {
        let skills_dir = agent.root.join(self.subdir);
        let target = skills_dir.join(&snapshot.name);
        if let Err(err) = fs::create_dir_all(&skills_dir) {
            return ApplyResult::err(format!("failed to create skills dir: {err}"));
        }
        if (target.exists() || target.symlink_metadata().is_ok())
            && fs::remove_file(&target).is_err()
            && fs::remove_dir_all(&target).is_err()
        {
            return ApplyResult::err(format!(
                "failed to replace existing target {}",
                target.display()
            ));
        }
        #[cfg(unix)]
        {
            if let Err(err) = std::os::unix::fs::symlink(&snapshot.path, &target) {
                return ApplyResult::err(format!("failed to create symlink: {err}"));
            }
        }
        #[cfg(not(unix))]
        {
            return ApplyResult::err("symlink install is unsupported on this platform");
        }
        ApplyResult::ok(
            format!("Successfully installed skill {}", snapshot.name),
            vec![Change {
                action: "symlink".to_string(),
                target: target.display().to_string(),
                summary: format!("Symlinked {} into {}", snapshot.name, self.subdir),
            }],
        )
    }

    fn uninstall(&self, snapshot: &Snapshot, agent: &AgentContext) -> ApplyResult {
        let target = agent.root.join(self.subdir).join(&snapshot.name);
        if !target.exists() && target.symlink_metadata().is_err() {
            return ApplyResult::ok(
                format!("Skill {} was not installed", snapshot.name),
                Vec::new(),
            );
        }
        let result = if target.is_dir() && !target.is_symlink() {
            fs::remove_dir_all(&target)
        } else {
            fs::remove_file(&target)
        };
        match result {
            Ok(_) => ApplyResult::ok(
                format!("Successfully uninstalled skill {}", snapshot.name),
                vec![Change {
                    action: "unlink".to_string(),
                    target: target.display().to_string(),
                    summary: format!("Removed skill {}", snapshot.name),
                }],
            ),
            Err(err) => ApplyResult::err(format!(
                "failed to uninstall skill {}: {err}",
                snapshot.name
            )),
        }
    }
}

#[derive(Debug)]
struct CopySkillAdapter {
    agent_name: &'static str,
    subdir: &'static str,
}

impl CopySkillAdapter {
    fn new(agent_name: &'static str, subdir: &'static str) -> Self {
        Self { agent_name, subdir }
    }
}

impl ResourceAdapter for CopySkillAdapter {
    fn supports(&self, snapshot: &Snapshot, agent: &AgentContext) -> bool {
        agent.name == self.agent_name && snapshot.kind == ResourceKind::Skill
    }

    fn apply(&self, snapshot: &Snapshot, agent: &AgentContext) -> ApplyResult {
        let target = agent.root.join(self.subdir).join(&snapshot.name);
        if let Some(parent) = target.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            return ApplyResult::err(format!("failed to create target dir: {err}"));
        }
        if target.exists()
            && fs::remove_dir_all(&target).is_err()
            && fs::remove_file(&target).is_err()
        {
            return ApplyResult::err(format!(
                "failed to replace existing target {}",
                target.display()
            ));
        }
        if let Err(err) = copy_dir_recursive(&snapshot.path, &target) {
            return ApplyResult::err(format!("failed to copy skill: {err}"));
        }
        ApplyResult::ok(
            format!("Successfully installed skill {}", snapshot.name),
            vec![Change {
                action: "copy".to_string(),
                target: target.display().to_string(),
                summary: format!("Copied {} into {}", snapshot.name, self.subdir),
            }],
        )
    }

    fn uninstall(&self, snapshot: &Snapshot, agent: &AgentContext) -> ApplyResult {
        let target = agent.root.join(self.subdir).join(&snapshot.name);
        if !target.exists() {
            return ApplyResult::ok(
                format!("Skill {} was not installed", snapshot.name),
                Vec::new(),
            );
        }
        let result = if target.is_dir() {
            fs::remove_dir_all(&target)
        } else {
            fs::remove_file(&target)
        };
        match result {
            Ok(_) => ApplyResult::ok(
                format!("Successfully uninstalled skill {}", snapshot.name),
                vec![Change {
                    action: "remove".to_string(),
                    target: target.display().to_string(),
                    summary: format!("Removed copied skill {}", snapshot.name),
                }],
            ),
            Err(err) => ApplyResult::err(format!("failed to uninstall skill: {err}")),
        }
    }
}

fn copy_dir_recursive(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else {
            fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}
