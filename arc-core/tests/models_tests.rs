use std::path::PathBuf;
use std::str::FromStr;

use arc_core::models::{ResourceKind, SkillEntry, SkillOrigin};

#[test]
fn resource_kind_roundtrip_string_values() {
    assert_eq!(ResourceKind::Skill.as_str(), "skill");
    assert_eq!(ResourceKind::ProviderProfile.as_str(), "provider_profile");
    assert_eq!(ResourceKind::SubAgent.as_str(), "sub_agent");

    assert_eq!(
        ResourceKind::from_str("skill").unwrap(),
        ResourceKind::Skill
    );
    assert_eq!(
        ResourceKind::from_str("provider_profile").unwrap(),
        ResourceKind::ProviderProfile
    );
    assert_eq!(
        ResourceKind::from_str("sub_agent").unwrap(),
        ResourceKind::SubAgent
    );
}

#[test]
fn resource_kind_rejects_unknown_value() {
    let err = ResourceKind::from_str("unknown").unwrap_err();
    assert!(err.contains("unsupported resource kind"));
}

#[test]
fn skill_origin_labels() {
    assert_eq!(SkillOrigin::Local.label(), "local");
    assert_eq!(SkillOrigin::BuiltIn.label(), "built-in");
    assert_eq!(
        SkillOrigin::Market {
            source_id: "x".to_string()
        }
        .label(),
        "market"
    );
}

#[test]
fn skill_origin_priority_local_is_highest() {
    assert!(
        SkillOrigin::Local.priority()
            < SkillOrigin::Market {
                source_id: String::new()
            }
            .priority()
    );
    assert!(
        SkillOrigin::Market {
            source_id: String::new()
        }
        .priority()
            < SkillOrigin::BuiltIn.priority()
    );
}

#[test]
fn skill_origin_display() {
    assert_eq!(format!("{}", SkillOrigin::Local), "local");
    assert_eq!(format!("{}", SkillOrigin::BuiltIn), "built-in");
}

fn sample_skill_entry(origin: SkillOrigin, market_repo: Option<&str>) -> SkillEntry {
    SkillEntry {
        name: "demo".to_string(),
        origin,
        summary: String::new(),
        source_path: PathBuf::from("."),
        installed_targets: Vec::new(),
        market_repo: market_repo.map(String::from),
    }
}

#[test]
fn skill_entry_origin_display_includes_market_repo() {
    let e = sample_skill_entry(
        SkillOrigin::Market {
            source_id: "my-slug".to_string(),
        },
        Some("owner/repo"),
    );
    assert_eq!(e.origin_display(), "market (owner/repo)");
    assert_eq!(e.origin_json(), "market:owner/repo");
}

#[test]
fn skill_entry_origin_display_market_falls_back_to_source_id() {
    let e = sample_skill_entry(
        SkillOrigin::Market {
            source_id: "my-slug".to_string(),
        },
        None,
    );
    assert_eq!(e.origin_display(), "market (my-slug)");
    assert_eq!(e.origin_json(), "market");
}

#[test]
fn skill_entry_origin_display_builtin_and_local() {
    assert_eq!(
        sample_skill_entry(SkillOrigin::BuiltIn, None).origin_display(),
        "built-in"
    );
    assert_eq!(
        sample_skill_entry(SkillOrigin::Local, None).origin_display(),
        "local"
    );
}
