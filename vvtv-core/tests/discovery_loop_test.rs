use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use async_trait::async_trait;
use futures::future::poll_fn;
use tokio::sync::Mutex;

use vvtv_core::browser::{
    BrowserCapture, BrowserCaptureKind, BrowserResult, Candidate, ContentMetadata, ContentSearcher,
    DiscoveryConfig, DiscoveryLoop, DiscoveryPbd, DiscoveryPlanStore, PbdOutcome,
    PlaybackValidation, SearchConfig, SearchEngine, SearchResultRaw, SearchSession,
    SearchSessionFactory,
};
use vvtv_core::plan::Plan;

fn search_config(engine: SearchEngine) -> Arc<SearchConfig> {
    Arc::new(SearchConfig {
        search_engine: engine,
        scroll_iterations: 1,
        max_results: 10,
        filter_domains: vec![],
        delay_range_ms: (0, 0),
    })
}

struct MockSearchSessionFactory {
    batches: Vec<Vec<CandidateStub>>,
}

#[derive(Clone)]
struct CandidateStub {
    url: String,
    title: Option<String>,
    snippet: Option<String>,
}

struct MockSearchSession {
    batches: Vec<Vec<CandidateStub>>,
    index: usize,
}

#[async_trait(?Send)]
impl SearchSession for MockSearchSession {
    async fn goto(&mut self, _url: &str) -> BrowserResult<()> {
        Ok(())
    }

    async fn idle(&mut self, _range_ms: (u64, u64)) -> BrowserResult<()> {
        Ok(())
    }

    async fn scroll(&mut self, _delta_y: f64) -> BrowserResult<()> {
        Ok(())
    }

    async fn extract_results(&mut self, _script: &str) -> BrowserResult<Vec<SearchResultRaw>> {
        let batch = if self.index < self.batches.len() {
            self.batches[self.index].clone()
        } else {
            Vec::new()
        };
        self.index += 1;
        Ok(batch
            .into_iter()
            .map(|stub| SearchResultRaw {
                url: stub.url,
                title: stub.title,
                snippet: stub.snippet,
            })
            .collect())
    }
}

#[async_trait(?Send)]
impl SearchSessionFactory for MockSearchSessionFactory {
    async fn create(&self) -> BrowserResult<Box<dyn SearchSession>> {
        Ok(Box::new(MockSearchSession {
            batches: self.batches.clone(),
            index: 0,
        }))
    }
}

struct MockPbd {
    outcome: PbdOutcome,
}

#[async_trait(?Send)]
impl DiscoveryPbd for MockPbd {
    async fn collect(&self, _url: &str) -> BrowserResult<PbdOutcome> {
        Ok(self.outcome.clone())
    }
}

struct MockPlanStore {
    created: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl DiscoveryPlanStore for MockPlanStore {
    async fn create_from_outcome(
        &self,
        candidate: &Candidate,
        _outcome: &PbdOutcome,
    ) -> vvtv_core::plan::PlanResult<Plan> {
        let mut guard = self.created.lock().await;
        guard.push(candidate.url.clone());
        let plan_id = format!("mock-{}", guard.len());
        drop(guard);
        let mut plan = Plan::new(plan_id, "video");
        plan.source_url = Some(candidate.url.clone());
        Ok(plan)
    }
}

fn sample_outcome(url: &str) -> PbdOutcome {
    PbdOutcome {
        capture: BrowserCapture {
            url: url.to_string(),
            kind: BrowserCaptureKind::HlsMediaPlaylist,
            quality_label: Some("1080p".to_string()),
            associated_requests: vec![],
        },
        validation: PlaybackValidation {
            video_width: 1920,
            video_height: 1080,
            duration_seconds: Some(600.0),
            current_time: 30.0,
            buffer_ahead: Some(10.0),
            ready_state: 4,
            hd_label: Some("1080p".to_string()),
        },
        metadata: ContentMetadata::default(),
    }
}

#[test]
fn test_google_query_url_generation() {
    let factory = MockSearchSessionFactory { batches: vec![] };
    let searcher = ContentSearcher::new(search_config(SearchEngine::Google), Arc::new(factory));
    let url = searcher.build_query_url("creative commons");
    let parsed = url::Url::parse(&url).unwrap();
    assert_eq!(parsed.host_str(), Some("www.google.com"));
    assert_eq!(parsed.path(), "/search");
    let mut params = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        params.remove("q").map(|v| v.into_owned()),
        Some("creative commons".to_string())
    );
    assert_eq!(
        params.remove("tbm").map(|v| v.into_owned()),
        Some("vid".to_string())
    );
}

#[tokio::test]
async fn test_video_candidate_filtering() {
    let factory = MockSearchSessionFactory {
        batches: vec![vec![
            CandidateStub {
                url: "https://www.youtube.com/watch?v=1".into(),
                title: Some("Documentary".into()),
                snippet: None,
            },
            CandidateStub {
                url: "https://example.com/blog".into(),
                title: Some("Blog".into()),
                snippet: Some("Latest news".into()),
            },
        ]],
    };
    let mut config = (*search_config(SearchEngine::Google)).clone();
    config.filter_domains = vec!["youtube".into()];
    let searcher = ContentSearcher::new(Arc::new(config), Arc::new(factory));
    let results = searcher.search("creative commons").await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].url.contains("youtube"));
}

#[tokio::test(start_paused = true)]
async fn test_rate_limiting_accumulates_delay() {
    let factory = MockSearchSessionFactory {
        batches: vec![vec![
            CandidateStub {
                url: "https://video.com/1".into(),
                title: Some("Video".into()),
                snippet: None,
            },
            CandidateStub {
                url: "https://video.com/2".into(),
                title: Some("Video".into()),
                snippet: None,
            },
        ]],
    };
    let config = search_config(SearchEngine::Google);
    let searcher = ContentSearcher::new(config, Arc::new(factory));
    let pbd: Arc<dyn DiscoveryPbd> = Arc::new(MockPbd {
        outcome: sample_outcome("https://video.com/manifest"),
    });
    let _recorded = Arc::new(Mutex::new(Vec::new()));
    let store: Arc<dyn DiscoveryPlanStore> = Arc::new(MockPlanStore {
        created: Arc::clone(&_recorded),
    });
    let mut loop_runner = DiscoveryLoop::new(
        searcher,
        pbd,
        store,
        DiscoveryConfig {
            max_plans_per_run: 5,
            candidate_delay_range_ms: (100, 100),
            stop_on_first_error: false,
            dry_run: false,
            debug: false,
        },
    );

    let mut future: Pin<Box<_>> = Box::pin(loop_runner.run("query"));
    poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("discovery completed without waiting"),
    })
    .await;
    tokio::time::advance(std::time::Duration::from_millis(100)).await;
    let stats = future.await.unwrap();
    assert_eq!(stats.total_wait_ms, 100);
}

#[tokio::test]
async fn test_discovery_dry_run_skips_plan_creation() {
    let factory = MockSearchSessionFactory {
        batches: vec![vec![CandidateStub {
            url: "https://video.com/only".into(),
            title: Some("Video".into()),
            snippet: Some("Clip".into()),
        }]],
    };
    let searcher = ContentSearcher::new(search_config(SearchEngine::Google), Arc::new(factory));
    let pbd: Arc<dyn DiscoveryPbd> = Arc::new(MockPbd {
        outcome: sample_outcome("https://video.com/manifest"),
    });
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let store_trait: Arc<dyn DiscoveryPlanStore> = Arc::new(MockPlanStore {
        created: Arc::clone(&recorded),
    });
    let mut loop_runner = DiscoveryLoop::new(
        searcher,
        pbd,
        store_trait,
        DiscoveryConfig {
            max_plans_per_run: 2,
            candidate_delay_range_ms: (0, 0),
            stop_on_first_error: false,
            dry_run: true,
            debug: false,
        },
    );

    let stats = loop_runner.run("query").await.unwrap();
    assert_eq!(stats.plans_created, 0);
    assert!(recorded.lock().await.is_empty());
}
