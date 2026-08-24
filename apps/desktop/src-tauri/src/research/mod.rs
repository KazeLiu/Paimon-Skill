pub mod adapters;
pub mod agent_tools;
pub mod executor;
pub mod media;
pub mod model_tasks;
pub mod models;
pub mod planner;
pub mod router;
pub mod tauri_executor;
pub(crate) mod zhihu_fields;

pub use adapters::{
    BilibiliAdapter, GenericAdapter, ResearchAdapter, XiaohongshuAdapter, ZhihuAdapter,
};
pub use executor::ResearchToolExecutor;
pub use models::{ResearchContext, ResearchReport, ResearchRequest};

#[allow(dead_code)]
pub async fn research_source<E: ResearchToolExecutor>(
    request: ResearchRequest,
    context: ResearchContext,
    executor: &E,
) -> Result<ResearchReport, models::ResearchError> {
    match router::route(&request)? {
        router::AdapterKind::Generic => {
            GenericAdapter::new(executor)
                .research(request, context)
                .await
        }
        router::AdapterKind::Zhihu => ZhihuAdapter::new(executor).research(request, context).await,
        router::AdapterKind::Xiaohongshu => {
            XiaohongshuAdapter::new(executor)
                .research(request, context)
                .await
        }
        router::AdapterKind::Bilibili => {
            BilibiliAdapter::new(executor)
                .research(request, context)
                .await
        }
    }
}

pub async fn research_source_with_visual<E: ResearchToolExecutor>(
    request: ResearchRequest,
    context: ResearchContext,
    executor: &E,
    visual: Option<&dyn model_tasks::VisualTaskExecutor>,
) -> Result<ResearchReport, models::ResearchError> {
    match router::route(&request)? {
        router::AdapterKind::Bilibili => {
            let adapter = BilibiliAdapter::new(executor);
            match visual {
                Some(visual) => adapter.research_with_visual(request, context, visual).await,
                None => adapter.research(request, context).await,
            }
        }
        router::AdapterKind::Xiaohongshu => {
            let adapter = XiaohongshuAdapter::new(executor);
            match visual {
                Some(visual) => adapter.research_with_visual(request, context, visual).await,
                None => adapter.research(request, context).await,
            }
        }
        router::AdapterKind::Generic => {
            GenericAdapter::new(executor)
                .research(request, context)
                .await
        }
        router::AdapterKind::Zhihu => ZhihuAdapter::new(executor).research(request, context).await,
    }
}
