/// 根据模型名称/类型自动推断 model_label 和 model_type。
/// 匹配规则与 seed_model_capability_tags.sql 保持一致。
pub fn infer_model_info(model_id: &str, api_type: Option<&str>) -> (String, String) {
    let id_lower = model_id.to_lowercase();
    let api_type = api_type.unwrap_or("chat");
    let model_type = if id_lower.contains("embed")
        || id_lower.contains("bge-m3")
        || id_lower.contains("bge-large")
        || id_lower.contains("text-embedding")
    {
        "embedding"
    } else if id_lower.contains("rerank")
        || id_lower.contains("bge-reranker")
        || id_lower.contains("cohere-rerank")
        || id_lower.contains("jina-reranker")
    {
        "rerank"
    } else if id_lower.contains("tts")
        || id_lower.contains("whisper")
        || id_lower.starts_with("speech")
    {
        "tts"
    } else {
        api_type
    };

    let mut labels: Vec<&str> = Vec::new();
    if id_lower.contains("vision")
        || id_lower.contains("vl-")
        || id_lower.contains("vl_")
        || id_lower.contains("gpt-4o")
    {
        labels.push("视觉");
    }
    if id_lower.contains("gpt")
        || id_lower.contains("claude")
        || id_lower.contains("qwen")
        || id_lower.contains("deepseek")
        || id_lower.contains("glm")
        || id_lower.contains("gemini")
        || id_lower.contains("doubao")
        || id_lower.contains("yi-")
        || id_lower.contains("moonshot")
        || id_lower.contains("mistral")
        || id_lower.contains("llama")
    {
        labels.push("工具");
    }
    if model_type != "embedding" && model_type != "rerank" {
        labels.push("推理");
    }
    if model_type == "embedding" {
        labels.push("嵌入");
    }
    if model_type == "rerank" {
        labels.push("重排");
    }
    if model_type == "tts" {
        labels.push("语音");
    }

    match model_id {
        "qwen3-235b-a22b" | "qwen3-122b-a10b" | "qwen3.5-35b-a3b" => {
            labels = vec!["视觉", "推理", "工具"]
        }
        "deepseek-v4-pro" | "qwen3.7-max" | "qwq-32b" => labels = vec!["推理", "工具"],
        "glm-5" | "glm-5.1" => labels = vec!["推理", "工具", "联网"],
        _ => {}
    }

    (model_type.to_string(), labels.join(","))
}

pub fn parse_remote_models(
    provider_code: &str,
    payload: &serde_json::Value,
    existing_ids: &HashSet<String>,
) -> Result<Vec<RemoteModelDto>, AppError> {
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::Parse("未找到 data 字段".to_string()))?;

    Ok(data
        .iter()
        .filter_map(|item| {
            let model_id = item.get("id")?.as_str()?;
            let context_window = item
                .get("context_window")
                .or_else(|| item.get("context_length"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(8192);
            let (model_type, model_label) = infer_model_info(
                model_id,
                item.get("type").and_then(serde_json::Value::as_str),
            );
            let id = format!("{}-{}", provider_code, model_id);
            Some(RemoteModelDto {
                exists: existing_ids.contains(&id),
                id,
                provider_code: provider_code.to_string(),
                model_id: Some(model_id.to_string()),
                model_name: model_id.to_string(),
                display_name: None,
                model_type,
                model_label: Some(model_label),
                context_window: Some(context_window),
                max_output: None,
                input_price: item.get("input_price").and_then(serde_json::Value::as_f64),
                output_price: item.get("output_price").and_then(serde_json::Value::as_f64),
                is_enabled: Some(false),
                tags: Vec::new(),
                extra_config: None,
                mxcs: None,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{infer_model_info, parse_remote_models};

    #[test]
    fn preserves_model_inference_baseline() {
        assert_eq!(
            infer_model_info("gpt-4o", None),
            ("chat".into(), "视觉,工具,推理".into())
        );
        assert_eq!(
            infer_model_info("text-embedding-3-small", Some("chat")),
            ("embedding".into(), "嵌入".into())
        );
        assert_eq!(
            infer_model_info("bge-reranker-v2", None),
            ("rerank".into(), "重排".into())
        );
        assert_eq!(
            infer_model_info("glm-5", None),
            ("chat".into(), "推理,工具,联网".into())
        );
    }

    #[test]
    fn converts_openai_payload_to_typed_remote_models() {
        let existing = HashSet::from(["OPENAI-gpt-4o".to_string()]);
        let models = parse_remote_models(
            "OPENAI",
            &serde_json::json!({"data": [{"id": "gpt-4o", "context_length": 128000}]}),
            &existing,
        )
        .unwrap();
        assert_eq!(models.len(), 1);
        assert!(models[0].exists);
        assert_eq!(models[0].context_window, Some(128000));
    }
}
use std::collections::HashSet;

use crate::core::error::AppError;
use crate::core::models::RemoteModelDto;
