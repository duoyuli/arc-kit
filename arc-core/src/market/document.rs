use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::io::{atomic_write_bytes, read_to_string_if_exists};
use crate::paths::ArcPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketsDocument {
    pub version: u32,
    pub updated_at: String,
    #[serde(default)]
    pub sources: BTreeMap<String, Value>,
    #[serde(default)]
    pub resources: Vec<Value>,
}

impl Default for MarketsDocument {
    fn default() -> Self {
        Self {
            version: 3,
            updated_at: String::new(),
            sources: BTreeMap::new(),
            resources: Vec::new(),
        }
    }
}

pub fn read_markets_document(paths: &ArcPaths) -> MarketsDocument {
    let path = paths.catalog();
    match read_to_string_if_exists(&path) {
        Ok(Some(content)) => serde_json::from_str(&content).unwrap_or_default(),
        _ => MarketsDocument::default(),
    }
}

pub fn write_markets_document(paths: &ArcPaths, document: &MarketsDocument) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    atomic_write_bytes(&paths.catalog(), &bytes)
}
