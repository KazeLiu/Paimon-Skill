pub mod bilibili;
pub mod generic;
pub mod xiaohongshu;
pub mod zhihu;

use async_trait::async_trait;

pub use bilibili::BilibiliAdapter;
pub use generic::GenericAdapter;
pub use xiaohongshu::XiaohongshuAdapter;
pub use zhihu::ZhihuAdapter;

use super::models::{ResearchContext, ResearchError, ResearchReport, ResearchRequest};

#[async_trait]
pub trait ResearchAdapter {
    async fn research(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
    ) -> Result<ResearchReport, ResearchError>;
}
