#[path = "../src/db/repo/quality_feedback.rs"]
mod feedback;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversations(id, user_id, title) VALUES ('a', 'default-user', 'A'), ('b', 'default-user', 'B')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO messages(id, conversation_id, role, content) VALUES ('m', 'a', 'assistant', 'answer')")
        .execute(&pool).await.unwrap();
    pool
}
async fn count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM quality_evaluations")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn inserts_feedback_for_the_exact_target_with_trimmed_content() {
    let pool = database().await;
    assert!(
        feedback::submit(&pool, "m", "a", 8.5, " useful ", Some("answer"))
            .await
            .unwrap()
    );
    let row: (String, String, f64, String) = sqlx::query_as(
        "SELECT message_id, conversation_id, score, content FROM quality_evaluations",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, ("m".into(), "a".into(), 8.5, "useful".into()));
}
#[tokio::test]
async fn rejects_missing_wrong_conversation_and_changed_source() {
    let pool = database().await;
    for (id, conversation, original) in [
        ("missing", "a", "answer"),
        ("m", "b", "answer"),
        ("m", "a", "old"),
    ] {
        assert!(
            !feedback::submit(&pool, id, conversation, 5.0, "feedback", Some(original))
                .await
                .unwrap()
        );
    }
    assert_eq!(count(&pool).await, 0);
}
#[tokio::test]
async fn rejects_invalid_scores_and_empty_unscored_feedback() {
    let pool = database().await;
    for score in [-1.0, 11.0, f64::NAN, f64::INFINITY] {
        assert!(!feedback::submit(&pool, "m", "a", score, "feedback", None)
            .await
            .unwrap());
    }
    assert!(!feedback::submit(&pool, "m", "a", 0.0, "  ", None)
        .await
        .unwrap());
    assert_eq!(count(&pool).await, 0);
}
#[tokio::test]
async fn accepts_score_only_and_legacy_callers_but_still_checks_conversation() {
    let pool = database().await;
    assert!(feedback::submit(&pool, "m", "a", 10.0, "", None)
        .await
        .unwrap());
    assert!(feedback::submit(&pool, "m", "a", 0.0, "text only", None)
        .await
        .unwrap());
    assert!(!feedback::submit(&pool, "m", "b", 1.0, "", None)
        .await
        .unwrap());
    assert_eq!(count(&pool).await, 2);
}
#[tokio::test]
async fn deleted_message_cannot_create_an_orphan_evaluation() {
    let pool = database().await;
    sqlx::query("DELETE FROM messages WHERE id = 'm'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !feedback::submit(&pool, "m", "a", 5.0, "feedback", Some("answer"))
            .await
            .unwrap()
    );
    assert_eq!(count(&pool).await, 0);
}
