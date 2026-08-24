use std::{collections::HashSet, sync::Mutex};

use super::models::BrowserTab;

#[derive(Debug, Default)]
struct BrowserStateInner {
    tabs: Vec<BrowserTab>,
    active_id: Option<String>,
    temporary: HashSet<String>,
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

    pub fn mark_temporary(&self, id: &str) {
        self.0
            .lock()
            .expect("browser state poisoned")
            .temporary
            .insert(id.to_string());
    }

    pub fn temporary_ids(&self) -> Vec<String> {
        self.0
            .lock()
            .expect("browser state poisoned")
            .temporary
            .iter()
            .cloned()
            .collect()
    }

    pub fn is_temporary(&self, id: &str) -> bool {
        self.0
            .lock()
            .expect("browser state poisoned")
            .temporary
            .contains(id)
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
        state.temporary.remove(id);
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

    pub fn clear(&self) -> Vec<BrowserTab> {
        let mut state = self.0.lock().expect("browser state poisoned");
        state.active_id = None;
        state.temporary.clear();
        std::mem::take(&mut state.tabs)
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
    fn clear_removes_tabs_and_active_selection() {
        let state = BrowserState::default();
        state.insert(tab("first"));
        state.insert(tab("second"));

        let cleared = state.clear();

        assert_eq!(cleared.len(), 2);
        assert!(state.list().is_empty());
        assert_eq!(state.active_id(), None);
    }

    #[test]
    fn temporary_marker_is_removed_with_the_tab() {
        let state = BrowserState::default();
        state.insert(tab("reader"));
        state.mark_temporary("reader");
        assert!(state.is_temporary("reader"));

        state.remove("reader").unwrap();

        assert!(!state.is_temporary("reader"));
    }
}
