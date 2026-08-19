use std::sync::Mutex;

use super::models::{BrowserBounds, BrowserTab};

#[derive(Debug, Default)]
struct BrowserStateInner {
    tabs: Vec<BrowserTab>,
    active_id: Option<String>,
    bounds: Option<BrowserBounds>,
    visible: bool,
}

#[derive(Debug, Default)]
pub struct BrowserState(Mutex<BrowserStateInner>);

impl BrowserState {
    pub fn list(&self) -> Vec<BrowserTab> {
        self.0.lock().expect("browser state poisoned").tabs.clone()
    }

    pub fn insert(&self, tab: BrowserTab) {
        let mut state = self.0.lock().expect("browser state poisoned");
        state.active_id = Some(tab.id.clone());
        state.tabs.push(tab);
    }

    pub fn update(&self, id: &str, update: impl FnOnce(&mut BrowserTab)) -> Option<BrowserTab> {
        let mut state = self.0.lock().expect("browser state poisoned");
        let tab = state.tabs.iter_mut().find(|tab| tab.id == id)?;
        update(tab);
        Some(tab.clone())
    }

    pub fn remove(&self, id: &str) -> Result<Option<String>, String> {
        let mut state = self.0.lock().expect("browser state poisoned");
        let index = state
            .tabs
            .iter()
            .position(|tab| tab.id == id)
            .ok_or_else(|| "浏览器标签不存在".to_string())?;
        state.tabs.remove(index);
        if state.active_id.as_deref() == Some(id) {
            state.active_id = state
                .tabs
                .get(index.saturating_sub(1))
                .or_else(|| state.tabs.first())
                .map(|tab| tab.id.clone());
        }
        Ok(state.active_id.clone())
    }

    pub fn select(&self, id: &str) -> Result<(), String> {
        let mut state = self.0.lock().expect("browser state poisoned");
        if !state.tabs.iter().any(|tab| tab.id == id) {
            return Err("浏览器标签不存在".into());
        }
        state.active_id = Some(id.to_string());
        Ok(())
    }

    pub fn active_id(&self) -> Option<String> {
        self.0
            .lock()
            .expect("browser state poisoned")
            .active_id
            .clone()
    }

    pub fn set_bounds(&self, bounds: BrowserBounds) {
        self.0.lock().expect("browser state poisoned").bounds = Some(bounds);
    }

    pub fn bounds(&self) -> Option<BrowserBounds> {
        self.0.lock().expect("browser state poisoned").bounds
    }

    pub fn set_visible(&self, visible: bool) {
        self.0.lock().expect("browser state poisoned").visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.0.lock().expect("browser state poisoned").visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::models::BrowserTab;

    fn tab(id: &str) -> BrowserTab {
        BrowserTab {
            id: id.into(),
            title: id.into(),
            url: format!("https://{id}.example.com"),
            loading: false,
            profile_id: "default".into(),
        }
    }

    #[test]
    fn removing_active_tab_selects_the_previous_remaining_tab() {
        let state = BrowserState::default();
        state.insert(tab("first"));
        state.insert(tab("second"));

        let next = state.remove("second").unwrap();

        assert_eq!(next.as_deref(), Some("first"));
        assert_eq!(state.active_id().as_deref(), Some("first"));
    }

    #[test]
    fn visibility_does_not_clear_tabs() {
        let state = BrowserState::default();
        state.insert(tab("first"));

        state.set_visible(false);
        state.set_visible(true);

        assert_eq!(state.list().len(), 1);
        assert!(state.is_visible());
    }
}
