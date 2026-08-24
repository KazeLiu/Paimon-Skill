import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

describe("Tauri capabilities", () => {
  it("allows event subscriptions only for the main application webview", () => {
    const capabilityPath = resolve(process.cwd(), "src-tauri/capabilities/main.json");

    expect(existsSync(capabilityPath)).toBe(true);
    const capability = JSON.parse(readFileSync(capabilityPath, "utf8"));

    expect(capability.webviews).toEqual(["main"]);
    expect(capability.windows).toBeUndefined();
    expect(capability.permissions).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
    ]);
  });
});
