import { describe, expect, it } from "vitest";

import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders GFM-style content and removes executable HTML", () => {
    const html = renderMarkdown("## 标题\n\n|A|B|\n|-|-|\n|1|2|\n\n<script>alert(1)</script>");

    expect(html).toContain("<h2>标题</h2>");
    expect(html).toContain("<table>");
    expect(html).not.toContain("<script");
  });

  it("rejects unsafe link protocols", () => {
    const html = renderMarkdown("[危险](javascript:alert(1)) [文件](file:///c:/secret.txt)");

    expect(html).not.toContain("javascript:");
    expect(html).not.toContain("file:");
  });

  it("opens every rendered link in a new window", () => {
    const html = renderMarkdown("[资料](https://example.com/page)");

    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });
});
