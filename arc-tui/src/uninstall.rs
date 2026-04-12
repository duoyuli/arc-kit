use crate::agent::agent_display_name;
use crate::fuzzy::fuzzy_select_opt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallEntry {
    pub name: String,
    pub origin: String,
    pub installed_targets: Vec<String>,
}

pub fn run_capability_uninstall_wizard(
    entries: &[UninstallEntry],
) -> dialoguer::Result<Option<String>> {
    let name_width = entries
        .iter()
        .map(|entry| entry.name.len())
        .max()
        .unwrap_or(0);
    let display_labels: Vec<String> = entries
        .iter()
        .map(|entry| uninstall_label(entry, name_width))
        .collect();
    let search_corpus: Vec<String> = entries.iter().map(uninstall_search_corpus).collect();
    let selected = fuzzy_select_opt(&display_labels, &search_corpus)?;

    Ok(selected.and_then(|index| entries.get(index).map(|entry| entry.name.clone())))
}

fn uninstall_label(entry: &UninstallEntry, name_width: usize) -> String {
    if entry.installed_targets.is_empty() {
        return format!(
            "{:<width$}  [{}]  definition only",
            entry.name,
            entry.origin,
            width = name_width
        );
    }

    let targets = entry
        .installed_targets
        .iter()
        .map(|id| agent_display_name(id))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{:<width$}  [{}]  → {}",
        entry.name,
        entry.origin,
        targets,
        width = name_width
    )
}

fn uninstall_search_corpus(entry: &UninstallEntry) -> String {
    let mut out = format!("{} {}", entry.name, entry.origin);
    if entry.installed_targets.is_empty() {
        out.push_str(" definition only");
        return out;
    }

    out.push(' ');
    out.push_str(
        &entry
            .installed_targets
            .iter()
            .map(|id| agent_display_name(id))
            .collect::<Vec<_>>()
            .join(" "),
    );
    out
}
