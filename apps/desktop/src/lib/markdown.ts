import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";

const markdown = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: false,
});

// Parse links normally, then let DOMPurify enforce the final URI allowlist.
markdown.validateLink = () => true;

export function renderMarkdown(source: string) {
  return DOMPurify.sanitize(markdown.render(source), {
    ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):)/i,
    FORBID_TAGS: ["script", "style", "iframe", "object", "embed", "form"],
    FORBID_ATTR: ["style", "onerror", "onclick", "onload"],
  });
}
