use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::market::document::{read_markets_document, write_markets_document};
use crate::market::git_url::parse_git_remote_parts;
use crate::market::index::MarketIndexStore;
use crate::models::MarketSource;
use crate::paths::ArcPaths;

#[derive(Debug, Clone)]
pub struct MarketSourceRegistry {
    paths: ArcPaths,
}

impl MarketSourceRegistry {
    pub fn new(paths: ArcPaths) -> Self {
        Self { paths }
    }

    pub fn load(&self) -> BTreeMap<String, MarketSource> {
        let mut sources = MarketIndexStore::new(self.paths.clone())
            .load_effective()
            .to_market_sources();
        sources.extend(self.load_local());
        sources
    }

    pub fn save(&self, sources: &BTreeMap<String, MarketSource>) -> std::io::Result<()> {
        let mut doc = read_markets_document(&self.paths);
        doc.updated_at = crate::io::now_unix_secs();
        doc.sources = sources
            .iter()
            .map(|(key, value)| (key.clone(), market_source_to_value(value)))
            .collect();
        write_markets_document(&self.paths, &doc)
    }

    pub fn add(&self, git_url: &str, parser: &str) -> std::io::Result<MarketSource> {
        let source_id = self.generate_slug(git_url);
        let (owner, repo) = parse_git_remote_parts(git_url).unwrap_or_default();
        let source = MarketSource {
            id: source_id.clone(),
            git_url: git_url.to_string(),
            parser: parser.to_string(),
            owner,
            repo,
            status: "ok".to_string(),
            last_updated_at: String::new(),
            resource_count: 0,
        };
        let mut sources = self.load_local();
        sources.insert(source_id, source.clone());
        self.save(&sources)?;
        Ok(source)
    }

    pub fn remove(&self, source_id: &str) -> std::io::Result<bool> {
        let mut sources = self.load_local();
        let removed = sources.remove(source_id).is_some();
        self.save(&sources)?;
        Ok(removed)
    }

    pub fn get(&self, source_id: &str) -> Option<MarketSource> {
        self.load().remove(source_id)
    }

    pub fn list_all(&self) -> Vec<MarketSource> {
        self.load().into_values().collect()
    }

    pub fn update_source(
        &self,
        source_id: &str,
        patch: MarketSourcePatch,
    ) -> std::io::Result<Option<MarketSource>> {
        let Some(mut source) = self.get(source_id) else {
            return Ok(None);
        };
        if let Some(git_url) = patch.git_url {
            source.git_url = git_url;
        }
        if let Some(parser) = patch.parser {
            source.parser = parser;
        }
        if let Some(owner) = patch.owner {
            source.owner = owner;
        }
        if let Some(repo) = patch.repo {
            source.repo = repo;
        }
        if let Some(status) = patch.status {
            source.status = status;
        }
        if let Some(last_updated_at) = patch.last_updated_at {
            source.last_updated_at = last_updated_at;
        }
        if let Some(resource_count) = patch.resource_count {
            source.resource_count = resource_count;
        }
        let updated = source;
        let mut sources = self.load_local();
        sources.insert(source_id.to_string(), updated.clone());
        self.save(&sources)?;
        Ok(Some(updated))
    }

    pub fn is_builtin(&self, source_id: &str) -> bool {
        MarketIndexStore::new(self.paths.clone())
            .load_effective()
            .to_market_sources()
            .contains_key(source_id)
    }

    pub fn generate_slug(&self, git_url: &str) -> String {
        crate::market::git_url::slug_from_git_url(git_url)
    }

    fn load_local(&self) -> BTreeMap<String, MarketSource> {
        let doc = read_markets_document(&self.paths);
        doc.sources
            .into_iter()
            .filter_map(|(source_id, value)| {
                value_to_market_source(&source_id, value).map(|source| (source_id, source))
            })
            .collect()
    }
}

#[derive(Debug, Default, Clone)]
pub struct MarketSourcePatch {
    pub git_url: Option<String>,
    pub parser: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub status: Option<String>,
    pub last_updated_at: Option<String>,
    pub resource_count: Option<usize>,
}

fn value_to_market_source(source_id: &str, value: Value) -> Option<MarketSource> {
    let mut source: MarketSource = serde_json::from_value(value).ok()?;
    if source.id.is_empty() {
        source.id = source_id.to_string();
    }
    if (source.owner.is_empty() || source.repo.is_empty())
        && let Some((owner, repo)) = parse_git_remote_parts(&source.git_url)
    {
        source.owner = owner;
        source.repo = repo;
    }
    Some(source)
}

fn market_source_to_value(source: &MarketSource) -> Value {
    json!({
        "id": source.id,
        "git_url": source.git_url,
        "parser": source.parser,
        "owner": source.owner,
        "repo": source.repo,
        "status": source.status,
        "last_updated_at": source.last_updated_at,
        "resource_count": source.resource_count,
    })
}
