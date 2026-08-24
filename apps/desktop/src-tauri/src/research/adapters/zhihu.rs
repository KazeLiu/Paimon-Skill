use super::ResearchAdapter;
use crate::research::{
    executor::ResearchToolExecutor,
    models::{
        ResearchAction, ResearchActionRequest, ResearchArtifact, ResearchArtifactKind,
        ResearchContext, ResearchError, ResearchReport, ResearchRequest,
    },
    zhihu_fields::{
        expand_element_id, extract_zhihu_fields, format_zhihu_body, validate_zhihu_detail_url,
        ZhihuPageKind,
    },
};
use async_trait::async_trait;

pub struct ZhihuAdapter<'a, E> {
    executor: &'a E,
}

impl<'a, E> ZhihuAdapter<'a, E> {
    pub fn new(executor: &'a E) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl<E: ResearchToolExecutor> ResearchAdapter for ZhihuAdapter<'_, E> {
    async fn research(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
    ) -> Result<ResearchReport, ResearchError> {
        let detail = validate_zhihu_detail_url(&request.url)
            .map_err(|error| ResearchError::new("invalid_zhihu_url", error))?;
        let mut tab_id = None;
        let result = self.run(&request, &context, detail.kind, &mut tab_id).await;
        if let Err(error) = &result {
            let _ = self.close(tab_id).await;
            return Err(error.clone());
        }
        let mut report = result.expect("checked successful result");
        self.close(tab_id).await?;
        report.url = detail.safe_url;
        Ok(report)
    }
}

impl<E: ResearchToolExecutor> ZhihuAdapter<'_, E> {
    async fn execute(
        &self,
        mut action: ResearchActionRequest,
        tab_id: &Option<String>,
        context: &ResearchContext,
    ) -> Result<Option<ResearchArtifact>, ResearchError> {
        if context.cancellation.is_cancelled() {
            return Err(ResearchError::new("cancelled", "研究任务已取消"));
        }
        if action.action != ResearchAction::OpenPage && action.tab_id.is_none() {
            action.tab_id = tab_id.clone();
        }
        self.executor.execute(action).await
    }

    async fn close(&self, tab_id: Option<String>) -> Result<(), ResearchError> {
        if tab_id.is_none() {
            return Ok(());
        }
        self.executor
            .execute(ResearchActionRequest::tab(
                ResearchAction::ClosePage,
                tab_id,
            ))
            .await
            .map(|_| ())
    }

    async fn run(
        &self,
        request: &ResearchRequest,
        context: &ResearchContext,
        page_kind: ZhihuPageKind,
        tab_id: &mut Option<String>,
    ) -> Result<ResearchReport, ResearchError> {
        let opened = self
            .execute(
                ResearchActionRequest::open(request.url.clone()),
                tab_id,
                context,
            )
            .await?
            .ok_or_else(|| {
                ResearchError::new("missing_open_artifact", "打开知乎详情页没有返回页面")
            })?;
        *tab_id = opened.metadata.get("tabId").cloned();
        let tab = tab_id.clone();

        let mut find = self
            .execute(
                ResearchActionRequest::find_context(tab.clone(), "阅读全文".into()),
                &tab,
                context,
            )
            .await?
            .ok_or_else(|| ResearchError::new("missing_context", "知乎页面没有返回语义上下文"))?;
        if expand_element_id(&find.content).is_none()
            && ["展开", "显示全部", "查看全部"]
                .iter()
                .any(|label| opened.content.contains(label))
        {
            find = self
                .execute(
                    ResearchActionRequest::find_context(tab.clone(), "展开".into()),
                    &tab,
                    context,
                )
                .await?
                .ok_or_else(|| {
                    ResearchError::new("missing_context", "知乎展开控件查找没有返回结果")
                })?;
        }
        if let Some(element_id) = expand_element_id(&find.content) {
            match self
                .execute(
                    ResearchActionRequest::click_element(tab.clone(), element_id),
                    &tab,
                    context,
                )
                .await
            {
                Ok(_) => {}
                Err(error) if is_stale_element_error(&error) => {
                    let rescanned = self
                        .execute(
                            ResearchActionRequest::tab(ResearchAction::ExtractPage, tab.clone()),
                            &tab,
                            context,
                        )
                        .await?
                        .ok_or_else(|| {
                            ResearchError::new("missing_rescan", "知乎页面重扫没有返回快照")
                        })?;
                    find = self
                        .execute(
                            ResearchActionRequest::find_context(tab.clone(), "阅读全文".into()),
                            &tab,
                            context,
                        )
                        .await?
                        .unwrap_or(rescanned);
                    if let Some(element_id) = expand_element_id(&find.content) {
                        self.execute(
                            ResearchActionRequest::click_element(tab.clone(), element_id),
                            &tab,
                            context,
                        )
                        .await?;
                    }
                }
                Err(error) => return Err(error),
            }
        }

        let mut scroll = ResearchActionRequest::tab(ResearchAction::ScrollPage, tab.clone());
        scroll.max_scroll_rounds = context.max_scroll_rounds;
        self.execute(scroll, &tab, context).await?;
        let raw_body = self
            .execute(
                ResearchActionRequest::tab(ResearchAction::ExtractPage, tab.clone()),
                &tab,
                context,
            )
            .await?
            .ok_or_else(|| ResearchError::new("missing_body", "知乎页面没有返回正文快照"))?;
        let fields = extract_zhihu_fields(&raw_body.content);
        let Some(body_text) = fields
            .body
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return Err(ResearchError::new("empty_body", "知乎详情页正文为空"));
        };
        let mut metadata = raw_body.metadata.clone();
        metadata.insert("pageType".into(), page_kind.as_str().into());
        metadata.insert("fieldExtraction".into(), "zhihu-semantic-html".into());
        if let Some(value) = &fields.author {
            metadata.insert("author".into(), value.clone());
        }
        if let Some(value) = &fields.published_at {
            metadata.insert("publishedAt".into(), value.clone());
        }
        if let Some(value) = &fields.updated_at {
            metadata.insert("updatedAt".into(), value.clone());
        }
        if fields.visual_candidate {
            metadata.insert("visualCandidate".into(), "true".into());
        }
        for (key, value) in &fields.interactions {
            metadata.insert(format!("interaction_{key}"), value.clone());
        }
        let body = ResearchArtifact {
            kind: ResearchArtifactKind::Body,
            title: fields.title.clone().unwrap_or(raw_body.title),
            url: raw_body.url,
            content: format_zhihu_body(&fields, body_text, page_kind),
            source_level: raw_body.source_level,
            captured_at: raw_body.captured_at,
            metadata,
        };
        let mut report = ResearchReport::empty(request);
        report.add_artifact(body);
        for (field, value) in [
            ("作者", fields.author.is_some()),
            ("发布时间", fields.published_at.is_some()),
            ("更新时间", fields.updated_at.is_some()),
        ] {
            if !value {
                report
                    .warnings
                    .push(format!("知乎详情未提取到{field}，未进行猜测"));
            }
        }
        if fields.visual_candidate {
            report.warnings.push(
                "知乎正文包含图片、表格或图表，已记录视觉分析候选；本阶段未调用视觉模型".into(),
            );
        }
        if request.wants_comments {
            match self
                .execute(
                    ResearchActionRequest::tab(ResearchAction::ExtractComments, tab.clone()),
                    &tab,
                    context,
                )
                .await
            {
                Ok(Some(comments)) => report.add_artifact(comments),
                Ok(None) => report.warnings.push("知乎评论提取没有返回结果".into()),
                Err(error) if error.code == "blocked" => report
                    .warnings
                    .push("知乎评论受登录、验证码或风控限制，正文仍可使用".into()),
                Err(error) => return Err(error),
            }
        }
        Ok(report)
    }
}

fn is_stale_element_error(error: &ResearchError) -> bool {
    error.message.contains("元素 ID") || error.message.contains("过期")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
