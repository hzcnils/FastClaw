use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct ProviderDto {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    /// 管理端契约要求返回完整 API Key；持久化列名不会暴露给前端。
    pub api_key: Option<String>,
    pub is_local: bool,
    pub is_enabled: bool,
    pub provider_code: Option<String>,
    pub source_category: String,
    pub extra_config: Option<String>,
    pub gateway_id: Option<String>,
}

/// Non-management provider projection. It exposes configuration completeness,
/// but never serializes the credential itself.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct ProviderSummaryDto {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub has_api_key: bool,
    pub is_local: bool,
    pub is_enabled: bool,
    pub provider_code: Option<String>,
    pub source_category: String,
}

/// Narrow runtime projection for a single provider. Callers must request it by
/// provider code instead of loading all management credentials.
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct ProviderRuntimeConfigDto {
    pub id: String,
    pub provider_code: String,
    pub base_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct ModelDto {
    pub id: String,
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
    pub is_enabled: bool,
    pub model_label: String,
    pub mxcs: Option<String>,
    pub context_window: Option<i64>,
    pub provider_code: String,
    pub supports_vision: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct GatewayDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GatewayWithProvidersDto {
    #[serde(flatten)]
    pub gateway: GatewayDto,
    pub providers: Vec<ProviderDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestResult {
    pub success: bool,
    pub latency_ms: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct ModelExportDto {
    pub id: String,
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
    pub is_enabled: bool,
    pub model_label: String,
    pub mxcs: Option<String>,
    pub context_window: Option<i64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub extra_config: Option<String>,
    pub provider_code: Option<String>,
    pub base_url: String,
    pub api_key_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileExportDto {
    pub id: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub model: String,
    pub provider: String,
    pub name: String,
    pub response_limit: Option<i64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub base_url: String,
    pub endpoint: Option<String>,
    pub gemini_location: Option<String>,
    pub gemini_project: Option<String>,
    pub azure_deployment_name: Option<String>,
    pub image_size: Option<String>,
    pub image_quality: Option<String>,
    pub image_style: Option<String>,
    pub dimension: Option<String>,
    pub model_label: String,
    pub source: String,
    pub mxcs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteModelDto {
    pub id: String,
    pub provider_code: String,
    #[serde(default)]
    pub model_id: Option<String>,
    pub model_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default = "default_model_type")]
    pub model_type: String,
    #[serde(default)]
    pub model_label: Option<String>,
    #[serde(default, alias = "context_length")]
    pub context_window: Option<i64>,
    #[serde(default, alias = "max_tokens")]
    pub max_output: Option<i64>,
    #[serde(default)]
    pub input_price: Option<f64>,
    #[serde(default)]
    pub output_price: Option<f64>,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default, rename = "_exists")]
    pub exists: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
    #[serde(default)]
    pub mxcs: Option<String>,
}

fn default_model_type() -> String {
    "chat".to_string()
}

#[cfg(test)]
mod tests {
    use super::ProviderDto;

    #[test]
    fn provider_contract_serializes_full_api_key_under_management_field() {
        let value = serde_json::to_value(ProviderDto {
            id: "provider-1".into(),
            name: "Provider".into(),
            provider_type: "openai".into(),
            base_url: "https://example.test/v1".into(),
            api_key: Some("sk-complete-management-key".into()),
            is_local: false,
            is_enabled: true,
            provider_code: Some("PROVIDER".into()),
            source_category: "online".into(),
            extra_config: None,
            gateway_id: None,
        })
        .unwrap();

        assert_eq!(value["api_key"], "sk-complete-management-key");
        assert!(value.get("api_key_ref").is_none());
        assert_eq!(value["source_category"], "online");
    }
}
