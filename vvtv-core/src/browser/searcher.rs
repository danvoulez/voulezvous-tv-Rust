use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rand::Rng;
use serde::Deserialize;
use tracing::trace;

use super::automation::BrowserContext;
use crate::browser::{BrowserAutomation, BrowserError, BrowserResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngine {
    Google,
    Bing,
    DuckDuckGo,
}

impl fmt::Display for SearchEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            SearchEngine::Google => "google",
            SearchEngine::Bing => "bing",
            SearchEngine::DuckDuckGo => "duckduckgo",
        };
        f.write_str(label)
    }
}

impl std::str::FromStr for SearchEngine {
    type Err = BrowserError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "google" => Ok(SearchEngine::Google),
            "bing" => Ok(SearchEngine::Bing),
            "duckduckgo" | "ddg" => Ok(SearchEngine::DuckDuckGo),
            other => Err(BrowserError::Configuration(format!(
                "invalid search engine: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub search_engine: SearchEngine,
    pub scroll_iterations: usize,
    pub max_results: usize,
    pub filter_domains: Vec<String>,
    pub delay_range_ms: (u64, u64),
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub domain: String,
    pub rank: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResultRaw {
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

#[async_trait(?Send)]
pub trait SearchSession {
    async fn goto(&mut self, url: &str) -> BrowserResult<()>;
    async fn idle(&mut self, range_ms: (u64, u64)) -> BrowserResult<()>;
    async fn scroll(&mut self, delta_y: f64) -> BrowserResult<()>;
    async fn extract_results(&mut self, script: &str) -> BrowserResult<Vec<SearchResultRaw>>;
}

#[async_trait(?Send)]
pub trait SearchSessionFactory: Send + Sync {
    async fn create(&self) -> BrowserResult<Box<dyn SearchSession>>;
}

pub struct BrowserSearchSessionFactory {
    automation: Arc<BrowserAutomation>,
}

impl BrowserSearchSessionFactory {
    pub fn new(automation: Arc<BrowserAutomation>) -> Self {
        Self { automation }
    }
}

pub struct BrowserSearchSession {
    context: BrowserContext,
}

#[async_trait(?Send)]
impl SearchSession for BrowserSearchSession {
    async fn goto(&mut self, url: &str) -> BrowserResult<()> {
        self.context.goto(url).await
    }

    async fn idle(&mut self, range_ms: (u64, u64)) -> BrowserResult<()> {
        if range_ms.0 == 0 && range_ms.1 == 0 {
            return Ok(());
        }
        let mut rng = rand::thread_rng();
        let lower = range_ms.0.min(range_ms.1);
        let upper = range_ms.0.max(range_ms.1);
        let millis = rng.gen_range(lower..=upper);
        tokio::time::sleep(Duration::from_millis(millis)).await;
        Ok(())
    }

    async fn scroll(&mut self, delta_y: f64) -> BrowserResult<()> {
        let script = format!("window.scrollBy({{ top: {delta_y}, behavior: 'smooth' }});");
        self.context
            .page()
            .evaluate(script.as_str())
            .await
            .map_err(|err| {
                BrowserError::Unexpected(format!("failed to execute scroll script: {err}"))
            })?;
        tokio::time::sleep(Duration::from_millis(180)).await;
        Ok(())
    }

    async fn extract_results(&mut self, script: &str) -> BrowserResult<Vec<SearchResultRaw>> {
        let value = self
            .context
            .page()
            .evaluate(script)
            .await?
            .into_value()
            .map_err(|err| {
                BrowserError::Unexpected(format!("failed to decode search results payload: {err}"))
            })?;
        let results: Vec<SearchResultRaw> = serde_json::from_value(value).map_err(|err| {
            BrowserError::Unexpected(format!("failed to deserialize search results: {err}"))
        })?;
        Ok(results)
    }
}

#[async_trait(?Send)]
impl SearchSessionFactory for BrowserSearchSessionFactory {
    async fn create(&self) -> BrowserResult<Box<dyn SearchSession>> {
        let context = self.automation.new_context().await?;
        Ok(Box::new(BrowserSearchSession { context }))
    }
}

pub struct ContentSearcher {
    config: Arc<SearchConfig>,
    sessions: Arc<dyn SearchSessionFactory>,
}

impl ContentSearcher {
    pub fn new(config: Arc<SearchConfig>, sessions: Arc<dyn SearchSessionFactory>) -> Self {
        Self { config, sessions }
    }

    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    pub fn search_engine(&self) -> SearchEngine {
        self.config.search_engine
    }

    pub fn build_query_url(&self, query: &str) -> String {
        let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        match self.config.search_engine {
            SearchEngine::Google => format!("https://www.google.com/search?q={encoded}&tbm=vid"),
            SearchEngine::Bing => format!("https://www.bing.com/videos/search?q={encoded}"),
            SearchEngine::DuckDuckGo => {
                format!("https://duckduckgo.com/?q={encoded}&ia=video&iax=videos")
            }
        }
    }

    pub async fn search(&self, query: &str) -> BrowserResult<Vec<Candidate>> {
        let url = self.build_query_url(query);
        trace!(engine = %self.config.search_engine, url = %url, "opening search url");

        let mut session = self.sessions.create().await?;
        session.goto(&url).await?;
        session.idle(self.config.delay_range_ms).await?;

        let mut aggregated = Vec::new();
        for iteration in 0..self.config.scroll_iterations.max(1) {
            let script = self.result_parser_script();
            let mut batch = session.extract_results(&script).await?;
            aggregated.append(&mut batch);
            trace!(
                iteration,
                collected = aggregated.len(),
                "parsed search results"
            );

            if iteration + 1 < self.config.scroll_iterations {
                session.scroll(self.random_scroll_distance()).await?;
                session.idle(self.config.delay_range_ms).await?;
            }
        }

        let candidates = self.normalize_candidates(aggregated);
        Ok(self.filter_candidates(candidates))
    }

    fn normalize_candidates(&self, raw: Vec<SearchResultRaw>) -> Vec<Candidate> {
        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for result in raw {
            if result.url.is_empty() {
                continue;
            }
            if seen.insert(result.url.clone()) {
                ordered.push(result);
            }
        }

        ordered
            .into_iter()
            .enumerate()
            .map(|(idx, item)| Candidate {
                domain: self.extract_domain(&item.url),
                url: item.url,
                title: item.title,
                snippet: item.snippet,
                rank: idx + 1,
            })
            .collect()
    }

    fn filter_candidates(&self, candidates: Vec<Candidate>) -> Vec<Candidate> {
        let mut filtered = Vec::new();
        for candidate in candidates {
            if !self.filter_by_domain(&candidate) {
                trace!(url = %candidate.url, "domain filtered");
                continue;
            }
            if !self.is_likely_video(&candidate) {
                trace!(url = %candidate.url, "failed video heuristic");
                continue;
            }
            filtered.push(candidate);
            if filtered.len() >= self.config.max_results {
                break;
            }
        }
        filtered
    }

    fn filter_by_domain(&self, candidate: &Candidate) -> bool {
        if self.config.filter_domains.is_empty() {
            return true;
        }
        let domain = candidate.domain.to_lowercase();
        self.config
            .filter_domains
            .iter()
            .any(|allowed| domain.contains(allowed))
    }

    fn is_likely_video(&self, candidate: &Candidate) -> bool {
        let video_indicators = [
            "youtube",
            "vimeo",
            "dailymotion",
            "metacafe",
            "watch",
            "video",
            "documentary",
            "livestream",
            "playlist",
        ];
        let mut haystack = candidate.url.to_lowercase();
        if let Some(title) = &candidate.title {
            haystack.push(' ');
            haystack.push_str(&title.to_lowercase());
        }
        if let Some(snippet) = &candidate.snippet {
            haystack.push(' ');
            haystack.push_str(&snippet.to_lowercase());
        }
        video_indicators
            .iter()
            .any(|indicator| haystack.contains(indicator))
    }

    fn extract_domain(&self, url: &str) -> String {
        url::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(|s| s.to_lowercase()))
            .unwrap_or_else(|| url.to_lowercase())
    }

    fn random_scroll_distance(&self) -> f64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(320.0..720.0)
    }

    fn result_parser_script(&self) -> String {
        match self.config.search_engine {
            SearchEngine::Google => r#"
(() => {
    const results = [];
    const items = document.querySelectorAll('div.g, div[data-sokoban-container]');
    items.forEach(item => {
        const link = item.querySelector('a[href^="http"]');
        const title = item.querySelector('h3');
        const snippet = item.querySelector('div[data-content-feature], div[data-sncf]');
        if (link && link.href) {
            results.push({
                url: link.href,
                title: title ? title.textContent : null,
                snippet: snippet ? snippet.textContent : null,
            });
        }
    });
    return results;
})()
"#
            .to_string(),
            SearchEngine::Bing => r#"
(() => {
    const results = [];
    const items = document.querySelectorAll('li.b_algo, li.b_algoVideo, div.video-item');
    items.forEach(item => {
        const link = item.querySelector('a[href^="http"]');
        const title = item.querySelector('h2, h3');
        const snippet = item.querySelector('p');
        if (link && link.href) {
            results.push({
                url: link.href,
                title: title ? title.textContent : null,
                snippet: snippet ? snippet.textContent : null,
            });
        }
    });
    return results;
})()
"#
            .to_string(),
            SearchEngine::DuckDuckGo => r#"
(() => {
    const results = [];
    const items = document.querySelectorAll('article[data-testid="result"], div.result__body');
    items.forEach(item => {
        const link = item.querySelector('a[data-testid="result-title-a"], a.result__a');
        const title = link ? link.textContent : null;
        const snippet = item.querySelector('div[data-result="snippet"], div.result__snippet');
        if (link && link.href) {
            results.push({
                url: link.href,
                title: title ? title.trim() : null,
                snippet: snippet ? snippet.textContent : null,
            });
        }
    });
    return results;
})()
"#
            .to_string(),
        }
    }
}
