// FastClaw · 统一错误类型
// 所有 Tauri Command 返回 Result<T, AppError>

use serde::Serialize;

/// 统一错误类型，可序列化给前端
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("数据库错误: {0}")]
    DatabaseMsg(String),

    #[error("LLM 调用错误: {0}")]
    Llm(String),

    #[error("WSP 服务错误: {0}")]
    Wsp(String),

    #[error("候选端口 {first} 至 {last} 均不可用，请修改首选端口后重试")]
    DependencyPortExhausted { first: u16, last: u16 },

    #[error("MCP 错误: {0}")]
    Mcp(String),

    #[error("Skill 错误: {0}")]
    Skill(String),

    #[error("规划引擎错误: {0}")]
    Plan(String),

    #[error("参数校验错误: {0}")]
    Validation(String),

    #[error("业务错误: {0}")]
    Business(String),

    #[error("参数错误: {0}")]
    InvalidInput(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("内部错误: {0}")]
    Internal(#[from] anyhow::Error),
}

// 实现 Serialize 以便 Tauri 可以返回给前端
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", &self.error_code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl AppError {
    /// 错误码，前端可据此做不同处理
    fn error_code(&self) -> &str {
        match self {
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::DatabaseMsg(_) => "DATABASE_ERROR",
            AppError::Llm(_) => "LLM_ERROR",
            AppError::Wsp(_) => "WSP_ERROR",
            AppError::DependencyPortExhausted { .. } => "PORT_RANGE_EXHAUSTED",
            AppError::Mcp(_) => "MCP_ERROR",
            AppError::Skill(_) => "SKILL_ERROR",
            AppError::Plan(_) => "PLAN_ERROR",
            AppError::Validation(_) => "VALIDATION_ERROR",
            AppError::Business(_) => "BUSINESS_ERROR",
            AppError::InvalidInput(_) => "INVALID_INPUT",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Network(_) => "NETWORK_ERROR",
            AppError::Parse(_) => "PARSE_ERROR",
            AppError::Io(_) => "IO_ERROR",
            AppError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}
