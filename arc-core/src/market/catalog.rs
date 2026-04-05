use serde_json::{Value, json};

use crate::market::document::{MarketsDocument, read_markets_document, write_markets_document};
use crate::models::{CatalogResource, ResourceInfo, ResourceKind};
use crate::paths::ArcPaths;

#[derive(Debug, Clone)]
pub struct CatalogManager {
    paths: ArcPaths,
}

impl CatalogManager {
    pub fn new(paths: ArcPaths) -> Self {
        Self { paths }
    }

    pub fn load(&self) -> MarketsDocument {
        read_markets_document(&self.paths)
    }

    pub fn rebuild(&self, resources: &[ResourceInfo]) -> std::io::Result<()> {
        let mut doc = self.load();
        doc.updated_at = crate::io::now_unix_secs();
        doc.resources = resources.iter().map(resource_to_value).collect();
        write_markets_document(&self.paths, &doc)
    }

    pub fn rebuild_source(
        &self,
        source_id: &str,
        resources: &[ResourceInfo],
    ) -> std::io::Result<()> {
        let mut doc = self.load();
        doc.updated_at = crate::io::now_unix_secs();
        doc.resources
            .retain(|value| value.get("source_id").and_then(Value::as_str) != Some(source_id));
        doc.resources
            .extend(resources.iter().map(resource_to_value));
        write_markets_document(&self.paths, &doc)
    }

    pub fn get_resources(&self, kind: Option<ResourceKind>) -> Vec<CatalogResource> {
        self.load()
            .resources
            .into_iter()
            .filter_map(value_to_catalog_resource)
            .filter(|resource| {
                kind.as_ref()
                    .is_none_or(|expected| &resource.kind == expected)
            })
            .collect()
    }

    pub fn get_resource(&self, resource_id: &str) -> Option<CatalogResource> {
        self.get_resources(None)
            .into_iter()
            .find(|resource| resource.id == resource_id)
    }

    pub fn remove_source_resources(&self, source_id: &str) -> std::io::Result<usize> {
        let mut doc = self.load();
        let before = doc.resources.len();
        doc.resources
            .retain(|value| value.get("source_id").and_then(Value::as_str) != Some(source_id));
        let removed = before.saturating_sub(doc.resources.len());
        write_markets_document(&self.paths, &doc)?;
        Ok(removed)
    }
}

fn resource_to_value(resource: &ResourceInfo) -> Value {
    json!({
        "id": resource.id,
        "kind": resource.kind.as_str(),
        "name": resource.name,
        "source_id": resource.source_id,
        "summary": resource.summary,
    })
}

fn value_to_catalog_resource(value: Value) -> Option<CatalogResource> {
    let kind = value.get("kind")?.as_str()?.parse().ok()?;
    Some(CatalogResource {
        id: value.get("id")?.as_str()?.to_string(),
        kind,
        name: value.get("name")?.as_str()?.to_string(),
        source_id: value.get("source_id")?.as_str()?.to_string(),
        summary: value
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        installed: value
            .get("installed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        installed_targets: value
            .get("installed_targets")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}
