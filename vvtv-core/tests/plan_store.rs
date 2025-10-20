use vvtv_core::plan::planner::PlannerEvent;
use vvtv_core::{
    Plan, PlanImportRecord, Planner, PlannerConfig, RealizationOutcome, Realizer, RealizerConfig,
    SqlitePlanStore,
};

fn setup_store() -> SqlitePlanStore {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plans.sqlite");
    // Preserve directory on disk for the duration of the test runs.
    #[allow(deprecated)]
    let _persist = dir.into_path();
    let store = SqlitePlanStore::builder()
        .path(&path)
        .create_if_missing(true)
        .build()
        .unwrap();
    store.initialize().unwrap();
    // Persist temp path for later use.
    store
}

#[test]
fn test_plan_crud_and_metrics() {
    let store = setup_store();
    let mut plan = Plan::new("p1", "music");
    plan.title = Some("Sample".into());
    plan.duration_est_s = Some(620);
    store.upsert_plan(&plan).unwrap();

    let fetched = store.fetch_by_id("p1").unwrap().unwrap();
    assert_eq!(fetched.plan_id, "p1");
    assert_eq!(fetched.kind, "music");

    store.update_score("p1", 0.9).unwrap();
    let metrics = store.compute_metrics().unwrap();
    assert_eq!(metrics.total, 1);
    assert_eq!(*metrics.by_status.get("planned").unwrap(), 1);

    let audit = store.audit(chrono::Utc::now()).unwrap();
    assert!(!audit.is_empty());
}

#[tokio::test]
async fn test_planner_and_realizer_flow() {
    let store = setup_store();
    for idx in 0..5 {
        let mut plan = Plan::new(
            format!("plan-{idx}"),
            if idx % 2 == 0 { "music" } else { "talk" },
        );
        plan.duration_est_s = Some(600 + (idx as i64 * 30));
        plan.curation_score = 0.5 + idx as f64 * 0.1;
        plan.tags = vec!["test".into()];
        store.upsert_plan(&plan).unwrap();
    }

    let planner = Planner::new(store.clone(), PlannerConfig::default());
    let event = planner.run_once(chrono::Utc::now()).unwrap();
    match event {
        PlannerEvent::Selected(decisions) => {
            assert!(!decisions.is_empty());
        }
        PlannerEvent::Idle => panic!("expected selections"),
    }

    let realizer = Realizer::new(store.clone(), RealizerConfig::default());
    let mut iterations = 0;
    let mut handler = |_plan: Plan| async move {
        iterations += 1;
        if iterations == 1 {
            Ok(RealizationOutcome::Retry {
                note: "retry test".into(),
            })
        } else {
            Ok(RealizationOutcome::Downloaded { mark_ready: true })
        }
    };

    let processed = realizer.tick(&mut handler).await.unwrap();
    assert!(processed);
    let processed_second = realizer.tick(&mut handler).await.unwrap();
    assert!(processed_second);
}

#[test]
fn test_import_blacklist() {
    let store = setup_store();
    let plan = Plan::new("imported", "documentary");
    let record = PlanImportRecord {
        plan,
        overwrite: false,
    };
    let imported = store.import(&[record]).unwrap();
    assert_eq!(imported, 1);

    let entry = store
        .blacklist_add("example.com", Some("spam domain"))
        .unwrap();
    assert_eq!(entry.domain, "example.com");
    assert_eq!(entry.reason.as_deref(), Some("spam domain"));

    let list = store.blacklist_list().unwrap();
    assert_eq!(list.len(), 1);
    store.blacklist_remove("example.com").unwrap();
    assert!(store.blacklist_list().unwrap().is_empty());
}
