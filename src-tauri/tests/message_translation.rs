#[path = "../src/db/repo/message_translation.rs"]
mod translation;

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL, content TEXT NOT NULL, \
         translation TEXT, translation_lang TEXT, is_liked INTEGER DEFAULT 0); \
         INSERT INTO messages(id, conversation_id, content, translation, translation_lang) \
         VALUES ('m', 'a', 'source', 'existing', 'en');",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20260924000001_message_translation_revision.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn saved(pool: &SqlitePool) -> (Option<String>, Option<String>, i64, Option<String>) {
    sqlx::query_as("SELECT translation, translation_lang, content_revision, translation_request_id FROM messages WHERE id = 'm'")
        .fetch_one(pool).await.unwrap()
}

async fn claim(pool: &SqlitePool) -> translation::TranslationTicket {
    translation::begin(pool, "m", "a", "source")
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn migration_preserves_existing_data_and_current_claim_finishes_only_once() {
    let pool = database().await;
    assert_eq!(
        saved(&pool).await,
        (Some("existing".into()), Some("en".into()), 0, None)
    );
    let ticket = claim(&pool).await;
    assert!(translation::finish(&pool, &ticket, "new", "ja")
        .await
        .unwrap());
    assert!(!translation::finish(&pool, &ticket, "duplicate", "fr")
        .await
        .unwrap());
    assert_eq!(
        saved(&pool).await,
        (Some("new".into()), Some("ja".into()), 0, None)
    );
}

#[tokio::test]
async fn changed_body_clears_saved_translation_and_rejects_old_result() {
    let pool = database().await;
    let ticket = claim(&pool).await;
    sqlx::query("UPDATE messages SET content = 'edited' WHERE id = 'm'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!translation::finish(&pool, &ticket, "obsolete", "en")
        .await
        .unwrap());
    assert_eq!(saved(&pool).await, (None, None, 1, None));
}

#[tokio::test]
async fn changing_back_to_the_original_body_does_not_revive_a_claim() {
    let pool = database().await;
    let ticket = claim(&pool).await;
    sqlx::raw_sql(
        "UPDATE messages SET content = 'edited'; UPDATE messages SET content = 'source';",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(!translation::finish(&pool, &ticket, "obsolete", "en")
        .await
        .unwrap());
    assert_eq!(saved(&pool).await, (None, None, 2, None));
    let fresh = claim(&pool).await;
    assert!(translation::finish(&pool, &fresh, "fresh", "en")
        .await
        .unwrap());
}

#[tokio::test]
async fn newer_registration_wins_across_independent_callers_in_either_completion_order() {
    for newer_first in [true, false] {
        let pool = database().await;
        let first = claim(&pool).await;
        let second = claim(&pool.clone()).await;
        if newer_first {
            assert!(translation::finish(&pool, &second, "new", "zh")
                .await
                .unwrap());
            assert!(!translation::finish(&pool, &first, "old", "en")
                .await
                .unwrap());
        } else {
            assert!(!translation::finish(&pool, &first, "old", "en")
                .await
                .unwrap());
            assert!(translation::finish(&pool, &second, "new", "zh")
                .await
                .unwrap());
        }
        assert_eq!(saved(&pool).await.0.as_deref(), Some("new"));
        assert_eq!(saved(&pool).await.1.as_deref(), Some("zh"));
    }
}

#[tokio::test]
async fn late_failure_cannot_clear_newer_claim_and_newer_failure_cannot_revive_old_claim() {
    let pool = database().await;
    let first = claim(&pool).await;
    let second = claim(&pool).await;
    let before = saved(&pool).await.3;
    translation::release(&pool, &first).await.unwrap();
    assert_eq!(saved(&pool).await.3, before);
    translation::release(&pool, &second).await.unwrap();
    assert!(!translation::finish(&pool, &first, "old", "en")
        .await
        .unwrap());
    assert!(!translation::finish(&pool, &second, "released", "en")
        .await
        .unwrap());
    assert_eq!(saved(&pool).await.0.as_deref(), Some("existing"));
}

#[tokio::test]
async fn deleted_and_recreated_message_does_not_accept_old_ticket() {
    let pool = database().await;
    let first = claim(&pool).await;
    sqlx::query("DELETE FROM messages WHERE id = 'm'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!translation::finish(&pool, &first, "deleted", "en")
        .await
        .unwrap());
    sqlx::query("INSERT INTO messages(id, conversation_id, content) VALUES ('m', 'a', 'source')")
        .execute(&pool)
        .await
        .unwrap();
    let fresh = claim(&pool).await;
    translation::release(&pool, &first).await.unwrap();
    assert!(!translation::finish(&pool, &first, "old", "en")
        .await
        .unwrap());
    assert!(translation::finish(&pool, &fresh, "new", "en")
        .await
        .unwrap());
}

#[tokio::test]
async fn moving_conversation_invalidates_translation_even_with_same_body() {
    let pool = database().await;
    let first = claim(&pool).await;
    sqlx::query("UPDATE messages SET conversation_id = 'b'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !translation::finish(&pool, &first, "wrong conversation", "en")
            .await
            .unwrap()
    );
    assert_eq!(saved(&pool).await, (None, None, 1, None));
}

#[tokio::test]
async fn unchanged_body_and_metadata_writes_preserve_translation_and_claim() {
    let pool = database().await;
    let ticket = claim(&pool).await;
    sqlx::raw_sql("UPDATE messages SET is_liked = 1; UPDATE messages SET content = 'source', conversation_id = 'a';")
        .execute(&pool).await.unwrap();
    assert_eq!(saved(&pool).await.2, 0);
    assert_eq!(saved(&pool).await.0.as_deref(), Some("existing"));
    assert!(translation::finish(&pool, &ticket, "new", "en")
        .await
        .unwrap());
}

#[tokio::test]
async fn mismatched_source_and_missing_id_do_not_supersede_a_valid_claim() {
    let pool = database().await;
    let ticket = claim(&pool).await;
    for (id, conversation, content) in [
        ("missing", "a", "source"),
        ("m", "b", "source"),
        ("m", "a", "edited"),
    ] {
        assert!(translation::begin(&pool, id, conversation, content)
            .await
            .unwrap()
            .is_none());
    }
    assert!(translation::finish(&pool, &ticket, "valid", "en")
        .await
        .unwrap());
}

#[tokio::test]
async fn competing_database_registrations_allow_exactly_one_commit() {
    let pool = database().await;
    let (first, second) = tokio::join!(
        translation::begin(&pool, "m", "a", "source"),
        translation::begin(&pool, "m", "a", "source")
    );
    let first = first.unwrap().unwrap();
    let second = second.unwrap().unwrap();
    let (one, two) = tokio::join!(
        translation::finish(&pool, &first, "one", "en"),
        translation::finish(&pool, &second, "two", "ja")
    );
    assert_ne!(one.unwrap(), two.unwrap());
    assert!(saved(&pool).await.3.is_none());
}

#[tokio::test]
async fn editing_after_translation_commit_invalidates_history_with_recursive_triggers_enabled() {
    let pool = database().await;
    sqlx::query("PRAGMA recursive_triggers = ON")
        .execute(&pool)
        .await
        .unwrap();
    let ticket = claim(&pool).await;
    assert!(translation::finish(&pool, &ticket, "translated", "en")
        .await
        .unwrap());
    sqlx::query("UPDATE messages SET content = 'edited after commit' WHERE id = 'm'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(saved(&pool).await, (None, None, 1, None));
}

#[tokio::test]
async fn complete_migration_chain_supports_translation_claim_and_content_invalidation() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO messages(id, conversation_id, role, content) VALUES ('m', 'a', 'assistant', 'source')")
        .execute(&pool).await.unwrap();
    let ticket = claim(&pool).await;
    assert!(translation::finish(&pool, &ticket, "translated", "en")
        .await
        .unwrap());
    sqlx::query("UPDATE messages SET content = 'edited' WHERE id = 'm'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(saved(&pool).await, (None, None, 1, None));
}
