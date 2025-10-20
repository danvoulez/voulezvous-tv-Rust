use std::collections::HashSet;

use regex::Regex;
use serde::{Deserialize, Serialize};

use chromiumoxide::page::Page;

use crate::config::SelectorSection;

use super::error::{BrowserError, BrowserResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizedTag {
    pub raw: String,
    pub normalized: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentMetadata {
    pub title: Option<String>,
    pub duration_seconds: Option<u64>,
    pub tags: Vec<NormalizedTag>,
    pub breadcrumbs: Vec<String>,
    pub resolution_label: Option<String>,
    pub expected_bitrate: Option<u64>,
    pub license_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MetadataExtractor {
    selectors: SelectorSection,
    sanitize_regex: Regex,
}

#[derive(Debug, Deserialize)]
struct RawMetadataPayload {
    title_candidates: Vec<String>,
    tags: Vec<String>,
    breadcrumbs: Vec<String>,
    resolution_labels: Vec<String>,
    license_text: Option<String>,
}

impl MetadataExtractor {
    pub fn new(selectors: SelectorSection) -> Self {
        let sanitize_regex = Regex::new(r"[^A-Za-z0-9\s\-_]").expect("valid regex");
        Self {
            selectors,
            sanitize_regex,
        }
    }

    pub async fn extract(&self, page: &Page) -> BrowserResult<ContentMetadata> {
        let payload: RawMetadataPayload = page
            .evaluate(self.dom_scraper_script().as_str())
            .await
            .map_err(|err| {
                BrowserError::Metadata(format!("failed to evaluate metadata script: {err}"))
            })?
            .into_value()
            .map_err(|err| {
                BrowserError::Metadata(format!("failed to parse metadata payload: {err}"))
            })?;

        let title = self.select_title(payload.title_candidates);
        let tags = self.normalize_tags(payload.tags);
        let breadcrumbs = payload
            .breadcrumbs
            .into_iter()
            .map(|crumb| crumb.trim().to_string())
            .filter(|crumb| !crumb.is_empty())
            .collect::<Vec<_>>();
        let resolution_label = payload
            .resolution_labels
            .into_iter()
            .find(|label| label.contains('p'))
            .map(|s| s.trim().to_string());
        let expected_bitrate = resolution_label
            .as_deref()
            .and_then(|label| bitrate_for_resolution(label));

        let duration_seconds = page
            .evaluate("(() => { const video = document.querySelector('video'); return video ? Math.floor(video.duration || 0) : null; })()")
            .await
            .map_err(|err| BrowserError::Metadata(format!("failed to read video duration: {err}")))?
            .into_value::<Option<u64>>()
            .unwrap_or(None);

        Ok(ContentMetadata {
            title,
            duration_seconds,
            tags,
            breadcrumbs,
            resolution_label,
            expected_bitrate,
            license_hint: payload
                .license_text
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        })
    }

    fn dom_scraper_script(&self) -> String {
        let selectors = &self.selectors;
        let tag_selector = selectors
            .play_buttons
            .iter()
            .map(|s| format!("'{}'", escape_js(s)))
            .collect::<Vec<_>>()
            .join(",");
        let breadcrumb_selector = "[class*='breadcrumb'] a, nav.breadcrumb a";
        let quality_selector = selectors
            .quality_menu
            .iter()
            .map(|s| {
                format!(
                    "document.querySelectorAll('{} *'), document.querySelectorAll('{}')",
                    escape_js(s),
                    escape_js(s)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let license_candidates = vec![
            "meta[name='license']",
            "meta[property='og:site_name']",
            "meta[name='copyright']",
        ];
        format!(
            r#"
(() => {{
    const pick = (list) => Array.from(list).map(el => (el.textContent || '').trim()).filter(Boolean);
    const unique = (items) => Array.from(new Set(items));
    const titleCandidates = unique([
        document.title || '',
        (document.querySelector('h1') || {{}}).innerText || '',
        (document.querySelector("meta[property='og:title']") || {{}}).content || '',
        (document.querySelector("meta[name='title']") || {{}}).content || ''
    ].filter(Boolean));
    const tagSelectors = [{tag_selector}];
    const tags = unique(tagSelectors.flatMap(selector => pick(document.querySelectorAll(selector))));
    const breadcrumbs = unique(pick(document.querySelectorAll('{breadcrumb_selector}')));
    const qualityNodes = [{quality_selector}];
    const resolutionLabels = unique(qualityNodes.flatMap(nodes => pick(nodes)));
    const licenseNodes = [{license_selectors}];
    const licenseText = licenseNodes
        .map(sel => document.querySelector(sel))
        .filter(Boolean)
        .map(node => (node.content || node.innerText || '').trim())
        .find(Boolean) || null;
    return {{
        title_candidates: titleCandidates,
        tags,
        breadcrumbs,
        resolution_labels: resolutionLabels,
        license_text: licenseText
    }};
}})()
"#,
            tag_selector = tag_selector,
            breadcrumb_selector = escape_js(breadcrumb_selector),
            quality_selector = quality_selector,
            license_selectors = license_candidates
                .into_iter()
                .map(|s| format!("'{}'", escape_js(s)))
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    fn select_title(&self, mut candidates: Vec<String>) -> Option<String> {
        candidates.retain(|c| !c.trim().is_empty());
        candidates
            .into_iter()
            .max_by_key(|candidate| candidate.len())
            .map(|title| title.trim().to_string())
    }

    fn normalize_tags(&self, tags: Vec<String>) -> Vec<NormalizedTag> {
        let mut normalized = Vec::new();
        let mut seen = HashSet::new();
        for tag in tags {
            let trimmed = tag.trim();
            if trimmed.is_empty() {
                continue;
            }
            let norm = self
                .sanitize_regex
                .replace_all(trimmed, "")
                .to_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if norm.is_empty() {
                continue;
            }
            if seen.insert(norm.clone()) {
                normalized.push(NormalizedTag {
                    raw: trimmed.to_string(),
                    normalized: norm,
                });
            }
        }
        normalized
    }
}

fn bitrate_for_resolution(label: &str) -> Option<u64> {
    if let Some(height) = parse_height(label) {
        match height {
            h if h >= 2160 => Some(14_000_000),
            h if h >= 1440 => Some(8_500_000),
            h if h >= 1080 => Some(6_000_000),
            h if h >= 720 => Some(3_200_000),
            h if h >= 480 => Some(1_800_000),
            h if h >= 360 => Some(1_000_000),
            _ => Some(600_000),
        }
    } else {
        None
    }
}

fn parse_height(label: &str) -> Option<u32> {
    let digits = label
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    digits.parse::<u32>().ok()
}

fn escape_js(input: &str) -> String {
    input.replace("\\", "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_tags_filters_duplicates() {
        let section = SelectorSection {
            video_element: "video".into(),
            play_buttons: vec![".play".into()],
            quality_menu: vec![".quality".into()],
            consent_buttons: vec![".consent".into()],
        };
        let extractor = MetadataExtractor::new(section);
        let result = extractor.normalize_tags(vec![
            "Ambient Nights".into(),
            "ambient nights".into(),
            "   ".into(),
        ]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].normalized, "ambient nights");
    }

    #[test]
    fn bitrate_for_resolution_maps_values() {
        assert_eq!(bitrate_for_resolution("1080p"), Some(6_000_000));
        assert_eq!(bitrate_for_resolution("720p"), Some(3_200_000));
        assert_eq!(bitrate_for_resolution("360p"), Some(1_000_000));
        assert_eq!(bitrate_for_resolution("abc"), None);
    }
}
