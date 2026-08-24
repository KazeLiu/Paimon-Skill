import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";

const markdown = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: false,
});

const defaultLinkOpen = markdown.renderer.rules.link_open;
markdown.renderer.rules.link_open = (tokens, index, options, env, self) => {
  const token = tokens[index];
  token.attrSet("target", "_blank");
  token.attrSet("rel", "noopener noreferrer");
  return defaultLinkOpen
    ? defaultLinkOpen(tokens, index, options, env, self)
    : self.renderToken(tokens, index, options);
};

// Parse links normally, then let DOMPurify enforce the final URI allowlist.
markdown.validateLink = () => true;

export function renderMarkdown(source: string) {
  const sanitized = DOMPurify.sanitize(markdown.render(source), {
    ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):)/i,
    ALLOWED_ATTR: ["href", "target", "rel"],
    ADD_ATTR: ["target", "rel"],
    FORBID_TAGS: ["script", "style", "iframe", "object", "embed", "form"],
    FORBID_ATTR: ["style", "onerror", "onclick", "onload"],
  });
  return sanitized.replace(/<a\b(?![^>]*\btarget=)/gi, '<a target="_blank" rel="noopener noreferrer"');
}
