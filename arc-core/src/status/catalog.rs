use super::*;

pub(super) fn collect_catalog(paths: &ArcPaths, global_skill_count: usize) -> CatalogStatus {
    let sources = MarketSourceRegistry::new(paths.clone()).list_all();
    let unhealthy_market_count = sources
        .iter()
        .filter(|source| source.status != "ok" && source.status != "indexed")
        .count();
    let resource_count = sources.iter().map(|source| source.resource_count).sum();

    CatalogStatus {
        market_count: sources.len(),
        resource_count,
        global_skill_count,
        unhealthy_market_count,
    }
}
