use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, StatusCode};
use regex::Regex;
use tower::util::ServiceExt;
use xenobot_api::agent;

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn normalize_agent_tool_alias_input(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut prev_was_sep = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_uppercase() {
            if !out.is_empty() && !prev_was_sep {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_was_sep = false;
            continue;
        }
        if matches!(ch, '-' | ' ' | '/' | '_') {
            if !out.is_empty() && !prev_was_sep {
                out.push('_');
            }
            prev_was_sep = true;
            continue;
        }
        out.push(ch.to_ascii_lowercase());
        prev_was_sep = false;
    }
    out
}

fn parse_frontend_fallback_alias_pairs(
    frontend_file: &PathBuf,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(frontend_file)?;
    let re = Regex::new(r"\[\s*'([^']+)'\s*,\s*'([^']+)'\s*\]")?;
    let mut map = HashMap::new();
    for captures in re.captures_iter(&content) {
        let alias = normalize_agent_tool_alias_input(
            captures.get(1).map(|m| m.as_str()).unwrap_or_default(),
        );
        let canonical = normalize_agent_tool_alias_input(
            captures.get(2).map(|m| m.as_str()).unwrap_or_default(),
        );
        if !alias.is_empty() && !canonical.is_empty() {
            map.insert(alias, canonical);
        }
    }
    Ok(map)
}

#[tokio::test]
async fn test_frontend_agent_tool_alias_fallback_stays_in_sync_with_backend_catalog(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let frontend_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("web")
        .join("frontend")
        .join("src")
        .join("composables")
        .join("useAIChat.ts")
        .canonicalize()?;
    let fallback_map = parse_frontend_fallback_alias_pairs(&frontend_file)?;
    assert!(
        !fallback_map.is_empty(),
        "frontend alias fallback map should not be empty"
    );

    let app = agent::router();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/tools")
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let tools_json = serde_json::from_slice::<serde_json::Value>(&bytes)?;
    let tools = tools_json
        .as_array()
        .ok_or("agent /tools response should be an array")?;

    for tool in tools {
        let canonical = tool
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("tool.name should be string")?;
        let normalized_canonical = normalize_agent_tool_alias_input(canonical);
        let aliases = tool
            .get("aliases")
            .and_then(|v| v.as_array())
            .ok_or("tool.aliases should be array")?;
        for alias in aliases {
            let alias = alias
                .as_str()
                .ok_or("tool alias should be a string")?
                .to_string();
            let normalized_alias = normalize_agent_tool_alias_input(&alias);
            if normalized_alias.is_empty() || normalized_alias == normalized_canonical {
                continue;
            }
            let mapped = fallback_map.get(&normalized_alias).cloned();
            assert_eq!(
                mapped,
                Some(normalized_canonical.clone()),
                "frontend fallback alias map is missing or mismatched for backend alias '{}' (tool '{}')",
                alias,
                canonical
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_frontend_agent_tool_alias_fallback_has_no_stale_backend_targets(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let frontend_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("web")
        .join("frontend")
        .join("src")
        .join("composables")
        .join("useAIChat.ts")
        .canonicalize()?;
    let fallback_map = parse_frontend_fallback_alias_pairs(&frontend_file)?;
    assert!(
        !fallback_map.is_empty(),
        "frontend alias fallback map should not be empty"
    );

    let app = agent::router();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/tools")
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let tools_json = serde_json::from_slice::<serde_json::Value>(&bytes)?;
    let tools = tools_json
        .as_array()
        .ok_or("agent /tools response should be an array")?;

    let backend_tool_names = tools
        .iter()
        .filter_map(|tool| tool.get("name"))
        .filter_map(|value| value.as_str())
        .map(normalize_agent_tool_alias_input)
        .collect::<std::collections::HashSet<_>>();
    assert!(
        !backend_tool_names.is_empty(),
        "backend tool catalog should not be empty"
    );

    for (alias, canonical) in &fallback_map {
        assert!(
            backend_tool_names.contains(canonical),
            "frontend alias '{}' targets stale canonical tool '{}', which is not present in backend catalog",
            alias,
            canonical
        );
    }

    Ok(())
}
