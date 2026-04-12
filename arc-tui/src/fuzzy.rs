use std::io;

use console::{Key, Term, style, truncate_str};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

use crate::validate_required_multi_selection;

enum SelectMode {
    Single,
    Multi { defaults: Vec<bool> },
}

enum SelectResult {
    Cancelled,
    One(usize),
    Many(Vec<usize>),
}

fn truncation_tail(max_width: usize) -> &'static str {
    match max_width {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

fn count_summary(total: usize, filtered: usize, item_label: &str) -> String {
    if filtered == total {
        format!("{total} {item_label}")
    } else {
        format!("{filtered} of {total} {item_label}")
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

fn toggle_multi_checked(checked: &mut [bool], filtered: &[usize], sel: usize) {
    if let Some(&item_idx) = filtered.get(sel) {
        checked[item_idx] = !checked[item_idx];
    }
}

fn fuzzy_select_engine(
    display_labels: &[String],
    search_corpus: &[String],
    mode: SelectMode,
    item_label: &str,
) -> io::Result<SelectResult> {
    fuzzy_select_engine_at(display_labels, search_corpus, mode, item_label, 0)
}

fn fuzzy_select_engine_at(
    display_labels: &[String],
    search_corpus: &[String],
    mode: SelectMode,
    item_label: &str,
    initial_sel: usize,
) -> io::Result<SelectResult> {
    let term = Term::stderr();
    let matcher = SkimMatcherV2::default();
    let mut search = String::new();
    let mut sel: usize = initial_sel;
    let mut starting_row: usize = 0;
    let mut prev_drawn: usize = 0;
    let mut warning: Option<&'static str> = None;
    let mut checked: Vec<bool> = match &mode {
        SelectMode::Single => vec![false; display_labels.len()],
        SelectMode::Multi { defaults } => defaults.clone(),
    };
    let is_multi = matches!(mode, SelectMode::Multi { .. });

    term.hide_cursor()?;

    loop {
        let (rows, cols) = term.size();
        // Reserve 3 lines: search prompt + hint line + safety margin.
        let visible_rows = (rows as usize).saturating_sub(3).clamp(1, 15);
        // Keep one spare column so write_line never triggers terminal auto-wrap.
        let max_line_width = (cols as usize).saturating_sub(1).max(1);

        if prev_drawn > 0 {
            // Move cursor up and clear lines individually for more reliable clearing
            term.move_cursor_up(prev_drawn)?;
            for _ in 0..prev_drawn {
                term.clear_line()?;
                term.move_cursor_down(1)?;
            }
            term.move_cursor_up(prev_drawn)?;
        }

        let filtered: Vec<usize> = if search.is_empty() {
            (0..display_labels.len()).collect()
        } else {
            let mut scored: Vec<(usize, i64)> = search_corpus
                .iter()
                .enumerate()
                .filter_map(|(i, corpus)| matcher.fuzzy_match(corpus, &search).map(|s| (i, s)))
                .collect();
            scored.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            scored.into_iter().map(|(i, _)| i).collect()
        };

        if filtered.is_empty() {
            sel = 0;
            starting_row = 0;
        } else {
            sel = sel.min(filtered.len() - 1);
            if sel < starting_row {
                starting_row = sel;
            } else if sel >= starting_row + visible_rows {
                starting_row = sel + 1 - visible_rows;
            }
        }

        if search.is_empty() {
            write_clamped_line(
                &term,
                format!(
                    "  {} {}",
                    style("/").dim(),
                    style("type to filter...").dim()
                ),
                max_line_width,
            )?;
        } else {
            write_clamped_line(
                &term,
                format!("  {} {}", style("/").cyan().bold(), search),
                max_line_width,
            )?;
        }

        let num_shown = filtered
            .len()
            .saturating_sub(starting_row)
            .min(visible_rows);
        for (pos, &item_idx) in filtered
            .iter()
            .enumerate()
            .skip(starting_row)
            .take(visible_rows)
        {
            let display = &display_labels[item_idx];
            let is_selected = pos == sel;
            let is_checked = checked[item_idx];

            let check = if is_multi {
                if is_checked {
                    format!("{} ", style("☑").green())
                } else {
                    format!("{} ", style("☐").dim())
                }
            } else {
                String::new()
            };

            if is_selected {
                write_clamped_line(
                    &term,
                    format!("  {} {check}{}", style("❯").green(), style(display).bold()),
                    max_line_width,
                )?;
            } else {
                write_clamped_line(
                    &term,
                    format!("    {check}{}", style(display).dim()),
                    max_line_width,
                )?;
            }
        }

        let selected_count = checked.iter().filter(|&&c| c).count();
        let count_str = count_summary(display_labels.len(), filtered.len(), item_label);
        let hint = if is_multi {
            let base = format!(
                "{}  ·  {} selected  ·  ↑↓ move  space toggle  ↵ confirm  esc quit",
                count_str, selected_count
            );
            if let Some(message) = warning {
                format!("{message}  ·  {base}")
            } else {
                base
            }
        } else {
            format!("{}  ·  ↑↓ move  ↵ select  esc quit", count_str)
        };
        write_clamped_line(&term, format!("  {}", style(hint).dim()), max_line_width)?;

        prev_drawn = num_shown + 2;
        term.flush()?;

        match term.read_key()? {
            Key::Escape => {
                term.move_cursor_up(prev_drawn)?;
                for _ in 0..prev_drawn {
                    term.clear_line()?;
                    term.move_cursor_down(1)?;
                }
                term.show_cursor()?;
                return Ok(SelectResult::Cancelled);
            }
            Key::Enter => {
                if is_multi {
                    let indices: Vec<usize> = checked
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| **c)
                        .map(|(i, _)| i)
                        .collect();
                    if let Err(message) = validate_required_multi_selection(&indices) {
                        warning = Some(message);
                        continue;
                    }
                    term.move_cursor_up(prev_drawn)?;
                    for _ in 0..prev_drawn {
                        term.clear_line()?;
                        term.move_cursor_down(1)?;
                    }
                    term.show_cursor()?;
                    return Ok(SelectResult::Many(indices));
                }
                term.move_cursor_up(prev_drawn)?;
                for _ in 0..prev_drawn {
                    term.clear_line()?;
                    term.move_cursor_down(1)?;
                }
                term.show_cursor()?;
                return Ok(match filtered.get(sel).copied() {
                    Some(i) => SelectResult::One(i),
                    None => SelectResult::Cancelled,
                });
            }
            Key::Char(' ') if is_multi => {
                warning = None;
                toggle_multi_checked(&mut checked, &filtered, sel);
            }
            Key::ArrowUp | Key::BackTab => {
                warning = None;
                if !filtered.is_empty() {
                    sel = if sel == 0 {
                        filtered.len() - 1
                    } else {
                        sel - 1
                    };
                }
            }
            Key::ArrowDown | Key::Tab => {
                warning = None;
                if !filtered.is_empty() {
                    sel = (sel + 1) % filtered.len();
                }
            }
            Key::Backspace => {
                warning = None;
                search.pop();
                sel = 0;
                starting_row = 0;
            }
            Key::Char(c) if !c.is_ascii_control() => {
                warning = None;
                search.push(c);
                sel = 0;
                starting_row = 0;
            }
            _ => {}
        }
    }
}

pub(crate) fn fuzzy_select_opt(
    display_labels: &[String],
    search_corpus: &[String],
) -> io::Result<Option<usize>> {
    fuzzy_select_opt_with_label(display_labels, search_corpus, "skills")
}

pub(crate) fn fuzzy_select_opt_with_label(
    display_labels: &[String],
    search_corpus: &[String],
    item_label: &str,
) -> io::Result<Option<usize>> {
    match fuzzy_select_engine(
        display_labels,
        search_corpus,
        SelectMode::Single,
        item_label,
    )? {
        SelectResult::One(i) => Ok(Some(i)),
        _ => Ok(None),
    }
}

pub(crate) fn fuzzy_multi_select(
    display_labels: &[String],
    search_corpus: &[String],
    defaults: &[bool],
) -> io::Result<Option<Vec<usize>>> {
    fuzzy_multi_select_with_label(display_labels, search_corpus, defaults, "skills")
}

pub(crate) fn fuzzy_multi_select_with_label(
    display_labels: &[String],
    search_corpus: &[String],
    defaults: &[bool],
    item_label: &str,
) -> io::Result<Option<Vec<usize>>> {
    match fuzzy_select_engine(
        display_labels,
        search_corpus,
        SelectMode::Multi {
            defaults: defaults.to_vec(),
        },
        item_label,
    )? {
        SelectResult::Many(indices) => Ok(Some(indices)),
        _ => Ok(None),
    }
}

pub(crate) fn browse_list<T, F>(
    items: &[T],
    display_labels: &[String],
    search_corpus: &[String],
    render_detail: F,
) -> io::Result<()>
where
    F: Fn(&T),
{
    browse_list_with_label(
        items,
        display_labels,
        search_corpus,
        "skills",
        render_detail,
    )
}

pub(crate) fn browse_list_with_label<T, F>(
    items: &[T],
    display_labels: &[String],
    search_corpus: &[String],
    item_label: &str,
    render_detail: F,
) -> io::Result<()>
where
    F: Fn(&T),
{
    let term = Term::stdout();
    let mut cursor: usize = 0;
    loop {
        match fuzzy_select_engine_at(
            display_labels,
            search_corpus,
            SelectMode::Single,
            item_label,
            cursor,
        )? {
            SelectResult::One(i) => {
                cursor = i;
                if let Some(item) = items.get(i) {
                    term.clear_screen()?;
                    render_detail(item);
                    println!("  {}", style("esc back  q quit").dim());
                    loop {
                        match term.read_key()? {
                            Key::Escape => {
                                term.clear_screen()?;
                                break;
                            }
                            Key::Char('q') => {
                                term.clear_screen()?;
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }
            SelectResult::Cancelled | SelectResult::Many(_) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_line_width, count_summary, toggle_multi_checked};
    use console::{measure_text_width, style};

    #[test]
    fn clamp_line_width_keeps_styled_lines_within_terminal_width() {
        let line = format!(
            "  {} {}",
            style("❯").green(),
            style("baoyu-cover-image  market (jimliu/baoyu-skills)  → Claude Code, Codex").bold()
        );

        let clamped = clamp_line_width(&line, 24);

        assert!(measure_text_width(&clamped) <= 24);
    }

    #[test]
    fn clamp_line_width_uses_short_tail_for_tiny_widths() {
        let clamped = clamp_line_width("abcdef", 2);

        assert_eq!(measure_text_width(&clamped), 2);
    }

    #[test]
    fn toggle_multi_checked_only_flips_current_item() {
        let mut checked = vec![false, true, false];
        let filtered = vec![2, 0];

        toggle_multi_checked(&mut checked, &filtered, 1);

        assert_eq!(checked, vec![true, true, false]);
    }

    #[test]
    fn count_summary_uses_custom_item_label() {
        assert_eq!(count_summary(4, 4, "MCPs"), "4 MCPs");
        assert_eq!(count_summary(4, 2, "MCPs"), "2 of 4 MCPs");
    }
}
