use std::str::FromStr;

use arc_core::models::{ResourceKind, SkillOrigin};

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
