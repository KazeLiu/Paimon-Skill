use super::models::{ResearchAction, ResearchActionRequest, ResearchRequest};

pub fn plan_generic(
    request: &ResearchRequest,
    max_scroll_rounds: u8,
) -> Vec<ResearchActionRequest> {
    let mut plan = vec![ResearchActionRequest::open(request.url.clone())];
    let tab = |action| ResearchActionRequest::tab(action, None);
    let mut scroll = tab(ResearchAction::ScrollPage);
    scroll.max_scroll_rounds = max_scroll_rounds;
    plan.push(scroll);
    plan.push(tab(ResearchAction::ExtractPage));
    if request.wants_comments {
        plan.push(tab(ResearchAction::ExtractComments));
    }
    plan.push(tab(ResearchAction::ClosePage));
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(wants_comments: bool) -> ResearchRequest {
        ResearchRequest {
            source_id: None,
            url: "https://example.com/article".into(),
            query: "测试".into(),
            wants_comments,
            wants_visual: false,
        }
    }

    #[test]
    fn generic_plan_keeps_body_and_comments_as_separate_steps() {
        let plan = plan_generic(&request(true), 6);
        assert_eq!(plan[0].action, ResearchAction::OpenPage);
        assert_eq!(plan[1].action, ResearchAction::ScrollPage);
        assert_eq!(plan[2].action, ResearchAction::ExtractPage);
        assert_eq!(plan[3].action, ResearchAction::ExtractComments);
        assert_eq!(plan[4].action, ResearchAction::ClosePage);
    }
}
