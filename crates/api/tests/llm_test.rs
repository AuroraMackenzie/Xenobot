use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::util::ServiceExt;
use xenobot_api::llm;

async fn get_json(
    app: &axum::Router,
    path: &str,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_json = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;
    Ok((status, body_json))
}

async fn post_json(
    app: &axum::Router,
    path: &str,
    payload: serde_json::Value,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_json = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;
    Ok((status, body_json))
}

#[tokio::test]
async fn test_get_providers_returns_expected_catalog_entries(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = llm::router();
    let (status, resp) = get_json(&app, "/providers").await?;
    assert_eq!(status, StatusCode::OK);

    let providers = resp
        .as_array()
        .expect("providers endpoint should return provider array");
    assert!(!providers.is_empty());
    assert!(providers.iter().any(|p| p["id"] == "qwen"));
    assert!(providers.iter().any(|p| p["id"] == "openai-compatible"));
    Ok(())
}

#[tokio::test]
async fn test_add_config_rejects_unknown_provider_and_invalid_model(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = llm::router();

    let (status, unknown_provider_resp) = post_json(
        &app,
        "/configs",
        json!({
            "name": "Invalid Provider Config",
            "provider": "unknown-provider",
            "apiKey": "dummy",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unknown_provider_resp["success"], false);
    assert!(unknown_provider_resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unsupported provider"));

    let (status, invalid_model_resp) = post_json(
        &app,
        "/configs",
        json!({
            "name": "Invalid Model Config",
            "provider": "qwen",
            "apiKey": "dummy",
            "model": "not-a-valid-model",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invalid_model_resp["success"], false);
    assert!(invalid_model_resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("not supported for provider `qwen`"));
    Ok(())
}

#[tokio::test]
async fn test_validate_api_key_enforces_provider_model_and_base_url_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = llm::router();

    let (status, unknown_provider_resp) = post_json(
        &app,
        "/validate-api-key",
        json!({
            "provider": "invalid-provider",
            "apiKey": "dummy",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unknown_provider_resp["success"], false);
    assert!(unknown_provider_resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unsupported provider"));

    let (status, invalid_model_resp) = post_json(
        &app,
        "/validate-api-key",
        json!({
            "provider": "qwen",
            "apiKey": "dummy",
            "model": "unknown-model",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invalid_model_resp["success"], false);
    assert!(invalid_model_resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("not supported for provider `qwen`"));

    let (status, missing_key_resp) = post_json(
        &app,
        "/validate-api-key",
        json!({
            "provider": "qwen",
            "apiKey": "",
            "model": "qwen-plus",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(missing_key_resp["success"], false);
    assert_eq!(missing_key_resp["error"], "apiKey is required");

    let (status, invalid_base_url_resp) = post_json(
        &app,
        "/validate-api-key",
        json!({
            "provider": "openai-compatible",
            "apiKey": "",
            "model": "my-local-model",
            "baseUrl": "invalid-url",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invalid_base_url_resp["success"], false);
    assert_eq!(invalid_base_url_resp["error"], "invalid baseUrl");

    let (status, ok_resp) = post_json(
        &app,
        "/validate-api-key",
        json!({
            "provider": "openai-compatible",
            "apiKey": "",
            "model": "my-local-model",
            "baseUrl": "http://localhost:11434/v1",
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ok_resp["success"], true);
    Ok(())
}
