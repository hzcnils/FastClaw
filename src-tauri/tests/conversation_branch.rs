#[path = "../src/db/repo/conversation_branch.rs"]
mod branch;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations(id, user_id, title) VALUES ('a','owner','A'),('b','owner','B')",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}
async fn message(pool: &SqlitePool, id: &str, content: &str) {
    sqlx::query("INSERT INTO messages(id, conversation_id, role, content, created_at, group_id) VALUES (?, 'a', 'assistant', ?, '2026-09-24T00:00:00Z', 'g')")
        .bind(id).bind(content).execute(pool).await.unwrap();
}
async fn count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM conversations")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn copies_exact_prefix_including_point_in_stable_order_and_remaps_groups() {
    let pool = database().await;
    for (id, body) in [("c", "third"), ("a", "first"), ("b", "second")] {
        message(&pool, id, body).await;
    }
    let id = branch::create(&pool, "a", "b", Some("second"))
        .await
        .unwrap();
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT id, content, group_id FROM messages WHERE conversation_id = ? ORDER BY created_at, id")
        .bind(&id).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!((&*rows[0].1, &*rows[1].1), ("first", "second"));
    assert_eq!(rows[0].2, rows[1].2);
    assert_ne!(rows[0].2, "g");
    for row in rows {
        assert!(uuid::Uuid::parse_str(&row.0).is_ok());
    }
    let conv: (String, String, String, Option<String>) =
        sqlx::query_as("SELECT title,user_id,mode,extra_config FROM conversations WHERE id = ?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        conv,
        ("A - 分支".into(), "owner".into(), "normal".into(), None)
    );
    let updated: String = sqlx::query_scalar("SELECT updated_at FROM conversations WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(updated.contains('T') && updated.ends_with('Z'));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages WHERE conversation_id = 'a'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}

#[tokio::test]
async fn rejects_missing_wrong_conversation_and_changed_content_without_creating_a_branch() {
    let pool = database().await;
    message(&pool, "m", "body").await;
    for (conv, msg, content) in [
        ("missing", "m", "body"),
        ("b", "m", "body"),
        ("a", "missing", "body"),
        ("a", "m", "old"),
    ] {
        assert!(branch::create(&pool, conv, msg, Some(content))
            .await
            .is_err());
    }
    assert_eq!(count(&pool).await, 2);
}

#[tokio::test]
async fn legacy_call_without_snapshot_still_validates_target() {
    let pool = database().await;
    message(&pool, "m", "body").await;
    assert!(branch::create(&pool, "a", "m", None).await.is_ok());
    assert!(branch::create(&pool, "b", "m", None).await.is_err());
    assert_eq!(count(&pool).await, 3);
}

#[tokio::test]
async fn rejects_streaming_anywhere_in_prefix_but_not_after_point() {
    let pool = database().await;
    for id in ["a", "b", "c"] {
        message(&pool, id, id).await;
    }
    sqlx::query("UPDATE messages SET status='streaming' WHERE id='c'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(branch::create(&pool, "a", "b", None).await.is_ok());
    assert!(branch::create(&pool, "a", "c", None).await.is_err());
    sqlx::query("UPDATE messages SET status='streaming' WHERE id='a'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(branch::create(&pool, "a", "b", None).await.is_err());
    assert_eq!(count(&pool).await, 3);
}

#[tokio::test]
async fn copies_beyond_old_500_limit() {
    let pool = database().await;
    for index in 0..505 {
        message(&pool, &format!("m{index:04}"), "body").await;
    }
    let id = branch::create(&pool, "a", "m0503", Some("body"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages WHERE conversation_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        504
    );
}

#[tokio::test]
async fn maps_parents_in_either_direction_and_drops_outside_references() {
    let pool = database().await;
    for id in ["a", "b", "c", "d"] {
        message(&pool, id, id).await;
    }
    sqlx::query("UPDATE messages SET parent_id = CASE id WHEN 'a' THEN 'b' WHEN 'b' THEN 'a' WHEN 'c' THEN 'd' END")
        .execute(&pool).await.unwrap();
    let id = branch::create(&pool, "a", "c", None).await.unwrap();
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT id,parent_id FROM messages WHERE conversation_id = ? ORDER BY created_at,id",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows[0].1.as_ref(), Some(&rows[1].0));
    assert_eq!(rows[1].1.as_ref(), Some(&rows[0].0));
    assert_eq!(rows[2].1, None);
}

#[tokio::test]
async fn copy_failure_rolls_back_conversation_and_all_copied_messages() {
    let pool = database().await;
    message(&pool, "a", "first").await;
    message(&pool, "b", "second").await;
    sqlx::query("CREATE TRIGGER fail_branch BEFORE INSERT ON messages WHEN NEW.conversation_id != 'a' AND NEW.content = 'second' BEGIN SELECT RAISE(ABORT, 'test failure'); END")
        .execute(&pool).await.unwrap();
    assert!(branch::create(&pool, "a", "b", None).await.is_err());
    assert_eq!(count(&pool).await, 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn copies_message_fields_but_not_inflight_translation_ticket() {
    let pool = database().await;
    message(&pool, "m", "body").await;
    sqlx::query("UPDATE messages SET translation='translated',translation_lang='en',translation_request_id='old',content_revision=7,attachments='[]',think_content='think',extra_data='{}',kb_references='[]',duration_ms=12,error_message='detail',is_favorite=1,model_instance_id='instance' WHERE id='m'")
        .execute(&pool).await.unwrap();
    let id = branch::create(&pool, "a", "m", None).await.unwrap();
    let values: (String,String,String,String,String,i64,String,i64,String) = sqlx::query_as("SELECT translation,translation_lang,attachments,think_content,kb_references,duration_ms,error_message,is_favorite,model_instance_id FROM messages WHERE conversation_id = ?")
        .bind(&id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        values,
        (
            "translated".into(),
            "en".into(),
            "[]".into(),
            "think".into(),
            "[]".into(),
            12,
            "detail".into(),
            1,
            "instance".into()
        )
    );
    let revision: (i64, Option<String>) = sqlx::query_as(
        "SELECT content_revision,translation_request_id FROM messages WHERE conversation_id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revision, (0, None));
}

#[tokio::test]
async fn ignored_copy_also_rolls_back_instead_of_reporting_partial_success() {
    let pool = database().await;
    message(&pool, "m", "body").await;
    sqlx::query("CREATE TRIGGER ignore_branch BEFORE INSERT ON messages WHEN NEW.conversation_id != 'a' BEGIN SELECT RAISE(IGNORE); END")
        .execute(&pool).await.unwrap();
    assert!(branch::create(&pool, "a", "m", None).await.is_err());
    assert_eq!(count(&pool).await, 2);
}

#[tokio::test]
async fn chronological_boundary_precedes_id_sorting() {
    let pool = database().await;
    for id in ["a", "b", "z"] {
        message(&pool, id, id).await;
    }
    sqlx::query("UPDATE messages SET created_at='2026-09-23T00:00:00Z' WHERE id='z'")
        .execute(&pool)
        .await
        .unwrap();
    let id = branch::create(&pool, "a", "a", None).await.unwrap();
    let content: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM messages WHERE conversation_id = ? ORDER BY created_at, id",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(content, vec!["z", "a"]);
}
