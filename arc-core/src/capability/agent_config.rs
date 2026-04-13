use super::*;

pub(crate) fn write_agent_mcp(
    paths: &ArcPaths,
    agent: &str,
    definition: &McpDefinition,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(spec) = agent_spec(agent) else {
        return Err(ArcError::new(format!("unknown agent '{agent}'")));
    };
    let Some(path) = agent_mcp_path(paths, agent, scope, project_root) else {
        return Err(ArcError::new(format!(
            "mcp path unavailable for agent '{agent}'"
        )));
    };
    match spec.mcp_config_format {
        Some(McpConfigFormat::JsonMapTypedMcpServers) => upsert_json_map_mcp(
            &path,
            "mcpServers",
            &definition.name,
            json_typed_mcp_value(definition),
        ),
        Some(McpConfigFormat::JsonMapPlainMcpServers) => upsert_json_map_mcp(
            &path,
            "mcpServers",
            &definition.name,
            json_plain_mcp_value(definition),
        ),
        Some(McpConfigFormat::JsonMapGeminiMcpServers) => upsert_json_map_mcp(
            &path,
            "mcpServers",
            &definition.name,
            gemini_mcp_value(definition),
        ),
        Some(McpConfigFormat::JsonMapKimiMcpServers) => upsert_json_map_mcp(
            &path,
            "mcpServers",
            &definition.name,
            kimi_mcp_value(definition),
        ),
        Some(McpConfigFormat::JsonOpenCode) => upsert_opencode_mcp(&path, definition),
        Some(McpConfigFormat::TomlCodexMcpServers) => upsert_codex_mcp(&path, definition),
        None => Err(ArcError::new(format!(
            "agent '{agent}' does not support mcp"
        ))),
    }
}

pub(crate) fn remove_agent_mcp(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(spec) = agent_spec(agent) else {
        return Ok(());
    };
    let Some(path) = agent_mcp_path(paths, agent, scope, project_root) else {
        return Ok(());
    };
    match spec.mcp_config_format {
        Some(McpConfigFormat::JsonMapTypedMcpServers)
        | Some(McpConfigFormat::JsonMapPlainMcpServers)
        | Some(McpConfigFormat::JsonMapGeminiMcpServers)
        | Some(McpConfigFormat::JsonMapKimiMcpServers) => {
            remove_json_map_mcp(&path, "mcpServers", name)
        }
        Some(McpConfigFormat::JsonOpenCode) => remove_opencode_mcp(&path, name),
        Some(McpConfigFormat::TomlCodexMcpServers) => remove_toml_mcp(&path, name),
        None => Ok(()),
    }
}

pub(crate) fn write_agent_subagent(
    paths: &ArcPaths,
    agent: &str,
    definition: &SubagentDefinition,
    prompt_body: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(dir) = agent_subagent_dir(paths, agent, scope, project_root) else {
        return Err(ArcError::new(format!(
            "subagent directory unavailable for agent '{agent}'"
        )));
    };
    fs::create_dir_all(&dir)
        .map_err(|e| ArcError::new(format!("failed to create {}: {e}", dir.display())))?;
    let Some(spec) = agent_spec(agent) else {
        return Err(ArcError::new(format!("unknown agent '{agent}'")));
    };
    let Some(format) = spec.subagent_format else {
        return Err(ArcError::new(format!(
            "subagent writer not implemented for agent '{agent}'"
        )));
    };
    match format {
        SubagentFormat::TomlDeveloperInstructions => {
            let file = dir.join(format!("{}.toml", sanitize_filename(&definition.name)));
            let mut table = toml::map::Map::new();
            table.insert(
                "name".to_string(),
                toml::Value::String(definition.name.clone()),
            );
            if let Some(description) = &definition.description {
                table.insert(
                    "description".to_string(),
                    toml::Value::String(description.clone()),
                );
            }
            table.insert(
                "developer_instructions".to_string(),
                toml::Value::String(prompt_body.to_string()),
            );
            let body = toml::to_string_pretty(&toml::Value::Table(table))
                .map_err(|e| ArcError::new(format!("failed to serialize codex subagent: {e}")))?;
            atomic_write_string(&file, &body)
                .map_err(|e| ArcError::new(format!("failed to write {}: {e}", file.display())))?;
        }
        SubagentFormat::MarkdownFrontmatter => {
            let file = dir.join(format!("{}.md", sanitize_filename(&definition.name)));
            let body = render_markdown_subagent(spec.id, definition, prompt_body)?;
            atomic_write_string(&file, &body)
                .map_err(|e| ArcError::new(format!("failed to write {}: {e}", file.display())))?;
        }
    }
    Ok(())
}

pub(crate) fn remove_agent_subagent(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(dir) = agent_subagent_dir(paths, agent, scope, project_root) else {
        return Ok(());
    };
    let Some(spec) = agent_spec(agent) else {
        return Ok(());
    };
    let file = match spec.subagent_format {
        Some(SubagentFormat::TomlDeveloperInstructions) => {
            dir.join(format!("{}.toml", sanitize_filename(name)))
        }
        Some(SubagentFormat::MarkdownFrontmatter) => {
            dir.join(format!("{}.md", sanitize_filename(name)))
        }
        _ => return Ok(()),
    };
    if file.exists() {
        fs::remove_file(&file)
            .map_err(|e| ArcError::new(format!("failed to remove {}: {e}", file.display())))?;
    }
    Ok(())
}

pub(crate) fn mcp_install_present(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<bool> {
    let Some(spec) = agent_spec(agent) else {
        return Ok(false);
    };
    let Some(path) = agent_mcp_path(paths, agent, scope, project_root) else {
        return Ok(false);
    };
    if !path.exists() {
        return Ok(false);
    }
    match spec.mcp_config_format {
        Some(McpConfigFormat::JsonMapTypedMcpServers)
        | Some(McpConfigFormat::JsonMapPlainMcpServers)
        | Some(McpConfigFormat::JsonMapGeminiMcpServers)
        | Some(McpConfigFormat::JsonMapKimiMcpServers) => {
            let root = load_json_root(&path)?;
            Ok(json_nested_contains_key(&root, "mcpServers", name))
        }
        Some(McpConfigFormat::JsonOpenCode) => {
            let root = load_json_root(&path)?;
            Ok(root
                .get("mcp")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|mcp| mcp.contains_key(name)))
        }
        Some(McpConfigFormat::TomlCodexMcpServers) => {
            let root = load_toml_root(&path)?;
            Ok(root
                .as_table()
                .and_then(|table| table.get("mcp_servers"))
                .and_then(toml::Value::as_table)
                .is_some_and(|servers| servers.contains_key(name)))
        }
        None => Ok(false),
    }
}

pub(crate) fn subagent_install_present(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<bool> {
    let Some(spec) = agent_spec(agent) else {
        return Ok(false);
    };
    let Some(dir) = agent_subagent_dir(paths, agent, scope, project_root) else {
        return Ok(false);
    };
    let file = match spec.subagent_format {
        Some(SubagentFormat::TomlDeveloperInstructions) => {
            dir.join(format!("{}.toml", sanitize_filename(name)))
        }
        Some(SubagentFormat::MarkdownFrontmatter) => {
            dir.join(format!("{}.md", sanitize_filename(name)))
        }
        None => return Ok(false),
    };
    Ok(file.exists())
}

pub(crate) fn render_markdown_subagent(
    agent_id: &str,
    definition: &SubagentDefinition,
    prompt_body: &str,
) -> Result<String> {
    #[derive(Serialize)]
    struct Frontmatter<'a> {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<&'a str>,
    }

    let frontmatter = serde_yaml::to_string(&Frontmatter {
        name: &definition.name,
        description: definition.description.as_deref(),
        mode: (agent_id == "opencode").then_some("subagent"),
    })
    .map_err(|e| ArcError::new(format!("failed to serialize subagent frontmatter: {e}")))?;
    Ok(format!("---\n{}---\n{}", frontmatter, prompt_body))
}

fn upsert_json_map_mcp(path: &Path, key: &str, name: &str, value: serde_json::Value) -> Result<()> {
    let mut root = load_json_root(path)?;
    set_nested_json_object(&mut root, key, name, value)?;
    write_json_root(path, &root)
}

fn remove_json_map_mcp(path: &Path, key: &str, name: &str) -> Result<()> {
    let mut root = load_json_root(path)?;
    remove_nested_json_key(&mut root, key, name)?;
    write_json_root(path, &root)
}

fn upsert_opencode_mcp(path: &Path, definition: &McpDefinition) -> Result<()> {
    let mut root = load_json_root(path)?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| ArcError::new(format!("expected JSON object in {}", path.display())))?;
    let mcp = obj
        .entry("mcp")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let mcp_obj = mcp
        .as_object_mut()
        .ok_or_else(|| ArcError::new(format!("expected object at mcp in {}", path.display())))?;
    mcp_obj.insert(definition.name.clone(), opencode_mcp_value(definition));
    write_json_root(path, &root)
}

fn remove_opencode_mcp(path: &Path, name: &str) -> Result<()> {
    let mut root = load_json_root(path)?;
    if let Some(obj) = root.as_object_mut()
        && let Some(mcp) = obj.get_mut("mcp")
        && let Some(mcp_obj) = mcp.as_object_mut()
    {
        mcp_obj.remove(name);
    }
    write_json_root(path, &root)
}

fn upsert_codex_mcp(path: &Path, definition: &McpDefinition) -> Result<()> {
    let mut root = load_toml_root(path)?;
    let table = root
        .as_table_mut()
        .ok_or_else(|| ArcError::new(format!("expected TOML table in {}", path.display())))?;
    let mcp_servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let server_table = mcp_servers.as_table_mut().ok_or_else(|| {
        ArcError::new(format!(
            "expected TOML table at mcp_servers in {}",
            path.display()
        ))
    })?;
    server_table.insert(definition.name.clone(), toml_codex_mcp_value(definition)?);
    write_toml_root(path, &root)
}

fn remove_toml_mcp(path: &Path, name: &str) -> Result<()> {
    let mut root = load_toml_root(path)?;
    if let Some(table) = root.as_table_mut()
        && let Some(mcp_servers) = table.get_mut("mcp_servers")
        && let Some(servers) = mcp_servers.as_table_mut()
    {
        servers.remove(name);
    }
    write_toml_root(path, &root)
}

fn json_typed_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => serde_json::json!({
            "type": "stdio",
            "command": definition.command.clone().unwrap_or_default(),
            "args": definition.args,
            "env": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.env).unwrap_or(serde_json::Value::Null) },
            "cwd": definition.cwd,
            "envFile": definition.env_file,
            "startup_timeout_sec": definition.startup_timeout_sec,
            "tool_timeout_sec": definition.tool_timeout_sec,
            "enabled": definition.enabled,
            "required": definition.required,
        }),
        McpTransportType::Sse => serde_json::json!({
            "type": "sse",
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
            "oauth": claude_oauth_value(definition),
        }),
        McpTransportType::StreamableHttp => serde_json::json!({
            "type": "http",
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
            "oauth": claude_oauth_value(definition),
        }),
    }
}

fn json_plain_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => serde_json::json!({
            "command": definition.command.clone().unwrap_or_default(),
            "args": definition.args,
            "env": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.env).unwrap_or(serde_json::Value::Null) },
            "cwd": definition.cwd,
            "envFile": definition.env_file.as_ref().map(|value| cursor_env_file_value(value)),
        }),
        McpTransportType::Sse | McpTransportType::StreamableHttp => serde_json::json!({
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(cursor_map(&definition.headers)).unwrap_or(serde_json::Value::Null) },
        }),
    }
}

pub(crate) fn gemini_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => serde_json::json!({
            "command": definition.command.clone().unwrap_or_default(),
            "args": definition.args,
            "env": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.env).unwrap_or(serde_json::Value::Null) },
            "cwd": definition.cwd,
            "timeout": definition.timeout,
            "trust": definition.trust,
            "includeTools": if definition.include_tools.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.include_tools).unwrap_or(serde_json::Value::Null) },
            "excludeTools": if definition.exclude_tools.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.exclude_tools).unwrap_or(serde_json::Value::Null) },
        }),
        McpTransportType::Sse => serde_json::json!({
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
            "timeout": definition.timeout,
            "trust": definition.trust,
            "includeTools": if definition.include_tools.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.include_tools).unwrap_or(serde_json::Value::Null) },
            "excludeTools": if definition.exclude_tools.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.exclude_tools).unwrap_or(serde_json::Value::Null) },
        }),
        McpTransportType::StreamableHttp => serde_json::json!({
            "httpUrl": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
            "timeout": definition.timeout,
            "trust": definition.trust,
            "includeTools": if definition.include_tools.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.include_tools).unwrap_or(serde_json::Value::Null) },
            "excludeTools": if definition.exclude_tools.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.exclude_tools).unwrap_or(serde_json::Value::Null) },
        }),
    }
}

pub(crate) fn kimi_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => serde_json::json!({
            "command": definition.command.clone().unwrap_or_default(),
            "args": definition.args,
            "env": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.env).unwrap_or(serde_json::Value::Null) },
            "transport": "stdio",
        }),
        McpTransportType::StreamableHttp => serde_json::json!({
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
            "transport": "http",
        }),
        McpTransportType::Sse => serde_json::json!({
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
            "transport": "sse",
        }),
    }
}

fn opencode_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => {
            let mut command = vec![definition.command.clone().unwrap_or_default()];
            command.extend(definition.args.clone());
            serde_json::json!({
                "type": "local",
                "command": command,
                "enabled": true,
                "environment": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(opencode_map(&definition.env)).unwrap_or(serde_json::Value::Null) },
                "timeout": definition.timeout,
            })
        }
        McpTransportType::Sse | McpTransportType::StreamableHttp => serde_json::json!({
            "type": "remote",
            "url": definition.url.clone().unwrap_or_default(),
            "enabled": true,
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(opencode_map(&definition.headers)).unwrap_or(serde_json::Value::Null) },
            "oauth": opencode_oauth_value(definition),
            "timeout": definition.timeout,
        }),
    }
}

fn claude_oauth_value(definition: &McpDefinition) -> serde_json::Value {
    match &definition.oauth {
        Some(McpOAuthConfig::Disabled(false)) => serde_json::Value::Bool(false),
        Some(McpOAuthConfig::Settings(settings)) => {
            let mut obj = serde_json::Map::new();
            if let Some(value) = &settings.client_id {
                obj.insert(
                    "clientId".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            if let Some(value) = &settings.client_secret {
                obj.insert(
                    "clientSecret".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            if let Some(value) = &settings.scope {
                obj.insert(
                    "scope".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            if let Some(value) = settings.callback_port {
                obj.insert(
                    "callbackPort".to_string(),
                    serde_json::Value::Number(value.into()),
                );
            }
            if let Some(value) = &settings.auth_server_metadata_url {
                obj.insert(
                    "authServerMetadataUrl".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

fn opencode_oauth_value(definition: &McpDefinition) -> serde_json::Value {
    match &definition.oauth {
        Some(McpOAuthConfig::Disabled(false)) => serde_json::Value::Bool(false),
        Some(McpOAuthConfig::Settings(settings)) => {
            let mut obj = serde_json::Map::new();
            if let Some(value) = &settings.client_id {
                obj.insert(
                    "clientId".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            if let Some(value) = &settings.client_secret {
                obj.insert(
                    "clientSecret".to_string(),
                    serde_json::Value::String(opencode_placeholder(value)),
                );
            }
            if let Some(value) = &settings.scope {
                obj.insert(
                    "scope".to_string(),
                    serde_json::Value::String(value.clone()),
                );
            }
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

fn cursor_map(map: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), cursor_placeholder(value)))
        .collect()
}

fn opencode_map(map: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), opencode_placeholder(value)))
        .collect()
}

fn cursor_env_file_value(value: &str) -> String {
    cursor_placeholder(value)
}

fn cursor_placeholder(value: &str) -> String {
    if let Some(name) = extract_env_name(value) {
        if value.starts_with("Bearer ") || value.starts_with("Basic ") {
            let (scheme, _) = value.split_once(' ').unwrap_or(("Bearer", ""));
            return format!("{scheme} ${{env:{name}}}");
        }
        return format!("${{env:{name}}}");
    }
    value.to_string()
}

fn opencode_placeholder(value: &str) -> String {
    if let Some(name) = extract_env_name(value) {
        if value.starts_with("Bearer ") || value.starts_with("Basic ") {
            let (scheme, _) = value.split_once(' ').unwrap_or(("Bearer", ""));
            return format!("{scheme} {{env:{name}}}");
        }
        return format!("{{env:{name}}}");
    }
    value.to_string()
}

fn extract_env_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    for prefix in ["Bearer ", "Basic "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return extract_env_name(rest);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("${")
        && let Some(name) = rest.strip_suffix('}')
    {
        return Some(name.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix('$') {
        return Some(rest.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("{env:")
        && let Some(name) = rest.strip_suffix('}')
    {
        return Some(name.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("${env:")
        && let Some(name) = rest.strip_suffix('}')
    {
        return Some(name.to_string());
    }
    None
}

fn split_codex_headers(
    headers: &BTreeMap<String, String>,
) -> (
    Option<String>,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
) {
    let mut bearer_token_env_var = None;
    let mut static_headers = BTreeMap::new();
    let mut env_headers = BTreeMap::new();
    for (key, value) in headers {
        if key.eq_ignore_ascii_case("authorization")
            && let Some(env_name) = extract_bearer_env_name(value)
        {
            bearer_token_env_var = Some(env_name);
            continue;
        }
        if let Some(env_name) = extract_env_name(value) {
            env_headers.insert(key.clone(), env_name);
        } else {
            static_headers.insert(key.clone(), value.clone());
        }
    }
    (bearer_token_env_var, static_headers, env_headers)
}

fn extract_bearer_env_name(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    extract_env_name(rest)
}

pub(crate) fn toml_codex_mcp_value(definition: &McpDefinition) -> Result<toml::Value> {
    let mut table = toml::map::Map::new();
    match definition.transport {
        McpTransportType::Stdio => {
            let command = definition.command.clone().ok_or_else(|| {
                ArcError::new(format!(
                    "mcp '{}' requires command for stdio transport",
                    definition.name
                ))
            })?;
            table.insert("command".to_string(), toml::Value::String(command));
            if !definition.args.is_empty() {
                table.insert(
                    "args".to_string(),
                    toml::Value::Array(
                        definition
                            .args
                            .iter()
                            .map(|arg| toml::Value::String(arg.clone()))
                            .collect(),
                    ),
                );
            }
            if !definition.env.is_empty() {
                let env_table = definition
                    .env
                    .iter()
                    .map(|(key, value)| (key.clone(), toml::Value::String(value.clone())))
                    .collect();
                table.insert("env".to_string(), toml::Value::Table(env_table));
            }
            if let Some(cwd) = &definition.cwd {
                table.insert("cwd".to_string(), toml::Value::String(cwd.clone()));
            }
        }
        McpTransportType::StreamableHttp => {
            table.insert(
                "url".to_string(),
                toml::Value::String(definition.url.clone().unwrap_or_default()),
            );
            let (bearer_token_env_var, static_headers, env_headers) =
                split_codex_headers(&definition.headers);
            if let Some(env_var) = bearer_token_env_var {
                table.insert(
                    "bearer_token_env_var".to_string(),
                    toml::Value::String(env_var),
                );
            }
            if !static_headers.is_empty() {
                table.insert(
                    "http_headers".to_string(),
                    toml::Value::Table(
                        static_headers
                            .into_iter()
                            .map(|(k, v)| (k, toml::Value::String(v)))
                            .collect(),
                    ),
                );
            }
            if !env_headers.is_empty() {
                table.insert(
                    "env_http_headers".to_string(),
                    toml::Value::Table(
                        env_headers
                            .into_iter()
                            .map(|(k, v)| (k, toml::Value::String(v)))
                            .collect(),
                    ),
                );
            }
        }
        McpTransportType::Sse => {
            return Err(ArcError::new(
                "codex does not support sse transport".to_string(),
            ));
        }
    }
    if let Some(timeout) = definition.startup_timeout_sec {
        table.insert(
            "startup_timeout_sec".to_string(),
            toml::Value::Integer(timeout as i64),
        );
    }
    if let Some(timeout) = definition.tool_timeout_sec {
        table.insert(
            "tool_timeout_sec".to_string(),
            toml::Value::Integer(timeout as i64),
        );
    }
    if let Some(enabled) = definition.enabled {
        table.insert("enabled".to_string(), toml::Value::Boolean(enabled));
    }
    if let Some(required) = definition.required {
        table.insert("required".to_string(), toml::Value::Boolean(required));
    }
    if !definition.include_tools.is_empty() {
        table.insert(
            "enabled_tools".to_string(),
            toml::Value::Array(
                definition
                    .include_tools
                    .iter()
                    .map(|item| toml::Value::String(item.clone()))
                    .collect(),
            ),
        );
    }
    if !definition.exclude_tools.is_empty() {
        table.insert(
            "disabled_tools".to_string(),
            toml::Value::Array(
                definition
                    .exclude_tools
                    .iter()
                    .map(|item| toml::Value::String(item.clone()))
                    .collect(),
            ),
        );
    }
    Ok(toml::Value::Table(table))
}

fn load_json_root(path: &Path) -> Result<serde_json::Value> {
    match fs::read_to_string(path) {
        Ok(body) => {
            if body.trim().is_empty() {
                Ok(serde_json::Value::Object(serde_json::Map::new()))
            } else {
                serde_json::from_str(&body)
                    .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::Value::Object(serde_json::Map::new()))
        }
        Err(err) => Err(ArcError::new(format!(
            "failed to read {}: {err}",
            path.display()
        ))),
    }
}

fn write_json_root(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ArcError::new(format!("failed to create {}: {e}", parent.display())))?;
    }
    let body = serde_json::to_string_pretty(value)
        .map_err(|e| ArcError::new(format!("failed to serialize {}: {e}", path.display())))?;
    atomic_write_string(path, &body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

fn load_toml_root(path: &Path) -> Result<toml::Value> {
    match fs::read_to_string(path) {
        Ok(body) => {
            if body.trim().is_empty() {
                Ok(toml::Value::Table(toml::map::Map::new()))
            } else {
                toml::from_str(&body)
                    .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(toml::Value::Table(toml::map::Map::new()))
        }
        Err(err) => Err(ArcError::new(format!(
            "failed to read {}: {err}",
            path.display()
        ))),
    }
}

fn write_toml_root(path: &Path, value: &toml::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ArcError::new(format!("failed to create {}: {e}", parent.display())))?;
    }
    let body = toml::to_string_pretty(value)
        .map_err(|e| ArcError::new(format!("failed to serialize {}: {e}", path.display())))?;
    atomic_write_string(path, &body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

fn set_nested_json_object(
    root: &mut serde_json::Value,
    path: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<()> {
    let keys: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for segment in keys {
        let object = current
            .as_object_mut()
            .ok_or_else(|| ArcError::new("expected JSON object while writing nested mcp config"))?;
        current = object
            .entry(segment)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    }
    let object = current
        .as_object_mut()
        .ok_or_else(|| ArcError::new("expected JSON object while writing mcp entry"))?;
    object.insert(key.to_string(), value);
    Ok(())
}

fn remove_nested_json_key(root: &mut serde_json::Value, path: &str, key: &str) -> Result<()> {
    let keys: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for segment in keys {
        let Some(next) = current.get_mut(segment) else {
            return Ok(());
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.remove(key);
    }
    Ok(())
}

fn json_nested_contains_key(root: &serde_json::Value, path: &str, key: &str) -> bool {
    let mut current = root;
    for segment in path.split('.') {
        let Some(next) = current.get(segment) else {
            return false;
        };
        current = next;
    }
    current
        .as_object()
        .is_some_and(|object| object.contains_key(key))
}

fn sanitize_filename(name: &str) -> String {
    let collapsed = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_ascii_lowercase();
    collapsed
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
