use std::sync::Arc;

use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions};
use xenobot_api::database::repository::{ChatMeta, ImportSourceCheckpoint, Member, Message};
use xenobot_api::database::Repository;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn setup_test_repo() -> Result<Repository, Box<dyn std::error::Error>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    MIGRATOR.run(&pool).await?;
    Ok(Repository::new(Arc::new(pool)))
}

#[tokio::test]
async fn test_repository_basic_crud() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_test_repo().await?;

    let chat_meta = ChatMeta {
        id: 0,
        name: "Test Chat".to_string(),
        platform: "test".to_string(),
        chat_type: "group".to_string(),
        imported_at: 1_234_567_890,
        group_id: Some("group123".to_string()),
        group_avatar: Some("avatar.png".to_string()),
        owner_id: Some("owner123".to_string()),
        schema_version: 3,
        session_gap_threshold: 1800,
    };

    let id = repo.create_chat(&chat_meta).await?;
    assert!(id > 0);

    let retrieved = repo.get_chat(id).await?.expect("chat should exist");
    assert_eq!(retrieved.name, "Test Chat");
    assert_eq!(retrieved.platform, "test");

    let chats = repo.list_chats(None, 10, 0).await?;
    assert!(!chats.is_empty());

    repo.delete_chat(id).await?;
    let deleted = repo.get_chat(id).await?;
    assert!(deleted.is_none());

    Ok(())
}

#[tokio::test]
async fn test_member_crud() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_test_repo().await?;

    let chat_meta = ChatMeta {
        id: 0,
        name: "Test".to_string(),
        platform: "test".to_string(),
        chat_type: "group".to_string(),
        imported_at: 1_234_567_890,
        group_id: None,
        group_avatar: None,
        owner_id: None,
        schema_version: 3,
        session_gap_threshold: 1800,
    };
    let _meta_id = repo.create_chat(&chat_meta).await?;

    let member = Member {
        id: 0,
        platform_id: "user123".to_string(),
        account_name: Some("Test User".to_string()),
        group_nickname: Some("Nickname".to_string()),
        aliases: Some("[]".to_string()),
        avatar: Some("avatar.png".to_string()),
        roles: Some("[]".to_string()),
    };

    let member_id = repo.create_member(&member).await?;
    assert!(member_id > 0);

    let existing_id = repo.get_or_create_member("user123", None).await?;
    assert_eq!(existing_id, member_id);

    let new_id = repo
        .get_or_create_member("user456", Some("New User"))
        .await?;
    assert!(new_id > member_id);

    Ok(())
}

#[tokio::test]
async fn test_import_source_checkpoint_upsert_and_unchanged(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_test_repo().await?;

    let checkpoint = ImportSourceCheckpoint {
        id: 0,
        source_kind: "import".to_string(),
        source_path: "/tmp/sample_chat_export.txt".to_string(),
        fingerprint: "123:1700000000:42".to_string(),
        file_size: 123,
        modified_at: 1_700_000_000,
        platform: Some("whatsapp".to_string()),
        chat_name: Some("Team Room".to_string()),
        meta_id: None,
        last_processed_at: 1_700_000_100,
        last_inserted_messages: 20,
        last_duplicate_messages: 5,
        status: "completed".to_string(),
        error_message: None,
    };
    repo.upsert_import_source_checkpoint(&checkpoint).await?;

    let stored = repo
        .get_import_source_checkpoint("import", "/tmp/sample_chat_export.txt")
        .await?
        .expect("checkpoint should exist after upsert");
    assert_eq!(stored.source_kind, "import");
    assert_eq!(stored.source_path, "/tmp/sample_chat_export.txt");
    assert_eq!(stored.fingerprint, "123:1700000000:42");
    assert_eq!(stored.last_inserted_messages, 20);
    assert_eq!(stored.last_duplicate_messages, 5);

    let unchanged = repo
        .source_checkpoint_is_unchanged(
            "import",
            "/tmp/sample_chat_export.txt",
            "123:1700000000:42",
        )
        .await?;
    assert!(unchanged);

    let changed = repo
        .source_checkpoint_is_unchanged(
            "import",
            "/tmp/sample_chat_export.txt",
            "999:1700000000:42",
        )
        .await?;
    assert!(!changed);

    let updated = ImportSourceCheckpoint {
        fingerprint: "999:1700000001:43".to_string(),
        file_size: 999,
        modified_at: 1_700_000_001,
        last_processed_at: 1_700_000_200,
        last_inserted_messages: 3,
        last_duplicate_messages: 7,
        ..stored.clone()
    };
    repo.upsert_import_source_checkpoint(&updated).await?;
    let stored_after_update = repo
        .get_import_source_checkpoint("import", "/tmp/sample_chat_export.txt")
        .await?
        .expect("checkpoint should still exist");
    assert_eq!(stored_after_update.id, stored.id);
    assert_eq!(stored_after_update.fingerprint, "999:1700000001:43");
    assert_eq!(stored_after_update.file_size, 999);
    assert_eq!(stored_after_update.last_inserted_messages, 3);
    assert_eq!(stored_after_update.last_duplicate_messages, 7);

    Ok(())
}

#[tokio::test]
async fn test_message_exists_incremental_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_test_repo().await?;

    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Incremental".to_string(),
            platform: "telegram".to_string(),
            chat_type: "group".to_string(),
            imported_at: 1_700_000_000,
            group_id: None,
            group_avatar: None,
            owner_id: None,
            schema_version: 3,
            session_gap_threshold: 1800,
        })
        .await?;
    let sender_id = repo
        .get_or_create_member("telegram:user_1", Some("Alice"))
        .await?;

    repo.create_message(&Message {
        id: 0,
        sender_id,
        sender_account_name: Some("Alice".to_string()),
        sender_group_nickname: None,
        ts: 1_700_000_123,
        msg_type: 0,
        content: Some("hello".to_string()),
        reply_to_message_id: None,
        platform_message_id: None,
        meta_id,
    })
    .await?;

    let exists_same = repo
        .message_exists(meta_id, sender_id, 1_700_000_123, 0, Some("hello"))
        .await?;
    assert!(exists_same);

    let exists_diff_content = repo
        .message_exists(meta_id, sender_id, 1_700_000_123, 0, Some("hello2"))
        .await?;
    assert!(!exists_diff_content);

    repo.create_message(&Message {
        id: 0,
        sender_id,
        sender_account_name: Some("Alice".to_string()),
        sender_group_nickname: None,
        ts: 1_700_000_124,
        msg_type: 0,
        content: None,
        reply_to_message_id: None,
        platform_message_id: None,
        meta_id,
    })
    .await?;
    let exists_null_content = repo
        .message_exists(meta_id, sender_id, 1_700_000_124, 0, None)
        .await?;
    assert!(exists_null_content);

    Ok(())
}
