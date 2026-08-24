use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct RunState {
    runs: Mutex<HashMap<String, RunEntry>>,
}

struct RunEntry {
    conversation_id: String,
    token: CancellationToken,
}

impl RunState {
    pub fn insert(&self, run_id: String, conversation_id: String, token: CancellationToken) {
        self.runs.lock().expect("run state poisoned").insert(
            run_id,
            RunEntry {
                conversation_id,
                token,
            },
        );
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        if let Some(entry) = self.runs.lock().expect("run state poisoned").get(run_id) {
            entry.token.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel_conversation(&self, conversation_id: &str) -> bool {
        let mut cancelled = false;
        for entry in self.runs.lock().expect("run state poisoned").values() {
            if entry.conversation_id == conversation_id {
                entry.token.cancel();
                cancelled = true;
            }
        }
        cancelled
    }

    pub async fn wait_for_conversation(&self, conversation_id: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let active = self
                .runs
                .lock()
                .expect("run state poisoned")
                .values()
                .any(|entry| entry.conversation_id == conversation_id);
            if !active {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    pub fn remove(&self, run_id: &str) {
        self.runs.lock().expect("run state poisoned").remove(run_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancels_and_waits_for_all_runs_in_a_conversation() {
        let state = RunState::default();
        let first = CancellationToken::new();
        let second = CancellationToken::new();
        state.insert("run-1".into(), "conversation-1".into(), first.clone());
        state.insert("run-2".into(), "conversation-1".into(), second.clone());
        state.insert(
            "run-3".into(),
            "conversation-2".into(),
            CancellationToken::new(),
        );

        assert!(state.cancel_conversation("conversation-1"));
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());

        state.remove("run-1");
        state.remove("run-2");
        assert!(
            state
                .wait_for_conversation("conversation-1", Duration::from_millis(50))
                .await
        );
        assert!(
            !state
                .wait_for_conversation("conversation-2", Duration::from_millis(1))
                .await
        );
    }
}
