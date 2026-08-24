use async_trait::async_trait;

use super::super::{
    executor::{execute_plan, ResearchToolExecutor},
    models::{
        ResearchArtifactKind, ResearchContext, ResearchError, ResearchReport, ResearchRequest,
    },
    planner::plan_generic,
};
use super::ResearchAdapter;

pub struct GenericAdapter<'a, E> {
    executor: &'a E,
}

impl<'a, E> GenericAdapter<'a, E> {
    pub fn new(executor: &'a E) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl<E: ResearchToolExecutor> ResearchAdapter for GenericAdapter<'_, E> {
    async fn research(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
    ) -> Result<ResearchReport, ResearchError> {
        let plan = plan_generic(&request, context.max_scroll_rounds);
        let artifacts = execute_plan(self.executor, plan, &context).await?;
        let mut report = ResearchReport::empty(&request);
        for artifact in artifacts {
            if artifact.kind != ResearchArtifactKind::Body
                && artifact.kind != ResearchArtifactKind::Comments
            {
                continue;
            }
            report.add_artifact(artifact);
        }
        if report.body.is_none() {
            report.warnings.push("未取得网页正文".into());
        }
        Ok(report)
    }
}
