use async_trait::async_trait;

use super::models::{ResearchActionRequest, ResearchArtifact, ResearchContext, ResearchError};

#[async_trait]
pub trait ResearchToolExecutor: Send + Sync {
    /// Read-only ownership information for diagnostic cleanup after an open
    /// step creates a tab but fails before returning its artifact.
    fn active_tab_id(&self) -> Option<String> { None }

    async fn execute(
        &self,
        request: ResearchActionRequest,
    ) -> Result<Option<ResearchArtifact>, ResearchError>;

    async fn cleanup(&self) -> Result<(), ResearchError> {
        Ok(())
    }
}

pub async fn execute_plan<E: ResearchToolExecutor>(
    executor: &E,
    mut plan: Vec<ResearchActionRequest>,
    context: &ResearchContext,
) -> Result<Vec<ResearchArtifact>, ResearchError> {
    let mut artifacts = Vec::new();
    let close_request = plan
        .iter()
        .find(|request| request.action == super::models::ResearchAction::ClosePage)
        .cloned();
    let mut active_tab_id = None;
    for request in &mut plan {
        if request.action != super::models::ResearchAction::OpenPage && request.tab_id.is_none() {
            request.tab_id = active_tab_id.clone();
        }
        if context.cancellation.is_cancelled() {
            if let Some(close) = close_request_for(&close_request, active_tab_id.as_deref()) {
                let _ = executor.execute(close).await;
            } else {
                let _ = executor.cleanup().await;
            }
            return Err(ResearchError::new("cancelled", "研究任务已取消"));
        }
        let execution = tokio::select! {
            _ = context.cancellation.cancelled() => {
                let _ = executor.cleanup().await;
                return Err(ResearchError::new("cancelled", "研究任务已取消"));
            }
            result = executor.execute(request.clone()) => result,
        };
        match execution {
            Ok(Some(artifact)) => {
                if request.action == super::models::ResearchAction::OpenPage {
                    active_tab_id = artifact.metadata.get("tabId").cloned();
                }
                artifacts.push(artifact);
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(close) = close_request_for(&close_request, active_tab_id.as_deref()) {
                    let _ = executor.execute(close).await;
                } else {
                    let _ = executor.cleanup().await;
                }
                return Err(error);
            }
        }
    }
    Ok(artifacts)
}

fn close_request_for(
    planned: &Option<ResearchActionRequest>,
    active_tab_id: Option<&str>,
) -> Option<ResearchActionRequest> {
    planned.clone().and_then(|mut request| {
        if request.tab_id.is_none() {
            request.tab_id = active_tab_id.map(str::to_owned);
        }
        request.tab_id.as_ref()?;
        Some(request)
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::research::models::{ResearchAction, ResearchActionRequest};

    struct FakeExecutor {
        calls: Mutex<Vec<ResearchActionRequest>>,
        results: Mutex<VecDeque<Result<Option<ResearchArtifact>, ResearchError>>>,
        cancel_after_first: Option<CancellationToken>,
        cleanup_calls: Mutex<usize>,
    }

    #[async_trait]
    impl ResearchToolExecutor for FakeExecutor {
        async fn execute(
            &self,
            request: ResearchActionRequest,
        ) -> Result<Option<ResearchArtifact>, ResearchError> {
            self.calls.lock().unwrap().push(request);
            let result = self.results.lock().unwrap().pop_front().unwrap();
            if self.calls.lock().unwrap().len() == 1 {
                if let Some(token) = &self.cancel_after_first {
                    token.cancel();
                }
            }
            result
        }

        async fn cleanup(&self) -> Result<(), ResearchError> {
            *self.cleanup_calls.lock().unwrap() += 1;
            Ok(())
        }
    }

    fn artifact() -> ResearchArtifact {
        ResearchArtifact {
            kind: super::super::models::ResearchArtifactKind::Body,
            title: "页面".into(),
            url: "https://example.com".into(),
            content: "正文".into(),
            source_level: "web".into(),
            captured_at: "now".into(),
            metadata: Default::default(),
        }
    }

    fn opened_artifact() -> ResearchArtifact {
        let mut artifact = artifact();
        artifact
            .metadata
            .insert("tabId".into(), "research-tab".into());
        artifact
    }

    #[tokio::test]
    async fn executor_closes_after_a_failed_step() {
        let executor = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([
                Ok(Some(opened_artifact())),
                Err(ResearchError::new("blocked", "登录阻断")),
                Ok(None),
            ])),
            cancel_after_first: None,
            cleanup_calls: Mutex::new(0),
        };
        let plan = vec![
            ResearchActionRequest::open("https://example.com".into()),
            ResearchActionRequest::tab(ResearchAction::ExtractPage, None),
            ResearchActionRequest::tab(ResearchAction::ClosePage, None),
        ];
        let result = execute_plan(&executor, plan, &ResearchContext::default()).await;
        assert!(result.is_err());
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls[0].action, ResearchAction::OpenPage);
        assert_eq!(calls[1].action, ResearchAction::ExtractPage);
        assert_eq!(calls[1].tab_id.as_deref(), Some("research-tab"));
        assert_eq!(calls[2].action, ResearchAction::ClosePage);
        assert_eq!(calls[2].tab_id.as_deref(), Some("research-tab"));
    }

    #[tokio::test]
    async fn executor_closes_the_opened_tab_when_cancelled_before_next_step() {
        let context = ResearchContext::default();
        let executor = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([Ok(Some(opened_artifact())), Ok(None)])),
            cancel_after_first: Some(context.cancellation.clone()),
            cleanup_calls: Mutex::new(0),
        };
        let plan = vec![
            ResearchActionRequest::open("https://example.com".into()),
            ResearchActionRequest::tab(ResearchAction::ExtractPage, None),
            ResearchActionRequest::tab(ResearchAction::ClosePage, None),
        ];
        let result = execute_plan(&executor, plan, &context).await;
        assert_eq!(result.unwrap_err().code, "cancelled");
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].action, ResearchAction::OpenPage);
        assert_eq!(calls[1].action, ResearchAction::ClosePage);
        assert_eq!(calls[1].tab_id.as_deref(), Some("research-tab"));
        assert_eq!(*executor.cleanup_calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn executor_propagates_opened_tab_id_to_successful_close() {
        let executor = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([
                Ok(Some(opened_artifact())),
                Ok(Some(artifact())),
                Ok(None),
            ])),
            cancel_after_first: None,
            cleanup_calls: Mutex::new(0),
        };
        let plan = vec![
            ResearchActionRequest::open("https://example.com".into()),
            ResearchActionRequest::tab(ResearchAction::ExtractPage, None),
            ResearchActionRequest::tab(ResearchAction::ClosePage, None),
        ];
        execute_plan(&executor, plan, &ResearchContext::default())
            .await
            .unwrap();
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls[1].tab_id.as_deref(), Some("research-tab"));
        assert_eq!(calls[2].tab_id.as_deref(), Some("research-tab"));
    }
}
