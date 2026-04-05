use std::io;

use console::{Key, Term, style};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

enum SelectMode {
    Single,
    Multi { defaults: Vec<bool> },
}

enum SelectResult {
    Cancelled,
    One(usize),
    Many(Vec<usize>),
}

fn fuzzy_select_engine(
    display_labels: &[String],
    search_corpus: &[String],
    mode: SelectMode,
) -> io::Result<SelectResult> {
    fuzzy_select_engine_at(display_labels, search_corpus, mode, 0)
}

fn fuzzy_select_engine_at(
    display_labels: &[String],
    search_corpus: &[String],
    mode: SelectMode,
    initial_sel: usize,
) -> io::Result<SelectResult> {
    let term = Term::stderr();
    let matcher = SkimMatcherV2::default();
    let mut search = String::new();
    let mut sel: usize = initial_sel;
    let mut starting_row: usize = 0;
    let mut prev_drawn: usize = 0;
    let mut checked: Vec<bool> = match &mode {
        SelectMode::Single => vec![false; display_labels.len()],
        SelectMode::Multi { defaults } => defaults.clone(),
    };
    let is_multi = matches!(mode, SelectMode::Multi { .. });

    let visible_rows = (term.size().0 as usize).saturating_sub(4).clamp(3, 15);

    term.hide_cursor()?;

    loop {
        if prev_drawn > 0 {
            term.clear_last_lines(prev_drawn)?;
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
            term.write_line(&format!(
                "  {} {}",
                style("/").dim(),
                style("type to filter...").dim()
            ))?;
        } else {
            term.write_line(&format!("  {} {}", style("/").cyan().bold(), search))?;
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
                term.write_line(&format!(
                    "  {} {check}{}",
                    style("❯").green(),
                    style(display).bold()
                ))?;
            } else {
                term.write_line(&format!("    {check}{}", style(display).dim()))?;
            }
        }

        let selected_count = checked.iter().filter(|&&c| c).count();
        let count_str = if search.is_empty() {
            format!("{} skills", display_labels.len())
        } else {
            format!("{} of {}", filtered.len(), display_labels.len())
        };
        let hint = if is_multi {
            format!(
                "{}  ·  {} selected  ·  ↑↓ move  space toggle  ↵ confirm  esc quit",
                count_str, selected_count
            )
        } else {
            format!("{}  ·  ↑↓ move  ↵ select  esc quit", count_str)
        };
        term.write_line(&format!("  {}", style(hint).dim()))?;

        prev_drawn = num_shown + 2;
        term.flush()?;

        match term.read_key()? {
            Key::Escape => {
                term.clear_last_lines(prev_drawn)?;
                term.show_cursor()?;
                return Ok(SelectResult::Cancelled);
            }
            Key::Enter => {
                term.clear_last_lines(prev_drawn)?;
                term.show_cursor()?;
                if is_multi {
                    let indices: Vec<usize> = checked
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| **c)
                        .map(|(i, _)| i)
                        .collect();
                    return Ok(SelectResult::Many(indices));
                }
                return Ok(match filtered.get(sel).copied() {
                    Some(i) => SelectResult::One(i),
                    None => SelectResult::Cancelled,
                });
            }
            Key::Char(' ') if is_multi => {
                if let Some(&item_idx) = filtered.get(sel) {
                    checked[item_idx] = !checked[item_idx];
                    if !filtered.is_empty() {
                        sel = (sel + 1) % filtered.len();
                    }
                }
            }
            Key::ArrowUp | Key::BackTab => {
                if !filtered.is_empty() {
                    sel = if sel == 0 {
                        filtered.len() - 1
                    } else {
                        sel - 1
                    };
                }
            }
            Key::ArrowDown | Key::Tab => {
                if !filtered.is_empty() {
                    sel = (sel + 1) % filtered.len();
                }
            }
            Key::Backspace => {
                search.pop();
                sel = 0;
                starting_row = 0;
            }
            Key::Char(c) if !c.is_ascii_control() => {
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
    match fuzzy_select_engine(display_labels, search_corpus, SelectMode::Single)? {
        SelectResult::One(i) => Ok(Some(i)),
        _ => Ok(None),
    }
}

pub(crate) fn fuzzy_multi_select(
    display_labels: &[String],
    search_corpus: &[String],
    defaults: &[bool],
) -> io::Result<Option<Vec<usize>>> {
    match fuzzy_select_engine(
        display_labels,
        search_corpus,
        SelectMode::Multi {
            defaults: defaults.to_vec(),
        },
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
    let term = Term::stdout();
    let mut cursor: usize = 0;
    loop {
        match fuzzy_select_engine_at(display_labels, search_corpus, SelectMode::Single, cursor)? {
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
