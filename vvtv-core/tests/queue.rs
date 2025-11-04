use std::path::Path;

use chrono::Duration;
use tempfile::TempDir;
use vvtv_core::{PlayoutQueueStore, QueueFilter, QueueItem, QueueSelectionPolicy, QueueStatus};

fn temp_store(dir: &Path) -> (PlayoutQueueStore, std::path::PathBuf) {
    let path = dir.join("queue.sqlite");
    let store = PlayoutQueueStore::builder()
        .path(&path)
        .create_if_missing(true)
        .build()
        .expect("create store");
    store.initialize().expect("initialize store");
    (store, path)
}

#[test]
fn enqueue_and_list_items() {
    let dir = TempDir::new().unwrap();
    let (store, _) = temp_store(dir.path());
    let item = QueueItem {
        plan_id: "plan-a".into(),
        asset_path: "/tmp/a.m3u8".into(),
        duration_s: Some(120),
        curation_score: Some(0.9),
        priority: 0,
        node_origin: Some("node-a".into()),
        content_kind: Some("video".into()),
    };
    store.enqueue(&item).unwrap();

    let list = store
        .list(&QueueFilter {
            status: Some(QueueStatus::Queued),
            limit: Some(10),
        })
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].plan_id, "plan-a");
    assert_eq!(list[0].content_kind.as_deref(), Some("video"));
}

#[test]
fn begin_playback_prefers_priority_and_music_ratio() {
    let dir = TempDir::new().unwrap();
    let (store, _) = temp_store(dir.path());

    let high_priority = QueueItem {
        plan_id: "priority".into(),
        asset_path: "/tmp/p.m3u8".into(),
        duration_s: Some(60),
        curation_score: Some(0.5),
        priority: 1,
        node_origin: None,
        content_kind: Some("video".into()),
    };
    store.enqueue(&high_priority).unwrap();

    let music = QueueItem {
        plan_id: "music".into(),
        asset_path: "/tmp/m.m3u8".into(),
        duration_s: Some(80),
        curation_score: Some(0.95),
        priority: 0,
        node_origin: None,
        content_kind: Some("music".into()),
    };
    store.enqueue(&music).unwrap();

    let policy = QueueSelectionPolicy::new(Some(10), 0.85, Duration::hours(24));
    let first = store.begin_playback(&policy).unwrap().unwrap();
    assert_eq!(first.plan_id, "priority");
    store
        .mark_playback_result(first.id, QueueStatus::Played, first.duration_s, None)
        .unwrap();

    // Mark the first item as played recently to trigger music preference.
    let second = store
        .begin_playback(&policy)
        .unwrap()
        .expect("expected music entry");
    assert_eq!(second.plan_id, "music");
}

#[test]
fn metrics_and_cleanup() {
    let dir = TempDir::new().unwrap();
    let (store, _) = temp_store(dir.path());

    let queued = QueueItem {
        plan_id: "queued".into(),
        asset_path: "/tmp/q.m3u8".into(),
        duration_s: Some(100),
        curation_score: Some(0.7),
        priority: 0,
        node_origin: None,
        content_kind: Some("video".into()),
    };
    let queued_id = store.enqueue(&queued).unwrap();

    let played = QueueItem {
        plan_id: "played".into(),
        asset_path: "/tmp/p.mp4".into(),
        duration_s: Some(90),
        curation_score: Some(0.8),
        priority: 0,
        node_origin: None,
        content_kind: Some("video".into()),
    };
    let played_id = store.enqueue(&played).unwrap();
    store
        .mark_playback_result(played_id, QueueStatus::Played, Some(90), None)
        .unwrap();

    let metrics = store.metrics().unwrap();
    assert_eq!(metrics.queue_length, 1);
    assert_eq!(metrics.failures_last_hour, 0);

    let removed = store.cleanup_played(Duration::hours(0)).unwrap();
    assert_eq!(removed, 1);

    // Backup should create file
    let backup_path = dir.path().join("queue_backup.sql.gz");
    store.export_backup(&backup_path).unwrap();
    assert!(backup_path.exists());

    // Remaining queued item should still exist
    let list = store
        .list(&QueueFilter {
            status: Some(QueueStatus::Queued),
            limit: Some(10),
        })
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, queued_id);
}
