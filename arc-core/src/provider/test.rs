use std::time::{Duration, Instant};

use log::info;

use super::{ProviderInfo, ProviderSettings};

#[derive(Debug, Clone)]
pub struct ProviderTestResult {
    pub provider_name: String,
    pub agent: String,
    pub display_name: String,
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub message: String,
}

/// Test connectivity to a provider's API endpoint.
pub fn test_provider(provider: &ProviderInfo) -> ProviderTestResult {
    let base = ProviderTestResult {
        provider_name: provider.name.clone(),
        agent: provider.agent.clone(),
        display_name: provider.display_name.clone(),
        ok: false,
        latency_ms: None,
        message: String::new(),
    };

    match &provider.settings {
        ProviderSettings::Claude(config) => {
            let Some(base_url) = config.env_vars.get("ANTHROPIC_BASE_URL") else {
                // Official provider — no custom URL to test, assume ok.
                info!(
                    "provider test: {} — official endpoint (skipped)",
                    provider.name
                );
                return ProviderTestResult {
                    ok: true,
                    message: "official endpoint (no custom URL)".to_string(),
                    ..base
                };
            };
            let auth_token = config.env_vars.get("ANTHROPIC_AUTH_TOKEN");
            test_http_endpoint(base, base_url, auth_token.map(|s| s.as_str()), "x-api-key")
        }
        ProviderSettings::Codex(config) => {
            let Some(base_url) = &config.base_url else {
                // Official provider — no custom URL.
                info!(
                    "provider test: {} — official endpoint (skipped)",
                    provider.name
                );
                return ProviderTestResult {
                    ok: true,
                    message: "official endpoint (no custom URL)".to_string(),
                    ..base
                };
            };
            test_http_endpoint(base, base_url, config.api_key.as_deref(), "Authorization")
        }
    }
}

fn test_http_endpoint(
    base: ProviderTestResult,
    base_url: &str,
    auth: Option<&str>,
    auth_header: &str,
) -> ProviderTestResult {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    info!("provider test: {} — GET {}", base.provider_name, url);

    let start = Instant::now();
    let mut request = ureq::get(&url).timeout(Duration::from_secs(10));

    if let Some(token) = auth {
        let value = if auth_header == "Authorization" {
            format!("Bearer {token}")
        } else {
            token.to_string()
        };
        request = request.set(auth_header, &value);
    }

    match request.call() {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = response.status();
            info!(
                "provider test: {} — {} ({}ms)",
                base.provider_name, status, latency
            );
            ProviderTestResult {
                ok: (200..300).contains(&(status as usize)),
                latency_ms: Some(latency),
                message: format!("HTTP {status} ({latency}ms)"),
                ..base
            }
        }
        Err(ureq::Error::Status(status, _response)) => {
            let latency = start.elapsed().as_millis() as u64;
            let msg = match status {
                401 | 403 => "authentication failed".to_string(),
                429 => "rate limited".to_string(),
                _ => format!("HTTP {status}"),
            };
            info!(
                "provider test: {} — {} ({}ms)",
                base.provider_name, msg, latency
            );
            ProviderTestResult {
                ok: false,
                latency_ms: Some(latency),
                message: format!("{msg} ({latency}ms)"),
                ..base
            }
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            let msg = if e.to_string().contains("timed out") || e.to_string().contains("Timeout") {
                "connection timed out".to_string()
            } else if e.to_string().contains("refused") {
                "connection refused".to_string()
            } else if e.to_string().contains("dns") || e.to_string().contains("resolve") {
                "DNS resolution failed".to_string()
            } else {
                format!("{e}")
            };
            info!("provider test: {} — {}", base.provider_name, msg);
            ProviderTestResult {
                ok: false,
                latency_ms: Some(latency),
                message: msg,
                ..base
            }
        }
    }
}
