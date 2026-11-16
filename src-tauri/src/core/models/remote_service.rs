use crate::core::error::AppError;

pub async fn fetch_openai_models_payload(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    let request_url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut request = client
        .get(&request_url)
        .timeout(std::time::Duration::from_secs(10));
    if let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = request
        .send()
        .await
        .map_err(|error| AppError::Network(error.to_string()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!("HTTP {} | {}", status, body)));
    }
    response
        .json()
        .await
        .map_err(|error| AppError::Parse(error.to_string()))
}
