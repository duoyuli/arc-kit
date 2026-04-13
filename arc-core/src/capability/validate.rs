use super::*;

static RESOURCE_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9-_]{0,63}$").expect("valid resource regex"));
static ENV_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\$\{[A-Z0-9_]+\}$").expect("valid env regex"));
static ENV_DOLLAR_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\$[A-Z0-9_]+$").expect("valid env dollar regex"));
static ENV_PROVIDER_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\$\{env:[A-Z0-9_]+\}$").expect("valid env provider regex"));
static ENV_OPENCODE_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\{env:[A-Z0-9_]+\}$").expect("valid opencode env regex"));
static AUTH_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(Bearer|Basic)\s+\$\{[A-Z0-9_]+\}$").expect("valid auth env regex"));
static AUTH_DOLLAR_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(Bearer|Basic)\s+\$[A-Z0-9_]+$").expect("valid auth dollar regex"));
static AUTH_PROVIDER_PLACEHOLDER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(Bearer|Basic)\s+\$\{env:[A-Z0-9_]+\}$").expect("valid auth provider regex")
});
static AUTH_OPENCODE_PLACEHOLDER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(Bearer|Basic)\s+\{env:[A-Z0-9_]+\}$").expect("valid auth opencode regex")
});

pub fn validate_mcp_definition(definition: &mut McpDefinition) -> Result<()> {
    normalize_targets(&mut definition.targets);
    validate_resource_name(&definition.name, "mcp")?;
    validate_declared_targets(definition.targets.as_ref(), &ResourceKind::Mcp)?;
    match definition.transport {
        McpTransportType::Stdio => {
            if definition.command.as_deref().unwrap_or("").is_empty() {
                return Err(ArcError::new(format!(
                    "mcp '{}' requires command for stdio transport",
                    definition.name
                )));
            }
            if definition.url.is_some() {
                return Err(ArcError::new(format!(
                    "mcp '{}' cannot set url for stdio transport",
                    definition.name
                )));
            }
        }
        McpTransportType::Sse | McpTransportType::StreamableHttp => {
            if definition.url.as_deref().unwrap_or("").is_empty() {
                return Err(ArcError::new(format!(
                    "mcp '{}' requires url for remote transport",
                    definition.name
                )));
            }
            if definition.command.is_some() {
                return Err(ArcError::new(format!(
                    "mcp '{}' cannot set command for remote transport",
                    definition.name
                )));
            }
        }
    }
    validate_secret_map(&definition.env)?;
    validate_secret_map(&definition.headers)?;
    Ok(())
}

pub fn validate_subagent_definition(
    definition: &mut SubagentDefinition,
    source_scope: SourceScope,
    base_dir: &Path,
) -> Result<PathBuf> {
    normalize_targets(&mut definition.targets);
    validate_resource_name(&definition.name, "subagent")?;
    validate_declared_targets(definition.targets.as_ref(), &ResourceKind::SubAgent)?;
    let prompt_path = if source_scope == SourceScope::Global {
        expand_user_path(&definition.prompt_file)
    } else {
        base_dir.join(&definition.prompt_file)
    };
    if !prompt_path.is_file() {
        return Err(ArcError::new(format!(
            "subagent '{}' prompt_file not found: {}",
            definition.name,
            prompt_path.display()
        )));
    }
    Ok(prompt_path)
}

pub fn validate_subagent_targets(
    cache: &DetectCache,
    definition: &SubagentDefinition,
) -> Result<Vec<String>> {
    let targets = resolve_declared_targets(cache, definition.targets.as_ref());
    if targets.iter().any(|agent| agent == "codex")
        && definition
            .description
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    {
        return Err(ArcError::new(
            "description_required: codex subagents require a non-empty description",
        ));
    }
    Ok(targets)
}

pub fn resolve_declared_targets(
    cache: &DetectCache,
    declared_targets: Option<&Vec<String>>,
) -> Vec<String> {
    if let Some(targets) = declared_targets {
        return dedupe_targets(targets.clone());
    }
    cache.detected_agents().keys().cloned().collect()
}

pub fn is_shadowed(name: &str, project_names: &BTreeSet<String>) -> bool {
    project_names.contains(name)
}

pub(crate) fn validate_resource_name(name: &str, kind: &str) -> Result<()> {
    if RESOURCE_NAME_RE.is_match(name) {
        return Ok(());
    }
    Err(ArcError::new(format!(
        "{kind} name '{name}' must match ^[a-z0-9][a-z0-9-_]{{0,63}}$"
    )))
}

pub(crate) fn validate_declared_targets(
    targets: Option<&Vec<String>>,
    kind: &ResourceKind,
) -> Result<()> {
    let Some(targets) = targets else {
        return Ok(());
    };
    let supported = ordered_agent_ids_for_resource_kind(kind);
    for target in targets {
        if supported.iter().any(|item| item == target) {
            continue;
        }
        return Err(ArcError::with_hint(
            format!("unsupported target agent '{target}' for {}", kind.as_str()),
            format!("Available: {}", supported.join(", ")),
        ));
    }
    Ok(())
}

pub(crate) fn normalize_targets(targets: &mut Option<Vec<String>>) {
    let Some(items) = targets.as_mut() else {
        return;
    };
    let mut seen = BTreeSet::new();
    items.retain(|item| !item.is_empty() && seen.insert(item.clone()));
    if items.is_empty() {
        *targets = None;
    }
}

pub(crate) fn dedupe_targets(targets: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    targets
        .into_iter()
        .filter(|item| !item.is_empty() && seen.insert(item.clone()))
        .collect()
}

pub(crate) fn supports_mcp_transport(
    support: McpTransportSupport,
    transport: McpTransportType,
) -> bool {
    match transport {
        McpTransportType::Stdio => support.supports_stdio,
        McpTransportType::Sse => support.supports_sse,
        McpTransportType::StreamableHttp => support.supports_streamable_http,
    }
}

fn validate_secret_map(map: &BTreeMap<String, String>) -> Result<()> {
    for (key, value) in map {
        if is_secret_key(key) && !is_secret_placeholder_value(value) {
            return Err(ArcError::new(format!(
                "secret field '{key}' must use an environment placeholder"
            )));
        }
    }
    Ok(())
}

fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered == "authorization"
        || lowered.contains("token")
        || lowered.contains("secret")
        || lowered.contains("key")
        || lowered.contains("cookie")
}

fn is_secret_placeholder_value(value: &str) -> bool {
    ENV_PLACEHOLDER_RE.is_match(value)
        || ENV_DOLLAR_PLACEHOLDER_RE.is_match(value)
        || ENV_PROVIDER_PLACEHOLDER_RE.is_match(value)
        || ENV_OPENCODE_PLACEHOLDER_RE.is_match(value)
        || AUTH_PLACEHOLDER_RE.is_match(value)
        || AUTH_DOLLAR_PLACEHOLDER_RE.is_match(value)
        || AUTH_PROVIDER_PLACEHOLDER_RE.is_match(value)
        || AUTH_OPENCODE_PLACEHOLDER_RE.is_match(value)
}
