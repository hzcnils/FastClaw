//! BERT 意图路由模块
//!
//! 在 LLM 调用前使用 DistilBERT 嵌入匹配将简单问题直接路由到 MCP 工具，
//! 跳过 LLM 调用以降低延迟和 token 消耗。
//!
//! # 架构
//! ```text
//! 用户消息 → IntentRouter::route()
//!   ├── 高置信度 → 直接调 MCP 工具
//!   ├── 中置信度 → LLM + 工具调用
//!   └── 低置信度 → 纯 LLM 流程
//! ```

pub mod embedding;
pub mod model;

use anyhow::Result;
use model::IntentModel;
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::core::mcp::SharedMcpManager;

// ── 配置 ──────────────────────────────────────────────────────────────

/// 意图路由配置
#[derive(Debug, Clone)]
pub struct IntentConfig {
    /// 是否启用 BERT 路由
    pub enabled: bool,

    /// 直接工具调用阈值（高于此值直接调 MCP 工具，默认 0.75）
    pub threshold_high: f32,

    /// LLM+工具阈值（高于此值走 LLM 工具调用流程，默认 0.5）
    pub threshold_low: f32,

    /// 最多匹配几个候选工具
    pub max_candidates: usize,
}

impl Default for IntentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_high: 0.75,
            threshold_low: 0.5,
            max_candidates: 3,
        }
    }
}

// ── 数据结构 ─────────────────────────────────────────────────────────

/// 工具嵌入条目
#[derive(Debug, Clone)]
pub struct ToolEmbedding {
    pub server_id: String,
    pub tool_name: String,
    pub description: String,
    pub embedding: Vec<f32>,
    pub input_schema: Value,
}

/// 路由决策
#[derive(Debug, Clone)]
pub enum RouteDecision {
    /// 直接调用 MCP 工具（高置信度匹配）
    DirectTool {
        server_id: String,
        tool_name: String,
        arguments: Value,
        score: f32,
    },
    /// 走 LLM + 工具调用流程（中置信度）
    LlmWithTools { candidate_tools: Vec<ToolCandidate> },
    /// 走纯 LLM 流程
    LlmOnly,
}

/// 候选工具
#[derive(Debug, Clone)]
pub struct ToolCandidate {
    pub server_id: String,
    pub tool_name: String,
    pub score: f32,
}

// ── IntentRouter ──────────────────────────────────────────────────────

/// 意图路由器
///
/// 用 BERT 嵌入匹配将用户消息路由到 MCP 工具或 LLM。
/// 所有公共方法都是同步的（BERT 推理是 CPU 密集操作，不需要 async）。
pub struct IntentRouter {
    model: Arc<IntentModel>,
    tool_embeddings: RwLock<Vec<ToolEmbedding>>,
    config: IntentConfig,
}

impl IntentRouter {
    /// 创建路由器
    pub fn new(model: IntentModel, config: IntentConfig) -> Self {
        Self {
            model: Arc::new(model),
            tool_embeddings: RwLock::new(Vec::new()),
            config,
        }
    }

    /// 从模型目录加载并创建路由器
    pub fn load(model_dir: &Path, config: IntentConfig) -> Result<Self> {
        let model = IntentModel::load(model_dir)?;
        Ok(Self::new(model, config))
    }

    /// 对用户消息做意图路由决策
    pub fn route(&self, message: &str) -> RouteDecision {
        if !self.config.enabled {
            return RouteDecision::LlmOnly;
        }

        let embeddings = self.tool_embeddings.read().unwrap();
        if embeddings.is_empty() {
            return RouteDecision::LlmOnly;
        }

        // 1. 嵌入用户消息
        let query_emb = match self.model.embed(message) {
            Ok(emb) => emb,
            Err(e) => {
                tracing::warn!("BERT 嵌入失败: {e}, 降级到 LLM");
                return RouteDecision::LlmOnly;
            }
        };

        // 2. 与所有工具描述做相似度匹配
        let candidate_embs: Vec<Vec<f32>> =
            embeddings.iter().map(|t| t.embedding.clone()).collect();

        let matches =
            embedding::find_best_matches(&query_emb, &candidate_embs, self.config.max_candidates);

        if matches.is_empty() {
            return RouteDecision::LlmOnly;
        }

        let (best_idx, best_score) = matches[0];
        let best_tool = &embeddings[best_idx];

        let preview: String = message.chars().take(50).collect();
        tracing::info!(
            "意图匹配: \"{}\" → {}::{} (score={:.3})",
            preview,
            best_tool.server_id,
            best_tool.tool_name,
            best_score,
        );

        // 3. 根据分数决定路由
        if best_score >= self.config.threshold_high {
            let args = self.extract_arguments(message, &best_tool.input_schema);
            RouteDecision::DirectTool {
                server_id: best_tool.server_id.clone(),
                tool_name: best_tool.tool_name.clone(),
                arguments: args,
                score: best_score,
            }
        } else if best_score >= self.config.threshold_low {
            let candidates = matches
                .iter()
                .map(|(idx, score)| ToolCandidate {
                    server_id: embeddings[*idx].server_id.clone(),
                    tool_name: embeddings[*idx].tool_name.clone(),
                    score: *score,
                })
                .collect();
            RouteDecision::LlmWithTools {
                candidate_tools: candidates,
            }
        } else {
            RouteDecision::LlmOnly
        }
    }

    /// 从 MCP 管理器刷新工具嵌入缓存（需在 tokio 上下文中调用）
    pub fn refresh_tool_embeddings(&self, mcp_manager: &SharedMcpManager) {
        let manager = match mcp_manager.try_read() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("MCP 管理器被锁定，跳过工具嵌入刷新");
                return;
            }
        };

        let tools = manager.list_all_tools();
        if tools.is_empty() {
            let mut guard = self.tool_embeddings.write().unwrap();
            guard.clear();
            return;
        }

        let descriptions: Vec<String> = tools
            .iter()
            .map(|(_, tool)| {
                // 组合工具名 + 描述以获得更好的嵌入
                format!("{}: {}", tool.name, tool.description)
            })
            .collect();
        let desc_refs: Vec<&str> = descriptions.iter().map(|s| s.as_str()).collect();

        let embeddings = match self.model.embed_batch(&desc_refs) {
            Ok(embs) => embs,
            Err(e) => {
                tracing::error!("工具描述嵌入失败: {e}");
                return;
            }
        };

        let mut new_embeddings = Vec::with_capacity(tools.len());
        for ((server_id, tool), embedding) in tools.iter().zip(embeddings) {
            new_embeddings.push(ToolEmbedding {
                server_id: server_id.clone(),
                tool_name: tool.name.clone(),
                description: tool.description.clone(),
                embedding,
                input_schema: tool.input_schema.clone(),
            });
        }

        let count = new_embeddings.len();
        let mut guard = self.tool_embeddings.write().unwrap();
        *guard = new_embeddings;

        tracing::info!("工具嵌入已刷新: {} 个 MCP 工具", count);
    }

    /// 获取配置
    pub fn config(&self) -> &IntentConfig {
        &self.config
    }

    /// 获取当前缓存的工具数量
    pub fn tool_count(&self) -> usize {
        self.tool_embeddings.read().unwrap().len()
    }

    // ── 私有方法 ──────────────────────────────────────────────────────

    /// 从用户消息中提取参数（MVP: 规则匹配）
    fn extract_arguments(&self, message: &str, schema: &Value) -> Value {
        let mut args = serde_json::Map::new();

        let props = match schema.get("properties") {
            Some(Value::Object(props)) => props,
            _ => return Value::Object(args),
        };

        let required: Vec<&str> = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        for (prop_name, prop_schema) in props {
            let is_required = required.contains(&prop_name.as_str());
            let prop_type = prop_schema
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string");

            let value = match prop_type {
                "string" => self.extract_string_param(message, prop_name, prop_schema),
                "number" | "integer" => self.extract_number_param(message, prop_name),
                _ => None,
            };

            match value {
                Some(v) => {
                    args.insert(prop_name.clone(), v);
                }
                None if is_required => {
                    tracing::debug!(
                        "参数提取失败: required 字段 '{}' 无法提取, 降级到 LLM",
                        prop_name
                    );
                    return Value::Object(serde_json::Map::new());
                }
                _ => {}
            }
        }

        Value::Object(args)
    }

    fn extract_string_param(
        &self,
        message: &str,
        prop_name: &str,
        _prop_schema: &Value,
    ) -> Option<Value> {
        match prop_name {
            "city" | "location" | "城市" | "地点" | "地址" => {
                let cities = [
                    "北京",
                    "上海",
                    "广州",
                    "深圳",
                    "杭州",
                    "南京",
                    "成都",
                    "武汉",
                    "西安",
                    "重庆",
                    "天津",
                    "苏州",
                    "长沙",
                    "郑州",
                    "青岛",
                    "大连",
                    "厦门",
                    "福州",
                    "合肥",
                    "沈阳",
                    "哈尔滨",
                    "昆明",
                    "贵阳",
                    "兰州",
                    "南宁",
                    "海口",
                    "拉萨",
                    "银川",
                    "西宁",
                    "乌鲁木齐",
                    "呼和浩特",
                    "石家庄",
                    "太原",
                    "长春",
                    "济南",
                    "南昌",
                    "宁波",
                    "无锡",
                    "佛山",
                    "东莞",
                    "温州",
                    "常州",
                ];
                for city in cities {
                    if message.contains(city) {
                        return Some(Value::String(city.to_string()));
                    }
                }
                None
            }
            "name" | "名称" | "query" | "查询" | "搜索" | "keyword" | "关键词" | "content"
            | "内容" => {
                if let Some(start) = message.find('"') {
                    if let Some(end) = message[start + 1..].find('"') {
                        return Some(Value::String(
                            message[start + 1..start + 1 + end].to_string(),
                        ));
                    }
                }
                None
            }
            "date" | "日期" | "时间" | "datetime" => {
                if message.contains("今天") {
                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                    return Some(Value::String(today));
                }
                if message.contains("明天") {
                    let tomorrow = chrono::Local::now() + chrono::Duration::days(1);
                    return Some(Value::String(tomorrow.format("%Y-%m-%d").to_string()));
                }
                None
            }
            _ => None,
        }
    }

    fn extract_number_param(&self, message: &str, prop_name: &str) -> Option<Value> {
        let number_words = [
            ("一", 1),
            ("二", 2),
            ("两", 2),
            ("三", 3),
            ("四", 4),
            ("五", 5),
            ("六", 6),
            ("七", 7),
            ("八", 8),
            ("九", 9),
            ("十", 10),
        ];

        for (word, num) in number_words {
            if message.contains(&format!("{word}{prop_name}"))
                || message.contains(&format!("{prop_name}{word}"))
            {
                return Some(Value::Number(serde_json::Number::from(num)));
            }
        }

        for ch in message.chars() {
            if ch.is_ascii_digit() {
                if let Ok(n) = ch.to_string().parse::<i64>() {
                    return Some(Value::Number(serde_json::Number::from(n)));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dummy_router() -> IntentRouter {
        IntentRouter {
            model: Arc::new(panic!("not used in tests")), // won't be called
            tool_embeddings: RwLock::new(vec![]),
            config: IntentConfig::default(),
        }
    }

    #[test]
    fn test_extract_city() {
        let router = dummy_router();
        let schema = json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "城市名称" }
            },
            "required": ["city"]
        });

        let args = router.extract_arguments("今天西安的天气咋样", &schema);
        assert_eq!(args["city"], json!("西安"));

        let args = router.extract_arguments("北京明天会下雨吗", &schema);
        assert_eq!(args["city"], json!("北京"));

        let args = router.extract_arguments("今天天气真好啊", &schema);
        assert_eq!(args.as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_extract_date() {
        let router = dummy_router();
        let schema = json!({
            "type": "object",
            "properties": {
                "date": { "type": "string", "description": "日期" }
            },
            "required": ["date"]
        });

        let args = router.extract_arguments("今天天气", &schema);
        assert!(args["date"].as_str().is_some());
    }

    #[test]
    fn test_route_llm_only_when_empty_tools() {
        let router = dummy_router();
        let decision = router.route("今天天气怎么样");
        assert!(matches!(decision, RouteDecision::LlmOnly));
    }
}
