use chrono::{Duration, Utc};
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use vvtv_core::monetization::{
    AdaptiveProgrammer, AudienceStoreBuilder, EconomyEventType, EconomyStoreBuilder,
    MicroSpotManager, MonetizationDashboard, NewEconomyEvent, NewViewerSession,
};
use vvtv_core::plan::{Plan, PlanStatus, SqlitePlanStore};
use vvtv_core::queue::PlayoutQueueStore;

fn setup_plan_store(base: &Path) -> SqlitePlanStore {
    let path = base.join("plans.sqlite");
    let store = SqlitePlanStore::builder()
        .path(&path)
        .build()
        .expect("plan store");
    store.initialize().expect("initialize plans");
    store
}

fn setup_queue_store(base: &Path) -> PlayoutQueueStore {
    let path = base.join("queue.sqlite");
    let store = PlayoutQueueStore::builder()
        .path(&path)
        .build()
        .expect("queue store");
    store.initialize().expect("initialize queue");
    store
}

#[test]
fn economy_audience_and_dashboard_pipeline() {
    let temp = tempdir().expect("tempdir");
    let base = temp.path().to_path_buf();

    let economy_path = base.join("economy.sqlite");
    let economy = EconomyStoreBuilder::new()
        .path(&economy_path)
        .build()
        .expect("economy store");
    economy.initialize().expect("economy schema");

    let audience_path = base.join("viewers.sqlite");
    let audience = AudienceStoreBuilder::new()
        .path(&audience_path)
        .build()
        .expect("audience store");
    audience.initialize().expect("audience schema");

    let plan_store = setup_plan_store(&base);
    let queue_store = setup_queue_store(&base);

    let mut session = NewViewerSession::new("sess-1", "EU", "desktop");
    session.leave_time = Some(session.join_time + Duration::minutes(20));
    session.bandwidth_mbps = Some(8.0);
    session.engagement_score = Some(0.85);
    audience.record_session(&session).expect("record session");

    let mut event = NewEconomyEvent::new(EconomyEventType::View, 0.25, "viewer", "plan-1");
    event.timestamp = Some(Utc::now() - Duration::minutes(30));
    economy.record_event(&event).expect("record event");

    let mut cost = NewEconomyEvent::new(EconomyEventType::Cost, 0.05, "ops", "cdn");
    cost.timestamp = Some(Utc::now() - Duration::minutes(25));
    economy.record_event(&cost).expect("record cost");

    let mut plan = Plan::new("plan-1", "video");
    plan.title = Some("Evening Warm Lights".into());
    plan.duration_est_s = Some(900);
    plan.tags = vec!["warm".into(), "city".into()];
    plan.curation_score = 0.5;
    plan.status = PlanStatus::Planned;
    plan_store.upsert_plan(&plan).expect("seed plan");

    let adaptive = AdaptiveProgrammer::new(plan_store.clone(), economy.clone(), audience.clone());
    let report = adaptive.run_once(Utc::now()).expect("adaptive run");
    assert!(!report.updates.is_empty(), "expect adaptive updates");

    let updated_plan = plan_store
        .fetch_by_id("plan-1")
        .expect("fetch plan")
        .expect("plan exists");
    assert!(updated_plan.curation_score > 0.5);
    assert!(updated_plan.desire_vector.is_some());

    let dashboard = MonetizationDashboard::new(&economy, &audience, &plan_store);
    let artifacts = dashboard
        .generate(&base, Utc::now())
        .expect("dashboard generated");
    assert!(artifacts.html_path.exists());
    assert!(artifacts.finance_path.exists());
    assert!(artifacts.trends_path.exists());
    assert!(artifacts.heatmap_path.exists());

    // Micro spot injection
    let manager = MicroSpotManager::new(economy.clone());
    let contract_path = base.join("spot.lll");
    fs::write(
        &contract_path,
        r#"{
            "id": "sponsor-1",
            "sponsor": "VoulezVous Atelier",
            "visual_style": "warm fade",
            "duration_s": 5,
            "value_eur": 1.2,
            "asset_path": "/vvtv/storage/spots/warm.mp4"
        }"#,
    )
    .expect("write contract");
    manager
        .register_from_file(&contract_path)
        .expect("register spot");
    let injections = manager
        .inject_due(&queue_store, Utc::now())
        .expect("inject spot");
    assert_eq!(injections.len(), 1);

    let queue_entries = queue_store
        .list(&vvtv_core::queue::QueueFilter {
            status: None,
            limit: Some(10),
        })
        .expect("queue list");
    assert_eq!(queue_entries.len(), 1);
}
