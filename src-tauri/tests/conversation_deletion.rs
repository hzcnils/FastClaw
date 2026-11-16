#[path = "../src/db/repo/conversation_deletion.rs"]
mod deletion;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    sqlx::query(
        "INSERT INTO conversations(id,user_id,title) VALUES ('a','owner','A'),('b','owner','B')",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (id, conversation) in [("a-user", "a"), ("a-answer", "a"), ("b-answer", "b")] {
        add(&pool, id, conversation).await;
    }
    pool
}
async fn add(pool: &SqlitePool, id: &str, conversation: &str) {
    let role = if id.ends_with("-user") {
        "user"
    } else {
        "assistant"
    };
    sqlx::query("INSERT INTO messages(id,conversation_id,role,content,group_id) VALUES (?,?,?,'body','shared-group')")
        .bind(id).bind(conversation).bind(role).execute(pool).await.unwrap();
}
async fn snapshot(pool: &SqlitePool) -> (Vec<String>, Vec<String>) {
    (
        sqlx::query_scalar("SELECT id FROM conversations ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT id FROM messages ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap(),
    )
}
async fn dependencies(pool: &SqlitePool) {
    sqlx::query("INSERT INTO message_attachments(id,message_id,file_name,file_path,file_size,mime_type,file_type) VALUES ('attachment','a-answer','file','unused-path',1,'text/plain','document')")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO quality_evaluations(id,message_id,conversation_id,score) VALUES ('quality','a-answer','a',5)")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO model_feedback(id,message_id,model_instance_id,signal) VALUES ('feedback','a-answer','model',1)")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO message_favorites(id,message_id,conversation_id,role,content) VALUES ('favorite','a-answer','a','assistant','favorite snapshot')")
        .execute(pool).await.unwrap();
}
async fn dependency_counts(pool: &SqlitePool) -> (i64, i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT COUNT(*) FROM message_attachments), (SELECT COUNT(*) FROM quality_evaluations), (SELECT COUNT(*) FROM model_feedback), (SELECT COUNT(*) FROM message_favorites)")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn deletes_all_pages_and_only_the_exact_conversation() {
    let pool = database().await;
    for index in 0..70 {
        add(&pool, &format!("history-{index}"), "a").await;
    }
    deletion::remove(&pool, "a").await.unwrap();
    assert_eq!(
        snapshot(&pool).await,
        (vec!["b".into()], vec!["b-answer".into()])
    );
}

#[tokio::test]
async fn second_statement_failure_restores_messages_and_cascaded_dependencies() {
    let pool = database().await;
    dependencies(&pool).await;
    let before = snapshot(&pool).await;
    sqlx::query("CREATE TRIGGER fail_conversation BEFORE DELETE ON conversations WHEN OLD.id='a' BEGIN SELECT RAISE(ABORT,'test failure'); END")
        .execute(&pool).await.unwrap();
    assert!(deletion::remove(&pool, "a").await.is_err());
    assert_eq!(snapshot(&pool).await, before);
    assert_eq!(dependency_counts(&pool).await, (1, 1, 1, 1));
}

#[tokio::test]
async fn first_statement_failure_keeps_both_conversations_and_messages() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    sqlx::query("CREATE TRIGGER fail_message BEFORE DELETE ON messages WHEN OLD.conversation_id='a' BEGIN SELECT RAISE(ABORT,'test failure'); END")
        .execute(&pool).await.unwrap();
    assert!(deletion::remove(&pool, "a").await.is_err());
    assert_eq!(snapshot(&pool).await, before);
}

#[tokio::test]
async fn commit_failure_also_restores_the_entire_conversation() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    sqlx::query("CREATE TABLE commit_guard(conversation_id TEXT REFERENCES conversations(id) DEFERRABLE INITIALLY DEFERRED)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO commit_guard(conversation_id) VALUES ('a')")
        .execute(&pool)
        .await
        .unwrap();
    // Both DELETEs succeed; only COMMIT fails on the deferred foreign key.
    assert!(deletion::remove(&pool, "a").await.is_err());
    assert_eq!(snapshot(&pool).await, before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM commit_guard")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn ignored_message_delete_rolls_back_instead_of_leaving_orphans() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    sqlx::query("CREATE TRIGGER ignore_message BEFORE DELETE ON messages WHEN OLD.id='a-answer' BEGIN SELECT RAISE(IGNORE); END")
        .execute(&pool).await.unwrap();
    assert!(deletion::remove(&pool, "a")
        .await
        .unwrap_err()
        .to_string()
        .contains("会话删除不完整"));
    assert_eq!(snapshot(&pool).await, before);
}

#[tokio::test]
async fn ignored_conversation_delete_rolls_back_instead_of_emptying_history() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    sqlx::query("CREATE TRIGGER ignore_conversation BEFORE DELETE ON conversations WHEN OLD.id='a' BEGIN SELECT RAISE(IGNORE); END")
        .execute(&pool).await.unwrap();
    assert!(deletion::remove(&pool, "a").await.is_err());
    assert_eq!(snapshot(&pool).await, before);
}

#[tokio::test]
async fn successful_delete_cascades_metadata_but_retains_independent_favorite_snapshot() {
    let pool = database().await;
    dependencies(&pool).await;
    deletion::remove(&pool, "a").await.unwrap();
    assert_eq!(dependency_counts(&pool).await, (0, 0, 0, 1));
    let favorite: (String, String) =
        sqlx::query_as("SELECT conversation_id,content FROM message_favorites WHERE id='favorite'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(favorite, ("a".into(), "favorite snapshot".into()));
}

#[tokio::test]
async fn empty_missing_and_repeated_deletes_remain_idempotent() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    deletion::remove(&pool, "missing").await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
    sqlx::query("INSERT INTO conversations(id,user_id,title) VALUES ('empty','owner','Empty')")
        .execute(&pool)
        .await
        .unwrap();
    deletion::remove(&pool, "empty").await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
    deletion::remove(&pool, "a").await.unwrap();
    let after = snapshot(&pool).await;
    deletion::remove(&pool, "a").await.unwrap();
    assert_eq!(snapshot(&pool).await, after);
}

#[tokio::test]
async fn existing_orphan_messages_are_removed_without_requiring_a_parent() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    add(&pool, "orphan", "missing-parent").await;
    deletion::remove(&pool, "missing-parent").await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
}

#[tokio::test]
async fn sql_like_ids_are_bound_as_exact_values() {
    let pool = database().await;
    let before = snapshot(&pool).await;
    let id = "a' OR 1=1 --";
    sqlx::query("INSERT INTO conversations(id,user_id,title) VALUES (?,'owner','Special')")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    add(&pool, "special-message", id).await;
    deletion::remove(&pool, id).await.unwrap();
    assert_eq!(snapshot(&pool).await, before);
}

#[tokio::test]
async fn repository_does_not_override_callers_execution_cancellation_policy() {
    let pool = database().await;
    sqlx::query("UPDATE messages SET status='streaming' WHERE conversation_id='a'")
        .execute(&pool)
        .await
        .unwrap();
    deletion::remove(&pool, "a").await.unwrap();
    assert_eq!(
        snapshot(&pool).await,
        (vec!["b".into()], vec!["b-answer".into()])
    );
}
