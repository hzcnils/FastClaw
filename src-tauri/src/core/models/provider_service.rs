use sqlx::SqlitePool;

use crate::core::error::AppError;

pub fn normalize_api_key(api_key: Option<String>) -> Option<String> {
    api_key.filter(|key| !key.trim().is_empty())
}

pub fn normalize_provider_code(name: &str) -> String {
    name.to_uppercase()
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | ' '))
        .collect()
}

pub async fn generate_unique_provider_code(
    pool: &SqlitePool,
    name: &str,
) -> Result<String, AppError> {
    let base_code = normalize_provider_code(name);
    let mut code = base_code.clone();
    let mut suffix = 2u32;
    loop {
        let exists = sqlx::query_scalar::<_, String>(
            "SELECT id FROM model_providers WHERE provider_code = ?",
        )
        .bind(&code)
        .fetch_optional(pool)
        .await?;
        if exists.is_none() {
            return Ok(code);
        }
        code = format!("{}_{}", base_code, suffix);
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_api_key, normalize_provider_code};

    #[test]
    fn normalizes_provider_code_without_separators() {
        assert_eq!(
            normalize_provider_code("My-Open_AI Provider"),
            "MYOPENAIPROVIDER"
        );
    }

    #[test]
    fn normalizes_blank_api_keys_to_none() {
        assert_eq!(normalize_api_key(Some("  ".into())), None);
        assert_eq!(
            normalize_api_key(Some("sk-full".into())),
            Some("sk-full".into())
        );
    }
}
