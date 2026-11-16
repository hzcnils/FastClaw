use serde_json::Value;

use super::{ModelExportDto, ModelFileExportDto};

fn string_field(value: &Value, name: &str) -> Option<String> {
    value.get(name).and_then(Value::as_str).map(str::to_string)
}

/// Converts the internal joined row into the public models.json contract.
/// Credential inclusion is decided here so the frontend cannot recover a
/// secret from the raw extra_config after opting out.
pub fn build_model_file_export(
    row: ModelExportDto,
    include_credentials: bool,
) -> ModelFileExportDto {
    let extra = row
        .extra_config
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Default::default()));
    let provider = row.provider_code.unwrap_or_default();

    ModelFileExportDto {
        id: row.id,
        model_type: row.model_type.to_uppercase(),
        model: row.model_id,
        provider: provider.clone(),
        name: row.display_name,
        response_limit: extra.get("responseLimit").and_then(Value::as_i64),
        temperature: row.temperature,
        top_p: row.top_p,
        api_key: include_credentials.then_some(row.api_key_ref).flatten(),
        secret_key: include_credentials
            .then(|| string_field(&extra, "secretKey"))
            .flatten(),
        base_url: row.base_url,
        endpoint: string_field(&extra, "endpoint"),
        gemini_location: string_field(&extra, "geminiLocation"),
        gemini_project: string_field(&extra, "geminiProject"),
        azure_deployment_name: string_field(&extra, "azureDeploymentName"),
        image_size: string_field(&extra, "imageSize"),
        image_quality: string_field(&extra, "imageQuality"),
        image_style: string_field(&extra, "imageStyle"),
        dimension: string_field(&extra, "dimension"),
        model_label: row.model_label,
        source: string_field(&extra, "source").unwrap_or(provider),
        mxcs: row.mxcs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> ModelExportDto {
        ModelExportDto {
            id: "OPENAI-gpt".into(),
            model_id: "gpt".into(),
            display_name: "GPT".into(),
            model_type: "chat".into(),
            is_enabled: true,
            model_label: "tools".into(),
            mxcs: None,
            context_window: Some(8192),
            temperature: Some(0.7),
            top_p: Some(1.0),
            extra_config: Some(
                r#"{"secretKey":"secret","apiKey":"duplicate","source":"OPENAI"}"#.into(),
            ),
            provider_code: Some("OPENAI".into()),
            base_url: "https://example.test/v1".into(),
            api_key_ref: Some("provider-key".into()),
        }
    }

    #[test]
    fn export_includes_only_canonical_provider_key_when_enabled() {
        let export = build_model_file_export(row(), true);
        assert_eq!(export.api_key.as_deref(), Some("provider-key"));
        assert_eq!(export.secret_key.as_deref(), Some("secret"));
    }

    #[test]
    fn export_removes_both_credentials_when_disabled() {
        let export = build_model_file_export(row(), false);
        assert_eq!(export.api_key, None);
        assert_eq!(export.secret_key, None);
    }
}
