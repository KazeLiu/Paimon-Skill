use std::{collections::HashMap, sync::Mutex};

use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct RunState {
    runs: Mutex<HashMap<String, CancellationToken>>,
}

impl RunState {
    pub fn insert(&self, run_id: String, token: CancellationToken) {
        self.runs
            .lock()
            .expect("run state poisoned")
            .insert(run_id, token);
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        if let Some(token) = self.runs.lock().expect("run state poisoned").get(run_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn remove(&self, run_id: &str) {
        self.runs.lock().expect("run state poisoned").remove(run_id);
    }
}
