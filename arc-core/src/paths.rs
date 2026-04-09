use std::path::{Path, PathBuf};

use crate::models::MarketSource;

pub const ARC_CLI_HOME: &str = ".arc-cli";
pub const ARC_KIT_HOME_ENV: &str = "ARC_KIT_HOME";
pub const ARC_KIT_USER_HOME_ENV: &str = "ARC_KIT_USER_HOME";

#[derive(Debug, Clone)]
pub struct ArcPaths {
    arc_home: PathBuf,
    user_home: PathBuf,
}

impl Default for ArcPaths {
    fn default() -> Self {
        if let Some(arc_home) = std::env::var_os(ARC_KIT_HOME_ENV) {
            return Self::with_arc_home(PathBuf::from(arc_home));
        }
        if let Some(user_home) = std::env::var_os(ARC_KIT_USER_HOME_ENV) {
            return Self::with_user_home(PathBuf::from(user_home));
        }
        let user_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::with_user_home(user_home)
    }
}

impl ArcPaths {
    pub fn with_user_home(user_home: impl Into<PathBuf>) -> Self {
        let user_home = user_home.into();
        Self {
            arc_home: user_home.join(ARC_CLI_HOME),
            user_home,
        }
    }

    pub fn with_arc_home(arc_home: impl Into<PathBuf>) -> Self {
        let arc_home = arc_home.into();
        let user_home = arc_home
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            arc_home,
            user_home,
        }
    }

    pub fn user_home(&self) -> &Path {
        &self.user_home
    }

    pub fn home(&self) -> &Path {
        &self.arc_home
    }

    pub fn config(&self) -> PathBuf {
        self.arc_home.join("config.toml")
    }

    pub fn markets_dir(&self) -> PathBuf {
        self.arc_home.join("markets")
    }

    pub fn markets_repo_root(&self) -> PathBuf {
        self.markets_dir().join("repo")
    }

    pub fn providers_dir(&self) -> PathBuf {
        self.arc_home.join("providers")
    }

    pub fn catalog(&self) -> PathBuf {
        self.markets_dir().join("catalog.json")
    }

    pub fn market_index_cache(&self) -> PathBuf {
        self.markets_dir().join("index.toml")
    }

    pub fn local_skills_dir(&self) -> PathBuf {
        self.arc_home.join("skills")
    }

    pub fn mcps_dir(&self) -> PathBuf {
        self.arc_home.join("mcps")
    }

    pub fn subagents_dir(&self) -> PathBuf {
        self.arc_home.join("subagents")
    }

    pub fn tracking_dir(&self) -> PathBuf {
        self.arc_home.join("tracking")
    }

    pub fn builtin_cache_dir(&self) -> PathBuf {
        self.arc_home.join("cache").join("built-in")
    }

    pub fn ensure_arc_home(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.markets_repo_root())?;
        std::fs::create_dir_all(self.local_skills_dir())?;
        std::fs::create_dir_all(self.mcps_dir())?;
        std::fs::create_dir_all(self.subagents_dir())?;
        std::fs::create_dir_all(self.tracking_dir())?;
        Ok(())
    }

    pub fn market_checkout(&self, source: &MarketSource) -> PathBuf {
        let (owner, repo) = if !source.owner.is_empty() && !source.repo.is_empty() {
            (source.owner.clone(), source.repo.clone())
        } else if let Some((owner, repo)) =
            crate::market::git_url::parse_git_remote_parts(&source.git_url)
        {
            (owner, repo)
        } else {
            return self
                .markets_repo_root()
                .join("_incomplete")
                .join(&source.id);
        };
        self.markets_repo_root().join(owner).join(repo)
    }
}

pub fn expand_user_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let raw = path.to_string_lossy();
    if (raw == "~" || raw.starts_with("~/"))
        && let Some(home) = dirs::home_dir()
    {
        let suffix = raw.strip_prefix("~/").unwrap_or("");
        return home.join(suffix);
    }
    path.to_path_buf()
}
