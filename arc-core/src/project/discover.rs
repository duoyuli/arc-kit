use std::path::{Path, PathBuf};

/// Walk upward from `start`, returning the first `arc.toml` found.
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join("arc.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_config_in_current_directory() {
        let dir = tempdir().unwrap();
        let arc_toml = dir.path().join("arc.toml");
        fs::write(&arc_toml, "").unwrap();

        let found = find_project_config(dir.path());
        assert_eq!(found, Some(arc_toml));
    }

    #[test]
    fn finds_config_walking_up_from_nested_path() {
        let root = tempdir().unwrap();
        let nested = root.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();

        let arc_toml = root.path().join("arc.toml");
        fs::write(&arc_toml, "").unwrap();

        let found = find_project_config(&nested);
        assert_eq!(found, Some(arc_toml));
    }

    #[test]
    fn finds_nearest_config_in_monorepo() {
        let root = tempdir().unwrap();
        let sub = root.path().join("packages").join("my-service");
        fs::create_dir_all(&sub).unwrap();

        // Both root and sub have arc.toml; the nearest (sub) wins.
        fs::write(root.path().join("arc.toml"), "").unwrap();
        let sub_toml = sub.join("arc.toml");
        fs::write(&sub_toml, "").unwrap();

        let found = find_project_config(&sub);
        assert_eq!(found, Some(sub_toml));
    }

    #[test]
    fn returns_none_outside_any_project() {
        let dir = tempdir().unwrap();
        // No arc.toml anywhere in the temp dir subtree.
        let found = find_project_config(dir.path());
        assert_eq!(found, None);
    }
}
