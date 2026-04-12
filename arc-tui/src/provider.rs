use std::collections::HashMap;
use std::io;

use arc_core::agent::agent_spec;
use arc_core::provider::{ProviderInfo, supported_provider_agents};
use console::{Alignment, Key, Term, measure_text_width, pad_str, style, truncate_str};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderTab {
    agent: String,
    agent_display: String,
    provider_indexes: Vec<usize>,
    name_width: usize,
    has_active_provider: bool,
    default_row: usize,
}

struct CursorGuard<'a> {
    term: &'a Term,
}

impl Drop for CursorGuard<'_> {
    fn drop(&mut self) {
        let _ = self.term.show_cursor();
    }
}

pub fn select_provider(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> io::Result<Option<ProviderInfo>> {
    let tabs = build_provider_tabs(providers, active_providers);
    if tabs.is_empty() {
        return Ok(None);
    }

    let term = Term::stderr();
    let mut tab = default_tab_index(&tabs);
    let mut rows: Vec<usize> = tabs.iter().map(|tab| tab.default_row).collect();
    let mut scrolls = vec![0usize; tabs.len()];
    let mut prev_drawn = 0usize;

    term.hide_cursor()?;
    let _cursor_guard = CursorGuard { term: &term };

    loop {
        let (term_rows, cols) = term.size();
        let visible_rows = (term_rows as usize).saturating_sub(4).clamp(1, 12);
        let max_line_width = (cols as usize).saturating_sub(1).max(1);

        if prev_drawn > 0 {
            clear_drawn_block(&term, prev_drawn)?;
        }

        let current_tab = &tabs[tab];
        let current_rows = current_tab.provider_indexes.len();
        if current_rows == 0 {
            term.show_cursor()?;
            return Ok(None);
        }

        rows[tab] = rows[tab].min(current_rows - 1);
        let mut scroll = scrolls[tab];
        if rows[tab] < scroll {
            scroll = rows[tab];
        } else if rows[tab] >= scroll + visible_rows {
            scroll = rows[tab] + 1 - visible_rows;
        }
        scrolls[tab] = scroll;

        write_clamped_line(
            &term,
            format!("  {}", style("Provider").bold()),
            max_line_width,
        )?;
        write_clamped_line(&term, render_tab_line(&tabs, tab), max_line_width)?;

        let shown = current_rows.saturating_sub(scroll).min(visible_rows);
        for (pos, &provider_idx) in current_tab
            .provider_indexes
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible_rows)
        {
            let provider = &providers[provider_idx];
            let is_selected = pos == rows[tab];
            let is_active = active_providers
                .get(&current_tab.agent)
                .is_some_and(|name| name == &provider.name);
            write_clamped_line(
                &term,
                render_provider_line(provider, current_tab.name_width, is_active, is_selected),
                max_line_width,
            )?;
        }

        write_clamped_line(
            &term,
            render_hint_line(current_tab, current_rows, tabs.len() > 1),
            max_line_width,
        )?;

        prev_drawn = shown + 3;
        term.flush()?;

        match term.read_key()? {
            Key::Escape => {
                clear_drawn_block(&term, prev_drawn)?;
                term.show_cursor()?;
                return Ok(None);
            }
            Key::Enter => {
                let provider_idx = current_tab.provider_indexes[rows[tab]];
                clear_drawn_block(&term, prev_drawn)?;
                term.show_cursor()?;
                return Ok(Some(providers[provider_idx].clone()));
            }
            Key::ArrowUp => {
                rows[tab] = if rows[tab] == 0 {
                    current_rows - 1
                } else {
                    rows[tab] - 1
                };
            }
            Key::ArrowDown => {
                rows[tab] = (rows[tab] + 1) % current_rows;
            }
            Key::ArrowLeft | Key::BackTab => {
                tab = if tab == 0 { tabs.len() - 1 } else { tab - 1 };
            }
            Key::ArrowRight | Key::Tab => {
                tab = (tab + 1) % tabs.len();
            }
            _ => {}
        }
    }
}

fn build_provider_tabs(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> Vec<ProviderTab> {
    supported_provider_agents()
        .into_iter()
        .filter_map(|agent| {
            let provider_indexes: Vec<usize> = providers
                .iter()
                .enumerate()
                .filter(|(_, provider)| provider.agent == agent)
                .map(|(index, _)| index)
                .collect();
            if provider_indexes.is_empty() {
                return None;
            }
            let name_width = provider_indexes
                .iter()
                .map(|&index| measure_text_width(&providers[index].display_name))
                .max()
                .unwrap_or(0);
            let active_row = active_providers.get(agent).and_then(|active_name| {
                provider_indexes
                    .iter()
                    .position(|&index| providers[index].name == *active_name)
            });
            let default_row = active_row.unwrap_or(0);

            Some(ProviderTab {
                agent: agent.to_string(),
                agent_display: agent_spec(agent)
                    .map(|spec| spec.display_name.to_string())
                    .unwrap_or_else(|| agent.to_string()),
                provider_indexes,
                name_width,
                has_active_provider: active_row.is_some(),
                default_row,
            })
        })
        .collect()
}

fn default_tab_index(tabs: &[ProviderTab]) -> usize {
    tabs.iter()
        .position(|tab| tab.has_active_provider)
        .unwrap_or(0)
}

fn render_tab_line(tabs: &[ProviderTab], active_tab: usize) -> String {
    let labels: Vec<String> = tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let label = format!("[{}]", tab.agent_display);
            if index == active_tab {
                format!("{}", style(label).green().bold())
            } else {
                format!("{}", style(label).dim())
            }
        })
        .collect();
    format!("  {}", labels.join("  "))
}

fn render_provider_line(
    provider: &ProviderInfo,
    name_width: usize,
    is_active: bool,
    is_selected: bool,
) -> String {
    let marker = if is_active {
        format!("{}", style("✓").green())
    } else {
        " ".to_string()
    };

    let content = if provider.description.is_empty() {
        provider.display_name.clone()
    } else {
        let padded = pad_str(&provider.display_name, name_width, Alignment::Left, None);
        format!("{padded}  {}", provider.description)
    };

    if is_selected {
        format!(
            "  {} {} {}",
            style("❯").green(),
            marker,
            style(content).bold()
        )
    } else {
        format!("    {} {}", marker, style(content).dim())
    }
}

fn render_hint_line(tab: &ProviderTab, provider_count: usize, can_switch_tab: bool) -> String {
    let count_label = if provider_count == 1 {
        "1 provider".to_string()
    } else {
        format!("{provider_count} providers")
    };
    let nav = if can_switch_tab {
        "←→/tab switch agent  ↑↓ move  ↵ select  esc quit"
    } else {
        "↑↓ move  ↵ select  esc quit"
    };
    format!(
        "  {}",
        style(format!(
            "{}  ·  {}  ·  {}",
            tab.agent_display, count_label, nav
        ))
        .dim()
    )
}

fn truncation_tail(max_width: usize) -> &'static str {
    match max_width {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

fn clamp_line_width(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    truncate_str(line, max_width, truncation_tail(max_width)).into_owned()
}

fn write_clamped_line(term: &Term, line: String, max_width: usize) -> io::Result<()> {
    term.write_line(&clamp_line_width(&line, max_width))
}

fn clear_drawn_block(term: &Term, lines: usize) -> io::Result<()> {
    term.move_cursor_up(lines)?;
    for _ in 0..lines {
        term.clear_line()?;
        term.move_cursor_down(1)?;
    }
    term.move_cursor_up(lines)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arc_core::provider::{
        ClaudeProviderConfig, CodexProviderConfig, ProviderInfo, ProviderSettings,
    };
    use console::measure_text_width;

    use super::{
        build_provider_tabs, clamp_line_width, default_tab_index, render_provider_line,
        render_tab_line,
    };

    fn provider(agent: &str, name: &str, display_name: &str, description: &str) -> ProviderInfo {
        ProviderInfo {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            agent: agent.to_string(),
            settings: match agent {
                "claude" => ProviderSettings::Claude(ClaudeProviderConfig::default()),
                "codex" => ProviderSettings::Codex(CodexProviderConfig::default()),
                other => panic!("unexpected agent {other}"),
            },
        }
    }

    #[test]
    fn provider_tabs_follow_supported_agent_order() {
        let providers = vec![
            provider("codex", "official", "OpenAI", "official"),
            provider("claude", "proxy", "Mirror", "proxy"),
            provider("claude", "official", "Anthropic", "official"),
        ];

        let tabs = build_provider_tabs(&providers, &HashMap::new());

        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].agent, "claude");
        assert_eq!(tabs[0].provider_indexes, vec![1, 2]);
        assert_eq!(tabs[1].agent, "codex");
        assert_eq!(tabs[1].provider_indexes, vec![0]);
    }

    #[test]
    fn default_tab_prefers_agent_with_active_provider() {
        let providers = vec![
            provider("claude", "official", "Anthropic", ""),
            provider("codex", "proxy", "Mirror", ""),
            provider("codex", "official", "OpenAI", ""),
        ];
        let mut active = HashMap::new();
        active.insert("codex".to_string(), "proxy".to_string());

        let tabs = build_provider_tabs(&providers, &active);

        assert_eq!(tabs[1].default_row, 0);
        assert_eq!(default_tab_index(&tabs), 1);
    }

    #[test]
    fn render_provider_line_keeps_content_for_alignment() {
        let line = render_provider_line(
            &provider("claude", "official", "Anthropic", "official"),
            9,
            true,
            true,
        );

        assert!(line.contains("Anthropic"));
        assert!(line.contains("official"));
    }

    #[test]
    fn render_tab_line_marks_active_tab() {
        let providers = vec![
            provider("claude", "official", "Anthropic", ""),
            provider("codex", "official", "OpenAI", ""),
        ];

        let tabs = build_provider_tabs(&providers, &HashMap::new());
        let line = render_tab_line(&tabs, 1);

        assert!(line.contains("[Claude Code]"));
        assert!(line.contains("[Codex]"));
    }

    #[test]
    fn clamp_line_width_respects_terminal_width() {
        let clamped = clamp_line_width("0123456789", 5);

        assert_eq!(measure_text_width(&clamped), 5);
    }
}
