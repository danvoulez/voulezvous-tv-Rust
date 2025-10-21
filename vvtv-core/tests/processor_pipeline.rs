use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use vvtv_core::browser::{
    BrowserCapture, BrowserCaptureKind, ContentMetadata, PbdOutcome, PlaybackValidation,
};
use vvtv_core::config::{load_processor_config, load_vvtv_config, ProcessorConfig, VvtvConfig};
use vvtv_core::plan::{Plan, PlanStatus, SqlitePlanStore};
use vvtv_core::processor::{MasteringStrategy, Processor, ProcessorResult};
use vvtv_core::queue::{PlayoutQueueStore, QueueItem};

fn adjust_vvtv_config(base: &TempDir, mut config: VvtvConfig) -> VvtvConfig {
    let base_dir = base.path().join("vvtv");
    let cache_dir = base_dir.join("cache");
    let storage_dir = base_dir.join("storage");
    let logs_dir = base_dir.join("logs");
    let data_dir = base_dir.join("data");
    let broadcast_dir = base_dir.join("broadcast");
    let vault_dir = base_dir.join("vault");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::create_dir_all(&storage_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::create_dir_all(&broadcast_dir).unwrap();
    std::fs::create_dir_all(&vault_dir).unwrap();
    config.paths.base_dir = base_dir.to_string_lossy().to_string();
    config.paths.cache_dir = cache_dir.to_string_lossy().to_string();
    config.paths.storage_dir = storage_dir.to_string_lossy().to_string();
    config.paths.logs_dir = logs_dir.to_string_lossy().to_string();
    config.paths.data_dir = data_dir.to_string_lossy().to_string();
    config.paths.broadcast_dir = broadcast_dir.to_string_lossy().to_string();
    config.paths.vault_dir = vault_dir.to_string_lossy().to_string();
    config
}

fn adjust_processor_config(mut config: ProcessorConfig) -> ProcessorConfig {
    config.download.retry_delay_seconds = [1, 1];
    config
}

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

async fn build_processor(
    base: &TempDir,
) -> ProcessorResult<(
    Processor,
    SqlitePlanStore,
    PlayoutQueueStore,
    VvtvConfig,
    ProcessorConfig,
    PathBuf,
)> {
    let vvtv = adjust_vvtv_config(
        base,
        load_vvtv_config(fixture_path("configs/vvtv.toml")).unwrap(),
    );
    let processor_cfg = adjust_processor_config(
        load_processor_config(fixture_path("configs/processor.toml")).unwrap(),
    );
    let plans_path = base.path().join("plans.sqlite");
    let queue_path = base.path().join("queue.sqlite");
    let plan_store = SqlitePlanStore::builder()
        .path(&plans_path)
        .build()
        .unwrap();
    plan_store.initialize().unwrap();
    let queue_store = PlayoutQueueStore::builder()
        .path(&queue_path)
        .build()
        .unwrap();
    queue_store.initialize().unwrap();
    let processor = Processor::new(
        plan_store.clone(),
        queue_store.clone(),
        processor_cfg.clone(),
        vvtv.clone(),
    )
    .unwrap()
    .with_retry_sleep_cap(std::time::Duration::from_millis(5));
    Ok((
        processor,
        plan_store,
        queue_store,
        vvtv,
        processor_cfg,
        queue_path,
    ))
}

fn make_plan(id: &str, url: &str) -> Plan {
    let mut plan = Plan::new(id.to_string(), "video");
    plan.source_url = Some(url.to_string());
    plan.status = PlanStatus::Selected;
    plan.duration_est_s = Some(120);
    plan
}

fn hls_playlist(fixtures: &Path, durations: &[f64]) -> (String, Vec<PathBuf>) {
    let playlist_path = fixtures.join("media.m3u8");
    let mut contents = String::from(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:0\n",
    );
    let mut segment_paths = Vec::new();
    for (index, duration) in durations.iter().enumerate() {
        let segment = fixtures.join(format!("segment_{index}.ts"));
        std::fs::write(&segment, format!("SEGMENT {index}\n")).unwrap();
        contents.push_str(&format!("#EXTINF:{duration},\nsegment_{index}.ts\n"));
        segment_paths.push(segment);
    }
    contents.push_str("#EXT-X-ENDLIST\n");
    std::fs::write(&playlist_path, contents).unwrap();
    (format!("file://{}", playlist_path.display()), segment_paths)
}

fn pbd_outcome(url: String, kind: BrowserCaptureKind, height: u32) -> PbdOutcome {
    let capture = BrowserCapture {
        url,
        kind,
        quality_label: Some("1080p".to_string()),
        associated_requests: Vec::new(),
    };
    let validation = PlaybackValidation {
        video_width: 1920,
        video_height: height,
        duration_seconds: Some(120.0),
        current_time: 10.0,
        buffer_ahead: Some(5.0),
        ready_state: 4,
        hd_label: Some("1080p".into()),
    };
    PbdOutcome {
        capture,
        validation,
        metadata: ContentMetadata::default(),
    }
}

fn read_queue_items(queue_path: &Path) -> Vec<QueueItem> {
    let db = Connection::open(queue_path).unwrap();
    let mut stmt = db
        .prepare("SELECT plan_id, asset_path, duration_s, curation_score, priority, node_origin FROM playout_queue")
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok(QueueItem {
                plan_id: row.get(0)?,
                asset_path: row.get(1)?,
                duration_s: row.get(2)?,
                curation_score: row.get(3)?,
                priority: row.get(4)?,
                node_origin: row.get(5)?,
            })
        })
        .unwrap();
    rows.map(|row| row.unwrap()).collect()
}

#[tokio::test]
async fn processor_hls_pipeline_success() {
    let base = TempDir::new().unwrap();
    let (processor, plan_store, _queue_store, vvtv_config, _processor_cfg, queue_path) =
        build_processor(&base).await.unwrap();

    let fixtures = base.path().join("fixtures");
    std::fs::create_dir_all(&fixtures).unwrap();
    let (playlist_url, _segments) = hls_playlist(&fixtures, &[4.0, 4.0, 4.0]);

    let plan = make_plan("plan-hls", &playlist_url);
    plan_store.upsert_plan(&plan).unwrap();

    let outcome = pbd_outcome(playlist_url, BrowserCaptureKind::HlsMediaPlaylist, 1080);
    let report = processor
        .process_with_capture(&plan, outcome)
        .await
        .unwrap();

    assert_eq!(report.strategy, MasteringStrategy::Remux);
    assert!(!report.hd_missing);

    let stored = plan_store.fetch_by_id("plan-hls").unwrap().unwrap();
    assert_eq!(stored.status, PlanStatus::Edited);
    assert!(!stored.hd_missing);

    let ready_dir = Path::new(&vvtv_config.paths.storage_dir)
        .join("ready")
        .join("plan-hls");
    assert!(ready_dir.join("hls_720p.m3u8").exists());
    assert!(ready_dir.join("checksums.json").exists());

    let staging_dir = Path::new(&vvtv_config.paths.cache_dir)
        .join("tmp_downloads")
        .join("plan-hls");
    assert!(!staging_dir.exists());

    let queue_items = read_queue_items(&queue_path);
    assert_eq!(queue_items.len(), 1);
    assert!(queue_items[0].asset_path.ends_with("hls_720p.m3u8"));
}

#[tokio::test]
async fn processor_progressive_transcode_and_hd_missing() {
    let base = TempDir::new().unwrap();
    let (_initial_processor, plan_store, queue_store, vvtv_config, mut processor_cfg, queue_path) =
        build_processor(&base).await.unwrap();

    // Force transcode path
    processor_cfg.remux.prefer_copy = false;
    let processor = Processor::new(
        plan_store.clone(),
        queue_store.clone(),
        processor_cfg,
        vvtv_config.clone(),
    )
    .unwrap()
    .with_retry_sleep_cap(std::time::Duration::from_millis(5));

    let fixtures = base.path().join("fixtures_prog");
    std::fs::create_dir_all(&fixtures).unwrap();
    let source_path = fixtures.join("source.mp4");
    std::fs::write(&source_path, "FAKE MP4").unwrap();
    let url = format!("file://{}", source_path.display());

    let plan = make_plan("plan-prog", &url);
    plan_store.upsert_plan(&plan).unwrap();

    let outcome = pbd_outcome(url, BrowserCaptureKind::Progressive, 540);
    let report = processor
        .process_with_capture(&plan, outcome)
        .await
        .unwrap();

    assert_eq!(report.strategy, MasteringStrategy::Transcode);
    assert!(report.hd_missing);

    let stored = plan_store.fetch_by_id("plan-prog").unwrap().unwrap();
    assert_eq!(stored.status, PlanStatus::Edited);
    assert!(stored.hd_missing);

    let ready_dir = Path::new(&vvtv_config.paths.storage_dir)
        .join("ready")
        .join("plan-prog");
    assert!(ready_dir.join("master.mp4").exists());

    let queue_items = read_queue_items(&queue_path);
    assert!(queue_items.iter().any(|item| item.plan_id == "plan-prog"));
}
