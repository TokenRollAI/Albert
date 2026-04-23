//! Remote source fetching — used by the Import dialog to pull OpenAPI
//! specs from a URL instead of making the user paste raw text.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

const MAX_FETCH_BYTES: usize = 2 * 1024 * 1024; // 2 MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedSource {
    pub url: String,
    pub content_type: Option<String>,
    pub body: String,
    pub suggested_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchArgs {
    pub url: String,
}

#[tauri::command]
pub async fn fetch_remote_source(args: FetchArgs) -> Result<FetchedSource, String> {
    if args.url.trim().is_empty() {
        return Err("URL is empty".into());
    }
    let parsed = reqwest::Url::parse(&args.url).map_err(|err| format!("invalid URL: {err}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported URL scheme: {other}")),
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("client build: {err}"))?;

    let response = client
        .get(parsed.clone())
        .header("accept", "application/json, text/yaml, text/plain, */*")
        .send()
        .await
        .map_err(|err| format!("fetch: {err}"))?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("read body: {err}"))?;
    if bytes.len() > MAX_FETCH_BYTES {
        return Err(format!(
            "response exceeds {MAX_FETCH_BYTES} bytes ({} bytes)",
            bytes.len()
        ));
    }
    if !status.is_success() {
        return Err(format!(
            "remote returned HTTP {status}: {}",
            String::from_utf8_lossy(&bytes)
                .chars()
                .take(512)
                .collect::<String>()
        ));
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();

    let suggested_name = parsed
        .path_segments()
        .and_then(|mut segs| segs.next_back())
        .and_then(|last| {
            if last.is_empty() {
                parsed.host_str().map(|s| s.to_string())
            } else {
                let trimmed = last.trim_end_matches(".json").trim_end_matches(".yaml");
                Some(trimmed.to_string())
            }
        });

    Ok(FetchedSource {
        url: parsed.to_string(),
        content_type,
        body,
        suggested_name,
    })
}
