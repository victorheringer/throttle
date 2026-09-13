use serde::Serialize;

#[derive(Debug, Serialize, Clone, Default)]
pub struct UsageResult {
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: String,
    pub error: Option<String>,
}

pub async fn fetch_usage(provider: &str, key: &str) -> UsageResult {
    match provider {
        "openrouter" => fetch_openrouter(key).await,
        "elevenlabs" => fetch_elevenlabs(key).await,
        other => UsageResult {
            error: Some(format!("provider \"{other}\" not supported yet")),
            ..Default::default()
        },
    }
}

async fn fetch_openrouter(key: &str) -> UsageResult {
    let client = reqwest::Client::new();
    let res = client
        .get("https://openrouter.ai/api/v1/credits")
        .bearer_auth(key)
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(body) => {
                let data = &body["data"];
                UsageResult {
                    used: data["total_usage"].as_f64(),
                    limit: data["total_credits"].as_f64(),
                    unit: "USD".into(),
                    error: None,
                }
            }
            Err(e) => error_result(format!("invalid response: {e}")),
        },
        Ok(resp) => error_result(format!("HTTP {}", resp.status())),
        Err(e) => error_result(format!("request failed: {e}")),
    }
}

async fn fetch_elevenlabs(key: &str) -> UsageResult {
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.elevenlabs.io/v1/user/subscription")
        .header("xi-api-key", key)
        .send()
        .await;

    match res {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(body) => UsageResult {
                used: body["character_count"].as_f64(),
                limit: body["character_limit"].as_f64(),
                unit: "characters".into(),
                error: None,
            },
            Err(e) => error_result(format!("invalid response: {e}")),
        },
        Ok(resp) => error_result(format!("HTTP {}", resp.status())),
        Err(e) => error_result(format!("request failed: {e}")),
    }
}

fn error_result(error: String) -> UsageResult {
    UsageResult {
        error: Some(error),
        ..Default::default()
    }
}
