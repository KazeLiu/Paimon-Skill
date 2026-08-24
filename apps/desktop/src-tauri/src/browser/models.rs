use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_PAGE_TEXT_CHARS: usize = 16_000;
pub const MAX_PAGE_LINKS: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
    pub loading: bool,
    pub profile_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserEvent {
    pub kind: String,
    pub tab: BrowserTab,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLink {
    pub text: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSnapshot {
    pub title: String,
    pub url: String,
    pub text: String,
    pub links: Vec<PageLink>,
    pub login_challenge: bool,
    pub challenge_reason: Option<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub structure: Option<Value>,
    #[serde(default)]
    pub snapshot_generation: Option<u64>,
    #[serde(default)]
    pub page_fingerprint: Option<Value>,
    #[serde(default)]
    pub interactive_total: usize,
    #[serde(default)]
    pub shadow_root_count: usize,
}

impl PageSnapshot {
    pub fn bounded(
        title: String,
        url: String,
        text: String,
        links: Vec<PageLink>,
        login_challenge: bool,
        challenge_reason: Option<String>,
    ) -> Self {
        let text_was_truncated = text.chars().count() > MAX_PAGE_TEXT_CHARS;
        let links_were_truncated = links.len() > MAX_PAGE_LINKS;
        Self {
            title,
            url,
            text: text.chars().take(MAX_PAGE_TEXT_CHARS).collect(),
            links: links.into_iter().take(MAX_PAGE_LINKS).collect(),
            login_challenge,
            challenge_reason,
            truncated: text_was_truncated || links_were_truncated,
            structure: None,
            snapshot_generation: None,
            page_fingerprint: None,
            interactive_total: 0,
            shadow_root_count: 0,
        }
    }

    pub fn bound_in_place(&mut self) {
        let text_was_truncated = self.text.chars().count() > MAX_PAGE_TEXT_CHARS;
        let links_were_truncated = self.links.len() > MAX_PAGE_LINKS;
        if text_was_truncated {
            self.text = self.text.chars().take(MAX_PAGE_TEXT_CHARS).collect();
        }
        if links_were_truncated {
            self.links.truncate(MAX_PAGE_LINKS);
        }
        self.truncated |= text_was_truncated || links_were_truncated;
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollSample {
    pub scroll_y: f64,
    pub scroll_height: f64,
    pub viewport_height: f64,
}

impl ScrollSample {
    #[cfg(test)]
    fn new(scroll_y: f64, scroll_height: f64, viewport_height: f64) -> Self {
        Self {
            scroll_y,
            scroll_height,
            viewport_height,
        }
    }

    pub fn is_at_bottom(&self) -> bool {
        self.scroll_y + self.viewport_height + 4.0 >= self.scroll_height
    }
}

#[derive(Debug, Default)]
pub struct ScrollTracker {
    previous: Option<ScrollSample>,
    unchanged_samples: u8,
}

impl ScrollTracker {
    pub fn observe(&mut self, sample: ScrollSample) -> bool {
        let unchanged = self.previous.is_some_and(|previous| {
            (previous.scroll_y - sample.scroll_y).abs() < 1.0
                && (previous.scroll_height - sample.scroll_height).abs() < 1.0
                && (previous.viewport_height - sample.viewport_height).abs() < 1.0
                && previous.is_at_bottom() == sample.is_at_bottom()
        });
        self.unchanged_samples = if unchanged {
            self.unchanged_samples.saturating_add(1)
        } else {
            0
        };
        self.previous = Some(sample);
        self.unchanged_samples >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_tracker_stops_after_two_unchanged_samples() {
        let mut tracker = ScrollTracker::default();

        assert!(!tracker.observe(ScrollSample::new(100.0, 1000.0, 500.0)));
        assert!(!tracker.observe(ScrollSample::new(500.0, 1400.0, 500.0)));
        assert!(!tracker.observe(ScrollSample::new(500.0, 1400.0, 500.0)));
        assert!(tracker.observe(ScrollSample::new(500.0, 1400.0, 500.0)));
    }

    #[test]
    fn page_snapshot_is_bounded_before_returning_to_model() {
        let snapshot = PageSnapshot::bounded(
            "Example".into(),
            "https://example.com".into(),
            "x".repeat(MAX_PAGE_TEXT_CHARS + 20),
            vec![
                PageLink {
                    text: "Docs".into(),
                    url: "https://example.com/docs".into(),
                };
                MAX_PAGE_LINKS + 5
            ],
            false,
            None,
        );

        assert_eq!(snapshot.text.chars().count(), MAX_PAGE_TEXT_CHARS);
        assert_eq!(snapshot.links.len(), MAX_PAGE_LINKS);
        assert!(snapshot.truncated);
    }

    #[test]
    fn bounding_a_snapshot_keeps_the_semantic_structure() {
        let structure = serde_json::json!({"tree": {"type": "document"}});
        let mut snapshot = PageSnapshot {
            title: "Example".into(),
            url: "https://example.com".into(),
            text: "x".repeat(MAX_PAGE_TEXT_CHARS + 1),
            links: Vec::new(),
            login_challenge: false,
            challenge_reason: None,
            truncated: false,
            structure: Some(structure.clone()),
            snapshot_generation: Some(3),
            page_fingerprint: None,
            interactive_total: 1,
            shadow_root_count: 0,
        };

        snapshot.bound_in_place();

        assert_eq!(snapshot.structure, Some(structure));
        assert_eq!(snapshot.snapshot_generation, Some(3));
        assert!(snapshot.truncated);
    }
}
