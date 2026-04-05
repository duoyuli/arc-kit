//! Merge skill entries from market, built-in, and local sources by name and origin priority.

use std::collections::BTreeMap;

use crate::models::SkillEntry;

/// Combine three scans into one map: same name keeps the entry with best (lowest) [`SkillOrigin::priority`].
pub fn merge_by_priority(
    market: Vec<SkillEntry>,
    builtin: Vec<SkillEntry>,
    local: Vec<SkillEntry>,
) -> BTreeMap<String, SkillEntry> {
    let mut map: BTreeMap<String, SkillEntry> = BTreeMap::new();

    for entry in market {
        map.entry(entry.name.clone()).or_insert(entry);
    }
    for entry in builtin {
        let e = map
            .entry(entry.name.clone())
            .or_insert_with(|| entry.clone());
        if entry.origin.priority() < e.origin.priority() {
            *e = entry;
        }
    }
    for entry in local {
        let e = map
            .entry(entry.name.clone())
            .or_insert_with(|| entry.clone());
        if entry.origin.priority() < e.origin.priority() {
            *e = entry;
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SkillOrigin;
    use std::path::PathBuf;

    fn entry(name: &str, origin: SkillOrigin) -> SkillEntry {
        SkillEntry {
            name: name.to_string(),
            origin,
            summary: String::new(),
            source_path: PathBuf::new(),
            installed_targets: Vec::new(),
            market_repo: None,
        }
    }

    #[test]
    fn local_beats_builtin_and_market() {
        let market = vec![entry(
            "a",
            SkillOrigin::Market {
                source_id: "m".to_string(),
            },
        )];
        let builtin = vec![entry("a", SkillOrigin::BuiltIn)];
        let local = vec![entry("a", SkillOrigin::Local)];
        let m = merge_by_priority(market, builtin, local);
        assert_eq!(m.get("a").unwrap().origin.label(), "local");
    }

    #[test]
    fn market_beats_builtin_when_no_local() {
        let market = vec![entry(
            "b",
            SkillOrigin::Market {
                source_id: "m".to_string(),
            },
        )];
        let builtin = vec![entry("b", SkillOrigin::BuiltIn)];
        let m = merge_by_priority(market, builtin, vec![]);
        assert_eq!(m.get("b").unwrap().origin.label(), "market");
    }

    #[test]
    fn names_are_sorted_by_btreemap_keys() {
        let m = merge_by_priority(
            vec![entry(
                "z",
                SkillOrigin::Market {
                    source_id: "m".to_string(),
                },
            )],
            vec![entry("a", SkillOrigin::BuiltIn)],
            vec![],
        );
        let keys: Vec<_> = m.keys().cloned().collect();
        assert_eq!(keys, vec!["a".to_string(), "z".to_string()]);
    }
}
