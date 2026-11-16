#[path = "../src/db/repo/message_deletion.rs"]
mod deletion;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversations(id,user_id,title) VALUES ('a','default-user','A'),('b','default-user','B')").execute(&pool).await.unwrap();
    add(&pool, "u", "user", Some("g"), "a").await;
    add(&pool, "m", "assistant", Some("g"), "a").await;
    pool
}
async fn add(pool: &SqlitePool, id: &str, role: &str, group: Option<&str>, conversation: &str) {
    sqlx::query(
        "INSERT INTO messages(id,conversation_id,role,content,group_id) VALUES (?,?,?,'body',?)",
    )
    .bind(id)
    .bind(conversation)
    .bind(role)
    .bind(group)
    .execute(pool)
    .await
    .unwrap();
}
async fn ids(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM messages ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}
#[tokio::test]
async fn deletes_last_answer_and_its_exact_question() {
    let pool = database().await;
    assert_eq!(
        deletion::remove(&pool, "a", "m", "body").await.unwrap(),
        vec!["m", "u"]
    );
    assert!(ids(&pool).await.is_empty());
}
#[tokio::test]
async fn keeps_question_when_sibling_exists_outside_loaded_page() {
    let pool = database().await;
    add(&pool, "old-sibling", "assistant", Some("g"), "a").await;
    for index in 0..55 {
        add(
            &pool,
            &format!("later-{index}"),
            "assistant",
            Some("other"),
            "a",
        )
        .await;
    }
    assert_eq!(
        deletion::remove(&pool, "a", "m", "body").await.unwrap(),
        vec!["m"]
    );
    assert!(ids(&pool).await.contains(&"u".into()));
}
#[tokio::test]
async fn reused_group_in_another_conversation_is_never_deleted_or_counted_as_sibling() {
    let pool = database().await;
    add(&pool, "b-user", "user", Some("g"), "b").await;
    add(&pool, "b-answer", "assistant", Some("g"), "b").await;
    assert_eq!(
        deletion::remove(&pool, "a", "m", "body").await.unwrap(),
        vec!["m", "u"]
    );
    assert_eq!(ids(&pool).await, vec!["b-answer", "b-user"]);
}
#[tokio::test]
async fn no_group_does_not_guess_neighbour_question() {
    let pool = database().await;
    sqlx::query("UPDATE messages SET group_id=NULL WHERE id='m'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        deletion::remove(&pool, "a", "m", "body").await.unwrap(),
        vec!["m"]
    );
    assert_eq!(ids(&pool).await, vec!["u"]);
}
#[tokio::test]
async fn ambiguous_or_missing_group_question_is_not_guessed() {
    let pool = database().await;
    add(&pool, "u2", "user", Some("g"), "a").await;
    assert_eq!(
        deletion::remove(&pool, "a", "m", "body").await.unwrap(),
        vec!["m"]
    );
    add(&pool, "orphan", "assistant", Some("missing"), "a").await;
    assert_eq!(
        deletion::remove(&pool, "a", "orphan", "body")
            .await
            .unwrap(),
        vec!["orphan"]
    );
    assert_eq!(ids(&pool).await, vec!["u", "u2"]);
}
#[tokio::test]
async fn direct_question_deletion_preserves_existing_single_target_semantics() {
    let pool = database().await;
    assert_eq!(
        deletion::remove(&pool, "a", "u", "body").await.unwrap(),
        vec!["u"]
    );
    assert_eq!(ids(&pool).await, vec!["m"]);
}
#[tokio::test]
async fn rejects_stale_content_wrong_conversation_and_missing_target() {
    let pool = database().await;
    for (conv, id, body) in [
        ("b", "m", "body"),
        ("a", "m", "changed"),
        ("a", "missing", "body"),
    ] {
        assert!(deletion::remove(&pool, conv, id, body).await.is_err());
    }
    assert_eq!(ids(&pool).await, vec!["m", "u"]);
}
#[tokio::test]
async fn streaming_in_same_conversation_blocks_delete_but_other_conversation_does_not() {
    let pool = database().await;
    add(&pool, "stream", "assistant", None, "a").await;
    sqlx::query("UPDATE messages SET status='streaming' WHERE id='stream'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(deletion::remove(&pool, "a", "m", "body").await.is_err());
    sqlx::query("UPDATE messages SET conversation_id='b' WHERE id='stream'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        deletion::remove(&pool, "a", "m", "body").await.unwrap(),
        vec!["m", "u"]
    );
}
#[tokio::test]
async fn second_delete_failure_rolls_back_answer_and_question() {
    let pool = database().await;
    sqlx::query("CREATE TRIGGER fail_delete BEFORE DELETE ON messages WHEN OLD.id='u' BEGIN SELECT RAISE(ABORT,'test failure'); END").execute(&pool).await.unwrap();
    assert!(deletion::remove(&pool, "a", "m", "body").await.is_err());
    assert_eq!(ids(&pool).await, vec!["m", "u"]);
}
#[tokio::test]
async fn ignored_delete_also_rolls_back_instead_of_returning_false_success() {
    let pool = database().await;
    sqlx::query("CREATE TRIGGER ignore_delete BEFORE DELETE ON messages WHEN OLD.id='u' BEGIN SELECT RAISE(IGNORE); END").execute(&pool).await.unwrap();
    assert!(deletion::remove(&pool, "a", "m", "body").await.is_err());
    assert_eq!(ids(&pool).await, vec!["m", "u"]);
}
