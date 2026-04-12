use std::io;

use arc_core::mcp_registry::{McpCatalogEntry, McpEntryOrigin};

use crate::agent::agent_display_name;
use crate::fuzzy::{browse_list_with_label, fuzzy_select_opt_with_label};

struct McpLabelLayout {
    name_width: usize,
    origin_width: usize,
}

pub fn pick_mcp(entries: &[McpCatalogEntry]) -> io::Result<Option<String>> {
    let (display_labels, search_corpus) = browser_data(entries);
    let selected = fuzzy_select_opt_with_label(&display_labels, &search_corpus, "MCPs")?;

    Ok(selected.and_then(|index| {
        entries
            .get(index)
            .map(|entry| entry.definition.name.clone())
    }))
}

pub fn run_mcp_browser<F>(entries: &[McpCatalogEntry], render_detail: F) -> io::Result<()>
where
    F: Fn(&McpCatalogEntry),
{
    let (display_labels, search_corpus) = browser_data(entries);
    browse_list_with_label(
        entries,
        &display_labels,
        &search_corpus,
        "MCPs",
        render_detail,
    )
}

fn browser_data(entries: &[McpCatalogEntry]) -> (Vec<String>, Vec<String>) {
    let layout = label_layout(entries, &[]);
    let display_labels = entries
        .iter()
        .map(|entry| mcp_label(entry, &layout))
        .collect();
    let search_corpus = entries.iter().map(mcp_search_corpus).collect();

    (display_labels, search_corpus)
}

fn label_layout(entries: &[McpCatalogEntry], extra_names: &[String]) -> McpLabelLayout {
    let name_width = entries
        .iter()
        .map(|entry| entry.definition.name.len())
        .chain(extra_names.iter().map(String::len))
        .max()
        .unwrap_or(0);
    let origin_width = entries
        .iter()
        .map(|entry| format!("[{}]", origin_label(&entry.origin)).len())
        .max()
        .unwrap_or(0);

    McpLabelLayout {
        name_width,
        origin_width,
    }
}

fn mcp_label(entry: &McpCatalogEntry, layout: &McpLabelLayout) -> String {
    let origin = format!("[{}]", origin_label(&entry.origin));
    let base = format!(
        "{:<name_width$}  {:<origin_width$}",
        entry.definition.name,
        origin,
        name_width = layout.name_width,
        origin_width = layout.origin_width
    );
    match targets_label(entry.definition.targets.as_ref()) {
        Some(targets) => format!("{base}  → {targets}"),
        None => base,
    }
}

fn mcp_search_corpus(entry: &McpCatalogEntry) -> String {
    let mut out = format!("{} {}", entry.definition.name, origin_label(&entry.origin));
    if let Some(targets) = targets_label(entry.definition.targets.as_ref()) {
        out.push(' ');
        out.push_str(&targets);
    }
    if let Some(description) = &entry.definition.description {
        out.push(' ');
        out.push_str(description);
    }
    out
}

fn origin_label(origin: &McpEntryOrigin) -> &'static str {
    match origin {
        McpEntryOrigin::Builtin => "built-in",
        McpEntryOrigin::User => "user",
    }
}

fn targets_label(targets: Option<&Vec<String>>) -> Option<String> {
    targets.map(|targets| {
        targets
            .iter()
            .map(|id| agent_display_name(id))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use arc_core::capability::{McpDefinition, McpTransportType};
    use arc_core::mcp_registry::{McpCatalogEntry, McpEntryOrigin};

    use super::{browser_data, mcp_search_corpus};

    fn catalog_entry(
        name: &str,
        origin: McpEntryOrigin,
        targets: &[&str],
        transport: McpTransportType,
    ) -> McpCatalogEntry {
        McpCatalogEntry {
            definition: McpDefinition {
                name: name.to_string(),
                targets: if targets.is_empty() {
                    None
                } else {
                    Some(targets.iter().map(|target| (*target).to_string()).collect())
                },
                transport,
                command: None,
                args: Vec::new(),
                env: BTreeMap::new(),
                url: None,
                headers: BTreeMap::new(),
                description: None,
                cwd: None,
                env_file: None,
                timeout: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled: None,
                required: None,
                trust: None,
                include_tools: Vec::new(),
                exclude_tools: Vec::new(),
                oauth: None,
            },
            origin,
        }
    }

    #[test]
    fn browser_labels_hide_transport_and_align_origin_column() {
        let entries = vec![
            catalog_entry(
                "drawio",
                McpEntryOrigin::Builtin,
                &[],
                McpTransportType::Stdio,
            ),
            catalog_entry(
                "sequential-thinking",
                McpEntryOrigin::User,
                &["codex"],
                McpTransportType::StreamableHttp,
            ),
        ];

        let (labels, _) = browser_data(&entries);

        assert_eq!(labels[0], "drawio               [built-in]");
        assert_eq!(labels[1], "sequential-thinking  [user]      → Codex");
    }

    #[test]
    fn search_corpus_omits_transport_keywords() {
        let entry = catalog_entry(
            "zhipu-web-search",
            McpEntryOrigin::Builtin,
            &["codex"],
            McpTransportType::StreamableHttp,
        );

        let corpus = mcp_search_corpus(&entry);

        assert!(!corpus.contains("stdio"));
        assert!(!corpus.contains("streamable_http"));
        assert!(corpus.contains("built-in"));
    }
}
