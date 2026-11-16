use serde::Serialize;
use tauri::State;

use crate::core::error::AppError;
use crate::state::AppState;

/// 获取所有模型的评分聚合（按 scope 区分全局/个人）
#[tauri::command]
pub async fn get_model_ratings(
    state: State<'_, AppState>,
    scope: Option<String>,
    user_id: Option<String>,
) -> Result<Vec<crate::db::repo::model_feedback::ModelRating>, AppError> {
    let pool = state.db.pool().await;
    let scope = scope.unwrap_or_else(|| "global".into());
    let ratings =
        crate::db::repo::model_feedback::get_ratings(&pool, &scope, user_id.as_deref()).await?;
    Ok(ratings)
}

#[derive(Serialize)]
pub struct RankedModel {
    pub rank: usize,
    pub model_id: String,
    pub provider_code: String,
    pub likes: i64,
    pub dislikes: i64,
    pub score: f64,
    pub tier: String,
}

#[tauri::command]
pub async fn get_model_ranking(state: State<'_, AppState>) -> Result<Vec<RankedModel>, AppError> {
    let pool = state.db.pool().await;
    let ratings = crate::db::repo::model_feedback::get_ratings(&pool, "global", None).await?;
    // 分区：≥3 票入主榜，其余为新晋
    let (mut ranked, _newcomers): (Vec<_>, Vec<_>) =
        ratings.into_iter().partition(|r| r.likes + r.dislikes >= 3);
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.likes.cmp(&a.likes))
    });
    // 并列名次：同分同名次
    let mut last_score = f64::NAN;
    let mut last_rank = 0usize;
    Ok(ranked
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let rank = if (r.score - last_score).abs() < f64::EPSILON {
                last_rank
            } else {
                last_score = r.score;
                last_rank = i + 1;
                last_rank
            };
            RankedModel {
                rank,
                model_id: r.model_id,
                provider_code: r.provider_code,
                likes: r.likes,
                dislikes: r.dislikes,
                score: r.score,
                tier: r.tier,
            }
        })
        .collect())
}
