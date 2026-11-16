use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PatchField<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

impl<T> PatchField<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

fn deserialize_patch_field<'de, D, T>(deserializer: D) -> Result<PatchField<T>, D::Error>
where
    D: Deserializer<'de>,
    Option<T>: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(PatchField::Value)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderUpdatePatch {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub api_key_ref: PatchField<String>,
    pub is_local: Option<bool>,
    pub is_enabled: Option<bool>,
    pub source_category: Option<String>,
    pub extra_config: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelUpdatePatch {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub display_name: PatchField<String>,
    pub model_type: Option<String>,
    pub context_window: Option<i64>,
    pub max_output: Option<i64>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub model_label: PatchField<String>,
    pub is_enabled: Option<bool>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub mxcs: PatchField<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayUpdatePatch {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub description: PatchField<String>,
    pub is_enabled: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{ModelUpdatePatch, PatchField, ProviderUpdatePatch};

    #[test]
    fn distinguishes_missing_value_and_null() {
        let missing: ProviderUpdatePatch = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(missing.api_key_ref.is_missing());

        let value: ProviderUpdatePatch =
            serde_json::from_value(serde_json::json!({"api_key_ref": "sk-full"})).unwrap();
        assert_eq!(value.api_key_ref, PatchField::Value(Some("sk-full".into())));

        let cleared: ProviderUpdatePatch =
            serde_json::from_value(serde_json::json!({"api_key_ref": null})).unwrap();
        assert_eq!(cleared.api_key_ref, PatchField::Value(None));
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(
            serde_json::from_value::<ProviderUpdatePatch>(serde_json::json!({
                "unexpected": true
            }))
            .is_err()
        );
    }

    #[test]
    fn model_patch_preserves_nullable_field_intent() {
        let patch: ModelUpdatePatch = serde_json::from_value(serde_json::json!({
            "display_name": null,
            "mxcs": null,
            "context_window": 32768,
            "is_enabled": false
        }))
        .unwrap();
        assert_eq!(patch.display_name, PatchField::Value(None));
        assert_eq!(patch.mxcs, PatchField::Value(None));
        assert_eq!(patch.context_window, Some(32768));
        assert_eq!(patch.is_enabled, Some(false));
    }
}
