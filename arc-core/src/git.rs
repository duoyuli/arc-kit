use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{ArcError, Result};

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub local_path: PathBuf,
}

impl GitRepo {
    pub fn new(local_path: impl Into<PathBuf>) -> Self {
        Self {
            local_path: local_path.into(),
        }
    }

    pub fn pull_default_branch(&self, remote: &str) -> Result<()> {
        self.fetch(remote)?;
        let branch = self.remote_default_branch(remote)?;
        run_git_dyn(
            ["pull", "--ff-only", remote, branch.as_str()],
            &self.local_path,
        )
    }

    pub fn pull(&self, remote: &str, branch: &str) -> Result<()> {
        run_git(["pull", remote, branch], &self.local_path)
    }

    pub fn fetch(&self, remote: &str) -> Result<()> {
        run_git(["fetch", remote], &self.local_path)
    }

    pub fn get_revision(&self) -> String {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.local_path)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .chars()
                    .take(7)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn remote_default_branch(&self, remote: &str) -> Result<String> {
        if let Ok(symbolic_ref) = run_git_capture(
            [
                "symbolic-ref",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ],
            &self.local_path,
        ) && let Some((prefix, branch)) = symbolic_ref.trim().split_once('/')
            && prefix == remote
            && !branch.is_empty()
        {
            return Ok(branch.to_string());
        }

        let output = run_git_capture(["ls-remote", "--symref", remote, "HEAD"], &self.local_path)?;
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("ref: refs/heads/")
                && let Some((branch, target)) = rest.split_once('\t')
                && target == "HEAD"
                && !branch.is_empty()
            {
                return Ok(branch.to_string());
            }
        }

        Err(ArcError::new(format!(
            "failed to detect default branch for remote: {remote}"
        )))
    }
}

pub fn clone(url: &str, target_dir: &Path, branch: Option<&str>) -> Result<GitRepo> {
    if let Some(parent) = target_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| ArcError::new(format!("failed to create checkout directory: {err}")))?;
    }
    let mut cmd = Command::new("git");
    cmd.arg("clone");
    if let Some(branch) = branch {
        cmd.args(["--branch", branch]);
    }
    cmd.arg(url).arg(target_dir);
    let output = cmd
        .output()
        .map_err(|err| ArcError::new(format!("failed to spawn git clone: {err}")))?;
    if !output.status.success() {
        return Err(ArcError::new(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(GitRepo::new(target_dir))
}

pub fn validate_git_url(url: &str) -> bool {
    ["https://", "git://", "ssh://", "git@", "file://"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

fn run_git<const N: usize>(args: [&str; N], cwd: &Path) -> Result<()> {
    run_git_dyn(args, cwd)
}

fn run_git_dyn<I, S>(args: I, cwd: &Path) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let output = Command::new("git")
        .args(args.into_iter().map(|arg| arg.as_ref().to_string()))
        .current_dir(cwd)
        .output()
        .map_err(|err| ArcError::new(format!("failed to spawn git: {err}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ArcError::new(format!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn run_git_capture<const N: usize>(args: [&str; N], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|err| ArcError::new(format!("failed to spawn git: {err}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(ArcError::new(format!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}
