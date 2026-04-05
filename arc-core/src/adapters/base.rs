use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::models::ResourceKind;

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub name: String,
    pub detected: bool,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub name: String,
    pub kind: ResourceKind,
    pub path: PathBuf,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Change {
    pub action: String,
    pub target: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub ok: bool,
    pub applied: Vec<Change>,
    pub failed_change: Option<Change>,
    pub message: String,
}

impl ApplyResult {
    pub fn ok(message: impl Into<String>, applied: Vec<Change>) -> Self {
        Self {
            ok: true,
            applied,
            failed_change: None,
            message: message.into(),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            applied: Vec::new(),
            failed_change: None,
            message: message.into(),
        }
    }
}

pub trait ResourceAdapter {
    fn supports(&self, snapshot: &Snapshot, agent: &AgentContext) -> bool;
    fn apply(&self, snapshot: &Snapshot, agent: &AgentContext) -> ApplyResult;
    fn uninstall(&self, snapshot: &Snapshot, agent: &AgentContext) -> ApplyResult;
}
