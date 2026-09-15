use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agent::SkillInstallStrategy;
use crate::error::{ArcError, Result};
use crate::skill::tracking::fingerprint_path;

use super::paths::{inspect_owned, io_error, sync_directory, validate_destination};
use super::state::{
    InstallRecord, InstallScope, InstallStore, Journal, JournalPhase, operations_dir,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NodeState {
    device: u64,
    inode: u64,
    fingerprint: String,
}

impl NodeState {
    pub fn capture(path: &Path) -> Result<Option<Self>> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(io_error("cannot inspect installation entry", path, err)),
        };
        if !metadata.is_dir() && !metadata.is_file() && !metadata.file_type().is_symlink() {
            return Err(io_error(
                "unsupported installation entry",
                path,
                "expected a file, directory, or symlink",
            ));
        }
        Ok(Some(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            fingerprint: fingerprint_path(path)?,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InstallOperation {
    pub context: InstallRecord,
    pub before: Option<InstallRecord>,
    pub after: Option<InstallRecord>,
    pub metadata_only: bool,
    pub allow_unmanaged_remove: bool,
}

impl InstallOperation {
    pub fn put(after: InstallRecord, before: Option<InstallRecord>, metadata_only: bool) -> Self {
        Self {
            context: after.clone(),
            before,
            after: Some(after),
            metadata_only,
            allow_unmanaged_remove: false,
        }
    }

    pub fn remove(before: InstallRecord) -> Self {
        Self {
            context: before.clone(),
            before: Some(before),
            after: None,
            metadata_only: false,
            allow_unmanaged_remove: false,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailurePoint {
    AfterJournal,
    AfterStage,
    AfterBackup,
    AfterTarget,
    BeforeCommitError,
    AfterCommit,
}

impl InstallStore {
    pub fn execute(&mut self, operation: InstallOperation) -> Result<()> {
        validate_destination(&self.paths, &operation.context)?;
        let target = &operation.context.target_path;
        if self.ledger.record(target) != operation.before.as_ref() {
            return Err(io_error(
                "installation record changed",
                target,
                "retry the operation",
            ));
        }
        if let Some(before) = &operation.before {
            if !before.same_owner(&operation.context) {
                return Err(ArcError::new("installation ownership collision"));
            }
            inspect_owned(&self.paths, before)?;
        }
        let before_state = NodeState::capture(target)?;
        if let Some(before) = &operation.before {
            inspect_owned(&self.paths, before)?;
        }
        if before_state.is_some()
            && operation.before.is_none()
            && !operation.metadata_only
            && !(operation.allow_unmanaged_remove
                && operation.context.scope == InstallScope::Global
                && operation.after.is_none())
        {
            return Err(io_error(
                "unmanaged installation target",
                target,
                "explicit adoption is required",
            ));
        }
        if operation.metadata_only
            && let Some(after) = &operation.after
            && !inspect_owned(&self.paths, after)?
        {
            return Err(io_error("adoption target disappeared", target, "retry"));
        }
        if self
            .journals
            .iter()
            .any(|pending| pending.context.target_path == *target)
        {
            return Err(io_error(
                "installation has a pending operation",
                target,
                "recover it first",
            ));
        }

        // 首次操作也先持久化空台账，使后续“台账缺失”能明确表示历史丢失。
        self.save()?;
        fs::create_dir_all(operations_dir(&self.paths))
            .map_err(|err| ArcError::new(err.to_string()))?;
        let mut temporary = tempfile::Builder::new()
            .prefix("op-")
            .tempfile_in(operations_dir(&self.paths))
            .map_err(|err| ArcError::new(format!("cannot prepare installation journal: {err}")))?;
        let id = temporary
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let mut journal = Journal {
            schema_version: 1,
            workspace: target.parent().unwrap().join(format!(".arc-install-{id}")),
            id,
            before: operation.before,
            after: operation.after,
            context: operation.context,
            workspace_identity: None,
            before_state,
            after_state: None,
            phase: JournalPhase::Prepared,
            metadata_only: operation.metadata_only,
        };
        let bytes =
            serde_json::to_vec_pretty(&journal).map_err(|err| ArcError::new(err.to_string()))?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|err| ArcError::new(err.to_string()))?;
        temporary
            .persist_noclobber(self.journal_path(&journal.id))
            .map_err(|err| ArcError::new(err.to_string()))?;
        sync_directory(&operations_dir(&self.paths))?;
        self.journals.push(journal.clone());

        let result = self.apply_prepared(&mut journal);
        if let Err(error) = result {
            #[cfg(test)]
            if error.message.starts_with("simulated crash") {
                return Err(error);
            }
            let mut committed = false;
            let recovery = self.reload().and_then(|_| {
                committed = journal.phase == JournalPhase::Committed
                    || self.ledger.committed_operations.contains(&journal.id);
                self.recover_one(&journal)
            });
            return match recovery {
                Ok(()) if committed => Ok(()),
                Ok(()) => Err(error),
                Err(recovery) => Err(ArcError::new(format!(
                    "{}; pending recovery for {}: {}",
                    error.message,
                    journal.context.target_path.display(),
                    recovery.message
                ))),
            };
        }
        Ok(())
    }

    fn apply_prepared(&mut self, journal: &mut Journal) -> Result<()> {
        #[cfg(test)]
        self.checkpoint(FailurePoint::AfterJournal)?;
        let target = &journal.context.target_path;
        if !journal.metadata_only {
            fs::create_dir_all(target.parent().unwrap())
                .map_err(|err| io_error("cannot create install parent", target, err))?;
            validate_destination(&self.paths, &journal.context)?;
            fs::create_dir(&journal.workspace).map_err(|err| {
                io_error(
                    "cannot create installation workspace",
                    &journal.workspace,
                    err,
                )
            })?;
            let metadata = fs::symlink_metadata(&journal.workspace)
                .map_err(|err| ArcError::new(err.to_string()))?;
            journal.workspace_identity = Some((metadata.dev(), metadata.ino()));
            sync_directory(target.parent().unwrap())?;
            self.save_journal(journal)?;
            if let Some(after) = &mut journal.after {
                let stage = journal.workspace.join("new");
                match after.strategy {
                    SkillInstallStrategy::Symlink => symlink(&after.source_path, &stage)
                        .map_err(|err| io_error("cannot stage skill symlink", &stage, err))?,
                    SkillInstallStrategy::Copy => {
                        let source = fs::canonicalize(&after.source_path).map_err(|err| {
                            io_error("source unavailable", &after.source_path, err)
                        })?;
                        copy_tree(&source, &stage)?;
                    }
                }
                journal.after_state = NodeState::capture(&stage)?;
                if after.strategy == SkillInstallStrategy::Copy {
                    after.target_fingerprint = journal
                        .after_state
                        .as_ref()
                        .map(|state| state.fingerprint.clone());
                }
                if super::source_fingerprint(&after.source_path, after.strategy)?
                    != after.source_fingerprint
                {
                    return Err(io_error(
                        "skill source changed during installation",
                        &after.source_path,
                        "retry",
                    ));
                }
                self.save_journal(journal)?;
            }
            #[cfg(test)]
            self.checkpoint(FailurePoint::AfterStage)?;
            validate_destination(&self.paths, &journal.context)?;
            self.validate_workspace(journal)?;
            if NodeState::capture(target)? != journal.before_state {
                return Err(io_error(
                    "installation target changed",
                    target,
                    "refusing replacement",
                ));
            }
            if journal.before_state.is_some() {
                fs::rename(target, journal.workspace.join("old")).map_err(|err| {
                    io_error("cannot preserve previous installation", target, err)
                })?;
            }
            #[cfg(test)]
            self.checkpoint(FailurePoint::AfterBackup)?;
            if journal.after.is_some() {
                fs::rename(journal.workspace.join("new"), target)
                    .map_err(|err| io_error("cannot publish installation", target, err))?;
            }
            sync_directory(&journal.workspace)?;
            sync_directory(target.parent().unwrap())?;
        } else {
            journal.after_state = journal.before_state.clone();
            self.save_journal(journal)?;
        }
        #[cfg(test)]
        self.checkpoint(FailurePoint::AfterTarget)?;

        self.ledger
            .installs
            .retain(|record| record.target_path != *target);
        if let Some(after) = &journal.after {
            self.ledger.installs.push(after.clone());
        }
        self.ledger.committed_operations.insert(journal.id.clone());
        #[cfg(test)]
        self.checkpoint(FailurePoint::BeforeCommitError)?;
        self.save()?;
        #[cfg(test)]
        self.checkpoint(FailurePoint::AfterCommit)?;
        journal.phase = JournalPhase::Committed;
        self.save_journal(journal)?;
        self.finish_committed(journal)
    }

    pub fn recover(
        &mut self,
        scope: InstallScope,
        root: Option<&Path>,
        agents: &[String],
    ) -> Result<()> {
        let pending: Vec<_> = self
            .journals
            .iter()
            .filter(|journal| journal.selected(scope, root, agents))
            .cloned()
            .collect();
        for journal in pending {
            self.recover_one(&journal)?;
        }
        Ok(())
    }

    fn recover_one(&mut self, journal: &Journal) -> Result<()> {
        // 丢失提交凭据时保留目标与日志，不将已提交操作猜测为未提交操作。
        if self.ledger.recovery_required {
            return Err(ArcError::new(
                "install ledger history was lost; pending operations require review before recovery",
            ));
        }
        validate_destination(&self.paths, &journal.context)?;
        for record in journal.before.iter().chain(journal.after.iter()) {
            if !record.same_owner(&journal.context) {
                return Err(ArcError::new(
                    "pending installation ownership does not match its scope",
                ));
            }
            validate_destination(&self.paths, record)?;
        }
        let expected_workspace = journal
            .context
            .target_path
            .parent()
            .unwrap()
            .join(format!(".arc-install-{}", journal.id));
        if journal.workspace != expected_workspace {
            return Err(ArcError::new(
                "pending installation workspace is outside its target",
            ));
        }
        self.validate_workspace(journal)?;
        if journal.phase == JournalPhase::Committed
            || self.ledger.committed_operations.contains(&journal.id)
        {
            let mut journal = journal.clone();
            journal.phase = JournalPhase::Committed;
            self.save_journal(&journal)?;
            return self.finish_committed(&journal);
        }
        if self.ledger.record(&journal.context.target_path) != journal.before.as_ref() {
            return Err(ArcError::new(
                "pending installation record conflicts with current ledger",
            ));
        }
        if !journal.metadata_only {
            let target = &journal.context.target_path;
            let actual = NodeState::capture(target)?;
            let backup_path = journal.workspace.join("old");
            let backup = NodeState::capture(&backup_path)?;
            if backup.is_some() && backup != journal.before_state {
                return Err(io_error(
                    "installation backup was modified",
                    &backup_path,
                    "manual review required",
                ));
            }
            if actual == journal.before_state && backup.is_none() {
                // 目标尚未变更，或此前的回滚已经将其恢复。
            } else if actual.is_none()
                || (journal.after_state.is_some() && actual == journal.after_state)
            {
                if journal.before_state.is_some() && backup.is_none() {
                    return Err(io_error(
                        "installation backup is missing",
                        target,
                        "manual review required",
                    ));
                }
                if actual.is_some() {
                    remove_entry(target)?;
                }
                if backup.is_some() {
                    fs::rename(&backup_path, target).map_err(|err| {
                        io_error("cannot restore installation backup", target, err)
                    })?;
                }
                sync_directory(target.parent().unwrap())?;
                if journal.workspace.exists() {
                    sync_directory(&journal.workspace)?;
                }
            } else {
                return Err(io_error(
                    "pending installation target was replaced",
                    target,
                    "manual review required",
                ));
            }
        }
        self.clean_workspace(journal)?;
        self.remove_journal(journal)
    }

    fn finish_committed(&mut self, journal: &Journal) -> Result<()> {
        validate_destination(&self.paths, &journal.context)?;
        self.validate_workspace(journal)?;
        #[cfg(test)]
        if self.fail_cleanup {
            return Err(ArcError::new("simulated post-commit cleanup failure"));
        }
        if self.ledger.record(&journal.context.target_path) != journal.after.as_ref() {
            return Err(ArcError::new(
                "committed installation conflicts with current ledger",
            ));
        }
        let expected = if journal.after.is_some() {
            &journal.after_state
        } else {
            &None
        };
        if NodeState::capture(&journal.context.target_path)? != *expected {
            return Err(io_error(
                "committed target was modified",
                &journal.context.target_path,
                "cleanup remains pending",
            ));
        }
        self.clean_workspace(journal)?;
        self.ledger.committed_operations.remove(&journal.id);
        self.save()?;
        self.remove_journal(journal)
    }

    fn clean_workspace(&self, journal: &Journal) -> Result<()> {
        self.validate_workspace(journal)?;
        let metadata = match fs::symlink_metadata(&journal.workspace) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(io_error(
                    "cannot inspect installation workspace",
                    &journal.workspace,
                    err,
                ));
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ArcError::new("installation workspace was replaced"));
        }
        if let Some(identity) = journal.workspace_identity {
            if identity != (metadata.dev(), metadata.ino()) {
                return Err(ArcError::new("installation workspace identity changed"));
            }
        } else {
            return fs::remove_dir(&journal.workspace).map_err(|err| {
                io_error(
                    "unverified installation workspace is not empty",
                    &journal.workspace,
                    err,
                )
            });
        }
        for entry in
            fs::read_dir(&journal.workspace).map_err(|err| ArcError::new(err.to_string()))?
        {
            let entry = entry.map_err(|err| ArcError::new(err.to_string()))?;
            let name = entry.file_name();
            if name == "old" {
                if NodeState::capture(&entry.path())? != journal.before_state {
                    return Err(ArcError::new("installation backup changed; preserving it"));
                }
            } else if name != "new" {
                return Err(ArcError::new(
                    "installation workspace contains unexpected files",
                ));
            }
            remove_entry(&entry.path())?;
        }
        fs::remove_dir(&journal.workspace).map_err(|err| {
            io_error(
                "cannot remove installation workspace",
                &journal.workspace,
                err,
            )
        })?;
        sync_directory(journal.workspace.parent().unwrap())
    }

    fn validate_workspace(&self, journal: &Journal) -> Result<()> {
        let metadata = match fs::symlink_metadata(&journal.workspace) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(io_error(
                    "cannot inspect installation workspace",
                    &journal.workspace,
                    err,
                ));
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ArcError::new(
                "installation workspace was replaced; refusing recovery",
            ));
        }
        match journal.workspace_identity {
            Some(identity) if identity != (metadata.dev(), metadata.ino()) => {
                Err(ArcError::new("installation workspace identity changed"))
            }
            None => {
                let mut entries = fs::read_dir(&journal.workspace).map_err(|err| {
                    io_error(
                        "cannot inspect installation workspace",
                        &journal.workspace,
                        err,
                    )
                })?;
                if entries.next().is_some() {
                    Err(ArcError::new(
                        "unverified installation workspace is not empty",
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn remove_journal(&mut self, journal: &Journal) -> Result<()> {
        match fs::remove_file(self.journal_path(&journal.id)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(io_error(
                    "cannot finish installation journal",
                    &self.journal_path(&journal.id),
                    err,
                ));
            }
        }
        self.journals.retain(|pending| pending.id != journal.id);
        sync_directory(&operations_dir(&self.paths))?;
        Ok(())
    }

    #[cfg(test)]
    fn checkpoint(&mut self, point: FailurePoint) -> Result<()> {
        if self.fault == Some(point) {
            self.fault = None;
            if point == FailurePoint::BeforeCommitError {
                return Err(ArcError::new("simulated metadata commit failure"));
            }
            return Err(ArcError::new(format!("simulated crash at {point:?}")));
        }
        Ok(())
    }
}

pub(crate) fn remove_entry(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(io_error("cannot inspect removal target", path, err)),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
            .map_err(|err| io_error("cannot remove installed directory", path, err))
    } else {
        fs::remove_file(path).map_err(|err| io_error("cannot remove installed entry", path, err))
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|err| io_error("cannot read skill source", source, err))?;
    if metadata.file_type().is_symlink() {
        let link = fs::read_link(source)
            .map_err(|err| io_error("cannot read source symlink", source, err))?;
        symlink(link, destination)
            .map_err(|err| io_error("cannot copy source symlink", destination, err))?;
    } else if metadata.is_dir() {
        fs::create_dir(destination)
            .map_err(|err| io_error("cannot create staged copy", destination, err))?;
        for entry in fs::read_dir(source)
            .map_err(|err| io_error("cannot read source directory", source, err))?
        {
            let entry = entry.map_err(|err| io_error("cannot read source entry", source, err))?;
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        sync_directory(destination)?;
    } else if metadata.is_file() {
        fs::copy(source, destination)
            .map_err(|err| io_error("cannot stage skill file", destination, err))?;
        fs::File::open(destination)
            .and_then(|file| file.sync_all())
            .map_err(|err| io_error("cannot sync staged skill file", destination, err))?;
    } else {
        return Err(io_error(
            "unsupported skill source entry",
            source,
            "expected a file, directory, or symlink",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::ArcPaths;
    use crate::skill::install::{InstallRecord, InstallScope, normalize_root};

    fn fixture(strategy: SkillInstallStrategy) -> (tempfile::TempDir, ArcPaths, InstallRecord) {
        let temp = tempfile::tempdir().unwrap();
        let paths = ArcPaths::with_user_home(temp.path().join("home"));
        fs::create_dir_all(paths.user_home()).unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "before").unwrap();
        let record = InstallRecord::new(
            InstallScope::Project,
            Some(&project),
            "codex",
            "demo",
            &source,
            &project.join(".codex/skills/demo"),
            strategy,
        )
        .unwrap();
        (temp, paths, record)
    }

    #[test]
    fn crashes_before_and_after_commit_recover_to_the_correct_side() {
        for strategy in [SkillInstallStrategy::Symlink, SkillInstallStrategy::Copy] {
            for point in [
                FailurePoint::AfterJournal,
                FailurePoint::AfterStage,
                FailurePoint::AfterBackup,
                FailurePoint::AfterTarget,
                FailurePoint::AfterCommit,
            ] {
                let (_temp, paths, record) = fixture(strategy);
                let mut store = InstallStore::open(&paths).unwrap();
                store.fault = Some(point);
                assert!(
                    store
                        .execute(InstallOperation::put(record.clone(), None, false))
                        .is_err()
                );
                drop(store);
                let mut recovered = InstallStore::open(&paths).unwrap();
                recovered
                    .recover(
                        InstallScope::Project,
                        record.project_root.as_deref(),
                        &["codex".to_string()],
                    )
                    .unwrap();
                let committed = point == FailurePoint::AfterCommit;
                assert_eq!(
                    recovered.ledger.record(&record.target_path).is_some(),
                    committed,
                    "{strategy:?} {point:?}"
                );
                assert_eq!(
                    NodeState::capture(&record.target_path).unwrap().is_some(),
                    committed,
                    "{strategy:?} {point:?}"
                );
                assert!(recovered.journals.is_empty());
                if committed {
                    assert!(
                        inspect_owned(
                            &paths,
                            recovered.ledger.record(&record.target_path).unwrap()
                        )
                        .unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn failed_metadata_commit_restores_the_previous_copy_and_record() {
        let (_temp, paths, record) = fixture(SkillInstallStrategy::Copy);
        let mut store = InstallStore::open(&paths).unwrap();
        store
            .execute(InstallOperation::put(record.clone(), None, false))
            .unwrap();
        let before = store.ledger.record(&record.target_path).unwrap().clone();
        fs::write(record.source_path.join("SKILL.md"), "after").unwrap();
        let after = InstallRecord::new(
            record.scope,
            record.project_root.as_deref(),
            &record.agent,
            &record.skill,
            &record.source_path,
            &record.target_path,
            record.strategy,
        )
        .unwrap();
        store.fault = Some(FailurePoint::BeforeCommitError);
        assert!(
            store
                .execute(InstallOperation::put(after, Some(before.clone()), false))
                .is_err()
        );
        assert_eq!(store.ledger.record(&record.target_path), Some(&before));
        assert_eq!(
            fs::read_to_string(record.target_path.join("SKILL.md")).unwrap(),
            "before"
        );
        assert_eq!(
            fs::read_to_string(record.source_path.join("SKILL.md")).unwrap(),
            "after"
        );
        assert!(store.journals.is_empty());
    }

    #[test]
    fn post_commit_cleanup_failure_never_rolls_back_the_installed_copy() {
        let (_temp, paths, record) = fixture(SkillInstallStrategy::Copy);
        let mut store = InstallStore::open(&paths).unwrap();
        store
            .execute(InstallOperation::put(record.clone(), None, false))
            .unwrap();
        let before = store.ledger.record(&record.target_path).unwrap().clone();
        fs::write(record.source_path.join("SKILL.md"), "after").unwrap();
        let after = InstallRecord::new(
            record.scope,
            record.project_root.as_deref(),
            &record.agent,
            &record.skill,
            &record.source_path,
            &record.target_path,
            record.strategy,
        )
        .unwrap();
        store.fail_cleanup = true;
        assert!(
            store
                .execute(InstallOperation::put(after, Some(before.clone()), false))
                .is_err()
        );
        assert!(store.committed_pending(&record.target_path));
        assert_eq!(
            fs::read_to_string(record.target_path.join("SKILL.md")).unwrap(),
            "after"
        );
        assert_ne!(store.ledger.record(&record.target_path), Some(&before));
        drop(store);
        let mut recovered = InstallStore::open(&paths).unwrap();
        recovered
            .recover(
                InstallScope::Project,
                record.project_root.as_deref(),
                &["codex".to_string()],
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(record.target_path.join("SKILL.md")).unwrap(),
            "after"
        );
        assert!(recovered.journals.is_empty());
    }

    #[test]
    fn recovery_keeps_commit_receipts_when_another_project_commits() {
        let (temp, paths, first) = fixture(SkillInstallStrategy::Symlink);
        let mut store = InstallStore::open(&paths).unwrap();
        store.fault = Some(FailurePoint::AfterCommit);
        assert!(
            store
                .execute(InstallOperation::put(first.clone(), None, false))
                .is_err()
        );
        drop(store);
        let second_root = temp.path().join("second");
        fs::create_dir_all(&second_root).unwrap();
        let second = InstallRecord::new(
            InstallScope::Project,
            Some(&second_root),
            "codex",
            "demo",
            &first.source_path,
            &second_root.join(".codex/skills/demo"),
            first.strategy,
        )
        .unwrap();
        let mut other = InstallStore::open(&paths).unwrap();
        other
            .recover(
                InstallScope::Project,
                second.project_root.as_deref(),
                &["codex".to_string()],
            )
            .unwrap();
        other
            .execute(InstallOperation::put(second.clone(), None, false))
            .unwrap();
        assert_eq!(other.journals.len(), 1);
        drop(other);
        let mut recovered = InstallStore::open(&paths).unwrap();
        recovered
            .recover(
                InstallScope::Project,
                first.project_root.as_deref(),
                &["codex".to_string()],
            )
            .unwrap();
        assert_eq!(recovered.ledger.installs.len(), 2);
        assert!(first.target_path.is_symlink() && second.target_path.is_symlink());
    }

    #[test]
    fn pending_only_agent_is_recovered_by_cleanup_only_project_operation() {
        let (_temp, paths, record) = fixture(SkillInstallStrategy::Symlink);
        let mut store = InstallStore::open(&paths).unwrap();
        store.fault = Some(FailurePoint::AfterTarget);
        assert!(
            store
                .execute(InstallOperation::put(record.clone(), None, false))
                .is_err()
        );
        drop(store);
        let cache = crate::detect::DetectCache::from_map(std::collections::BTreeMap::new());
        let root = normalize_root(record.project_root.as_ref().unwrap()).unwrap();
        let report = crate::project::skills::reconcile_project_skills(
            &paths,
            &cache,
            &root,
            &[],
            &crate::project::skills::ProjectSkillOptions::default(),
            true,
            &[],
        );
        assert!(report.ok(), "{report:?}");
        assert!(!record.target_path.is_symlink());
        assert_eq!(
            report.items[0].status,
            crate::skill::install::InstallStatus::Recovered
        );
    }

    #[test]
    fn lost_ledger_does_not_turn_a_committed_journal_into_a_rollback() {
        // 缺失与损坏都必须保留已提交目标，且再次打开状态后仍拒绝猜测恢复。
        for missing in [false, true] {
            let (_temp, paths, record) = fixture(SkillInstallStrategy::Symlink);
            let mut store = InstallStore::open(&paths).unwrap();
            store.fault = Some(FailurePoint::AfterCommit);
            assert!(
                store
                    .execute(InstallOperation::put(record.clone(), None, false))
                    .is_err()
            );
            drop(store);
            if missing {
                fs::remove_file(paths.skill_tracking_file()).unwrap();
            } else {
                fs::write(paths.skill_tracking_file(), "corrupt").unwrap();
            }
            assert!(
                super::super::inspect_install_tracking(&paths)
                    .error
                    .is_some()
            );
            for _ in 0..2 {
                let mut store = InstallStore::open(&paths).unwrap();
                assert!(
                    store
                        .recover(
                            InstallScope::Project,
                            record.project_root.as_deref(),
                            &["codex".to_string()]
                        )
                        .is_err()
                );
                assert!(record.target_path.is_symlink());
                assert_eq!(store.journals.len(), 1);
            }
        }
    }

    #[test]
    fn dry_run_reports_pending_recovery_without_replaying_it() {
        let (_temp, paths, record) = fixture(SkillInstallStrategy::Symlink);
        let mut store = InstallStore::open(&paths).unwrap();
        store.fault = Some(FailurePoint::AfterTarget);
        assert!(
            store
                .execute(InstallOperation::put(record.clone(), None, false))
                .is_err()
        );
        let path = store.journal_path(&store.journals[0].id);
        let before = fs::read(&path).unwrap();
        let ledger_before = fs::read(paths.skill_tracking_file()).unwrap();
        drop(store);
        let options = crate::project::skills::ProjectSkillOptions {
            dry_run: true,
            ..Default::default()
        };
        let report = crate::project::skills::reconcile_project_skills(
            &paths,
            &crate::detect::DetectCache::from_map(std::collections::BTreeMap::new()),
            record.project_root.as_ref().unwrap(),
            &[],
            &options,
            true,
            &[],
        );
        assert!(!report.ok());
        assert!(record.target_path.is_symlink());
        assert_eq!(fs::read(path).unwrap(), before);
        assert_eq!(
            fs::read(paths.skill_tracking_file()).unwrap(),
            ledger_before
        );
    }

    #[test]
    fn recovery_refuses_a_workspace_replaced_with_an_escaping_symlink() {
        let (temp, paths, record) = fixture(SkillInstallStrategy::Symlink);
        let mut store = InstallStore::open(&paths).unwrap();
        store.fault = Some(FailurePoint::AfterTarget);
        assert!(
            store
                .execute(InstallOperation::put(record.clone(), None, false))
                .is_err()
        );
        let workspace = store.journals[0].workspace.clone();
        fs::remove_dir(&workspace).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("old"), "unrelated").unwrap();
        symlink(&outside, &workspace).unwrap();
        drop(store);
        let mut recovered = InstallStore::open(&paths).unwrap();
        assert!(
            recovered
                .recover(
                    InstallScope::Project,
                    record.project_root.as_deref(),
                    &["codex".to_string()]
                )
                .is_err()
        );
        assert!(record.target_path.is_symlink());
        assert_eq!(
            fs::read_to_string(outside.join("old")).unwrap(),
            "unrelated"
        );
    }
}
