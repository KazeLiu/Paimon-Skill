# Tauri 2 Desktop Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a runnable Tauri 2 + Vue 3 desktop shell with persisted, fully editable prompt settings and the existing Genshin skill packaged as an optional default rule resource.

**Architecture:** The Vue renderer owns navigation and settings forms. Rust owns prompt resource discovery, persisted overrides, validation, and composition. Default prompt text lives in packaged Markdown files; the application code contains only resource IDs, ordering rules, and data contracts.

**Tech Stack:** Tauri 2, Rust, Vue 3, TypeScript, Vite, Vitest, `serde`, `serde_json`, `tempfile`.

---

## Task 1: Scaffold the desktop package

**Files:**
- Modify: `package.json`
- Create: `apps/desktop/package.json`
- Create: `apps/desktop/index.html`
- Create: `apps/desktop/tsconfig.json`
- Create: `apps/desktop/vite.config.ts`
- Create: `apps/desktop/src/main.ts`
- Create: `apps/desktop/src/App.vue`
- Create: `apps/desktop/src/styles.css`
- Create: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/build.rs`
- Create: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/src/main.rs`
- Create: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add root desktop scripts**

  Extend the existing root `scripts` object without changing `npm test`:

  ```json
  {
    "desktop:dev": "npm --prefix apps/desktop run tauri dev",
    "desktop:build": "npm --prefix apps/desktop run tauri build",
    "desktop:test": "npm --prefix apps/desktop test"
  }
  ```

- [ ] **Step 2: Create the desktop package manifest**

  Create `apps/desktop/package.json` with locally installed tooling:

  ```json
  {
    "name": "paimon-desktop",
    "version": "0.1.0",
    "private": true,
    "type": "module",
    "scripts": {
      "dev": "vite",
      "build": "vue-tsc --noEmit && vite build",
      "test": "vitest run",
      "tauri": "tauri"
    },
    "dependencies": {
      "@tauri-apps/api": "^2.0.0",
      "vue": "^3.5.0"
    },
    "devDependencies": {
      "@tauri-apps/cli": "^2.0.0",
      "@vitejs/plugin-vue": "^5.0.0",
      "@vue/tsconfig": "^0.7.0",
      "typescript": "~5.6.0",
      "vite": "^6.0.0",
      "vitest": "^2.1.0",
      "vue-tsc": "^2.2.0"
    }
  }
  ```

- [ ] **Step 3: Create the Vue entry point**

  `apps/desktop/src/main.ts`:

  ```ts
  import { createApp } from "vue";
  import App from "./App.vue";
  import "./styles.css";

  createApp(App).mount("#app");
  ```

  `apps/desktop/src/App.vue`:

  ```vue
  <script setup lang="ts">
  import { ref } from "vue";

  const activeView = ref<"chat" | "settings">("chat");
  </script>

  <template>
    <div class="app-shell">
      <aside class="sidebar">
        <strong>派蒙助手</strong>
        <button type="button" @click="activeView = 'chat'">对话</button>
        <button type="button" @click="activeView = 'settings'">设置</button>
      </aside>
      <main class="content">
        <section v-if="activeView === 'chat'">
          <h1>开始新对话</h1>
          <p>配置模型后即可直接提问。</p>
        </section>
        <section v-else>
          <h1>设置</h1>
          <p>在这里配置应用使用的系统提示词与规则资源。</p>
        </section>
      </main>
    </div>
  </template>
  ```

- [ ] **Step 4: Create the Tauri Rust shell**

  `apps/desktop/src-tauri/Cargo.toml`:

  ```toml
  [package]
  name = "paimon-desktop"
  version = "0.1.0"
  edition = "2021"

  [lib]
  name = "paimon_desktop_lib"
  crate-type = ["staticlib", "cdylib", "rlib"]

  [build-dependencies]
  tauri-build = { version = "2", features = [] }

  [dependencies]
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  tauri = { version = "2", features = [] }

  [dev-dependencies]
  tempfile = "3"
  ```

  `apps/desktop/src-tauri/src/main.rs`:

  ```rust
  fn main() {
      paimon_desktop_lib::run();
  }
  ```

  `apps/desktop/src-tauri/src/lib.rs`:

  ```rust
  #[cfg_attr(mobile, tauri::mobile_entry_point)]
  pub fn run() {
      tauri::Builder::default()
          .run(tauri::generate_context!())
          .expect("failed to run Paimon Desktop");
  }
  ```

- [ ] **Step 5: Install dependencies and verify the shell**

  Run:

  ```powershell
  npm install
  npm --prefix apps/desktop install
  npm run desktop:build
  ```

  Expected: the existing skill tests still install normally, Vue type-check succeeds, Rust compiles, and Tauri produces a Windows bundle.

- [ ] **Step 6: Commit the scaffold**

  ```powershell
  git add package.json package-lock.json apps/desktop
  git commit -m "feat: scaffold tauri desktop app"
  ```

## Task 2: Add prompt resources and a deterministic composer

**Files:**
- Create: `apps/desktop/resources/prompts/base-system.md`
- Create: `apps/desktop/resources/prompts/paimon-persona.md`
- Create: `apps/desktop/resources/prompts/tool-policy.md`
- Create: `apps/desktop/resources/rules/genshin-build-wiki/SKILL.md`
- Create: `apps/desktop/scripts/sync-skill.mjs`
- Modify: `apps/desktop/package.json`
- Create: `apps/desktop/src-tauri/src/prompt/mod.rs`
- Create: `apps/desktop/src-tauri/src/prompt/models.rs`
- Create: `apps/desktop/src-tauri/src/prompt/composer.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing Rust tests for prompt order and resource toggles**

  Add these tests to `prompt/composer.rs` before the implementation:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::prompt::models::{PromptFragment, PromptSettings};

      #[test]
      fn composes_enabled_fragments_in_declared_order() {
          let fragments = vec![
              PromptFragment::new("base", "base", true, 10),
              PromptFragment::new("persona", "persona", true, 20),
              PromptFragment::new("skill", "skill", true, 30),
          ];
          let settings = PromptSettings::default();

          let result = compose_prompt(&fragments, &settings).unwrap();

          assert_eq!(result.text, "base\n\npersona\n\nskill");
          assert_eq!(result.resource_ids, vec!["base", "persona", "skill"]);
      }

      #[test]
      fn skips_disabled_resource_and_appends_custom_suffix() {
          let fragments = vec![
              PromptFragment::new("base", "base", true, 10),
              PromptFragment::new("skill", "skill", true, 30),
          ];
          let settings = PromptSettings {
              disabled_resource_ids: vec!["skill".into()],
              custom_suffix: "custom".into(),
              ..PromptSettings::default()
          };

          let result = compose_prompt(&fragments, &settings).unwrap();

          assert_eq!(result.text, "base\n\ncustom");
          assert_eq!(result.resource_ids, vec!["base"]);
      }
  }
  ```

- [ ] **Step 2: Run the tests and verify failure**

  Run:

  ```powershell
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml prompt::composer
  ```

  Expected: compilation fails because `PromptFragment`, `PromptSettings`, and `compose_prompt` do not exist.

- [ ] **Step 3: Implement prompt models**

  `apps/desktop/src-tauri/src/prompt/models.rs`:

  ```rust
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct PromptFragment {
      pub id: String,
      pub content: String,
      pub enabled_by_default: bool,
      pub order: u32,
  }

  impl PromptFragment {
      pub fn new(id: &str, content: &str, enabled_by_default: bool, order: u32) -> Self {
          Self {
              id: id.into(),
              content: content.into(),
              enabled_by_default,
              order,
          }
      }
  }

  #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
  pub struct PromptSettings {
      pub overrides: std::collections::BTreeMap<String, String>,
      pub disabled_resource_ids: Vec<String>,
      pub custom_suffix: String,
      pub updated_at: Option<String>,
  }

  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct ComposedPrompt {
      pub text: String,
      pub resource_ids: Vec<String>,
  }
  ```

- [ ] **Step 4: Implement deterministic composition**

  `apps/desktop/src-tauri/src/prompt/composer.rs`:

  ```rust
  use super::models::{ComposedPrompt, PromptFragment, PromptSettings};

  pub fn compose_prompt(
      fragments: &[PromptFragment],
      settings: &PromptSettings,
  ) -> Result<ComposedPrompt, String> {
      let mut ordered = fragments.to_vec();
      ordered.sort_by_key(|fragment| fragment.order);

      let mut resource_ids = Vec::new();
      let mut sections = Vec::new();

      for fragment in ordered {
          if !fragment.enabled_by_default
              || settings.disabled_resource_ids.contains(&fragment.id)
          {
              continue;
          }

          let content = settings
              .overrides
              .get(&fragment.id)
              .cloned()
              .unwrap_or(fragment.content)
              .trim()
              .to_owned();

          if !content.is_empty() {
              resource_ids.push(fragment.id);
              sections.push(content);
          }
      }

      let suffix = settings.custom_suffix.trim();
      if !suffix.is_empty() {
          sections.push(suffix.to_owned());
      }

      if sections.is_empty() {
          return Err("系统提示词不能为空".into());
      }

      Ok(ComposedPrompt {
          text: sections.join("\n\n"),
          resource_ids,
      })
  }
  ```

- [ ] **Step 5: Add editable default resource files**

  Keep each Markdown file narrowly scoped. `base-system.md` defines evidence and safety behavior, `paimon-persona.md` defines the optional persona, and `tool-policy.md` explains that tool output and webpage text are untrusted data. Copy the current skill with a script instead of maintaining a second hand-edited copy:

  ```js
  import { cp, mkdir, rm } from "node:fs/promises";
  import { fileURLToPath } from "node:url";
  import path from "node:path";

  const desktopDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const repoDir = path.resolve(desktopDir, "..", "..");
  const source = path.join(repoDir, "skills", "genshin-build-wiki");
  const target = path.join(desktopDir, "resources", "rules", "genshin-build-wiki");

  await rm(target, { recursive: true, force: true });
  await mkdir(path.dirname(target), { recursive: true });
  await cp(source, target, { recursive: true });
  ```

  Add `"sync:skill": "node scripts/sync-skill.mjs"` and run it from `predev` and `prebuild`.

- [ ] **Step 6: Run tests and commit**

  ```powershell
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml prompt::composer
  npm --prefix apps/desktop run sync:skill
  git diff --no-index skills/genshin-build-wiki/SKILL.md apps/desktop/resources/rules/genshin-build-wiki/SKILL.md
  git add apps/desktop
  git commit -m "feat: add configurable prompt resources"
  ```

  Expected: Rust tests pass and `git diff --no-index` exits with no content difference.

## Task 3: Persist prompt settings through Tauri commands

**Files:**
- Create: `apps/desktop/src-tauri/src/prompt/repository.rs`
- Create: `apps/desktop/src-tauri/src/prompt/resources.rs`
- Create: `apps/desktop/src-tauri/src/commands/mod.rs`
- Create: `apps/desktop/src-tauri/src/commands/prompts.rs`
- Modify: `apps/desktop/src-tauri/src/prompt/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing persistence tests**

  Add tests using `tempfile::tempdir()` that save non-default text, reload it, reset it, and verify malformed JSON returns an error without overwriting the file:

  ```rust
  #[test]
  fn saves_loads_and_resets_settings() {
      let dir = tempfile::tempdir().unwrap();
      let repository = PromptRepository::new(dir.path().join("prompt-settings.json"));
      let settings = PromptSettings {
          custom_suffix: "回答保持简洁".into(),
          ..PromptSettings::default()
      };

      repository.save(&settings).unwrap();
      assert_eq!(repository.load().unwrap(), settings);

      repository.reset().unwrap();
      assert_eq!(repository.load().unwrap(), PromptSettings::default());
  }
  ```

- [ ] **Step 2: Run the tests and verify failure**

  ```powershell
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml prompt::repository
  ```

  Expected: compilation fails because `PromptRepository` is undefined.

- [ ] **Step 3: Implement atomic JSON persistence**

  `PromptRepository::save` serializes pretty JSON to a sibling `.tmp` file and renames it over the destination. `load` returns `PromptSettings::default()` only when the file does not exist; malformed or unreadable files return a user-visible error. `reset` removes only the exact settings file and leaves packaged resources untouched.

  Core write sequence:

  ```rust
  let temporary = self.path.with_extension("json.tmp");
  std::fs::write(&temporary, serde_json::to_vec_pretty(settings).map_err(to_error)?)
      .map_err(to_error)?;
  std::fs::rename(&temporary, &self.path).map_err(to_error)?;
  ```

- [ ] **Step 4: Expose prompt commands**

  Define commands with these exact signatures:

  ```rust
  #[tauri::command]
  pub fn get_prompt_settings(app: tauri::AppHandle) -> Result<PromptSettings, String>;

  #[tauri::command]
  pub fn save_prompt_settings(
      app: tauri::AppHandle,
      settings: PromptSettings,
  ) -> Result<ComposedPrompt, String>;

  #[tauri::command]
  pub fn reset_prompt_settings(app: tauri::AppHandle) -> Result<PromptSettings, String>;

  #[tauri::command]
  pub fn list_prompt_resources(app: tauri::AppHandle) -> Result<Vec<PromptFragment>, String>;
  ```

  Resolve the writable file under Tauri's application config directory and resolve defaults from packaged resources. Register all four commands in `generate_handler!`.

- [ ] **Step 5: Test and commit**

  ```powershell
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml prompt
  git add apps/desktop/src-tauri
  git commit -m "feat: persist prompt settings"
  ```

  Expected: composer and repository tests pass, including malformed JSON and reset coverage.

## Task 4: Build the prompt settings page

**Files:**
- Create: `apps/desktop/src/types/prompts.ts`
- Create: `apps/desktop/src/api/prompts.ts`
- Create: `apps/desktop/src/composables/usePromptSettings.ts`
- Create: `apps/desktop/src/components/PromptResourceEditor.vue`
- Create: `apps/desktop/src/pages/PromptSettingsPage.vue`
- Create: `apps/desktop/src/pages/PromptSettingsPage.test.ts`
- Modify: `apps/desktop/src/App.vue`

- [ ] **Step 1: Write the failing component test**

  Mock the prompt API and verify the page loads all resource editors, edits a resource override, disables the skill resource, saves, displays success, and restores defaults after confirmation:

  ```ts
  it("edits, saves, disables, and resets prompt resources", async () => {
    const wrapper = mount(PromptSettingsPage);
    await flushPromises();

    await wrapper.get('[data-resource-id="base-system"] textarea').setValue("custom base");
    await wrapper.get('[data-resource-id="genshin-build-wiki"] input[type="checkbox"]').setValue(false);
    await wrapper.get('[data-action="save-prompts"]').trigger("click");

    expect(savePromptSettings).toHaveBeenCalledWith(expect.objectContaining({
      overrides: expect.objectContaining({ "base-system": "custom base" }),
      disabled_resource_ids: ["genshin-build-wiki"]
    }));
    expect(wrapper.text()).toContain("已保存");
  });
  ```

- [ ] **Step 2: Run the component test and verify failure**

  ```powershell
  npm --prefix apps/desktop test -- PromptSettingsPage
  ```

  Expected: the test fails because the page and prompt API do not exist.

- [ ] **Step 3: Implement the prompt API and state composable**

  Use `invoke` with the exact Rust command names. Expose `load`, `save`, `reset`, `resources`, `settings`, `saving`, and `error`. Preserve unsaved editor values after a save failure.

  ```ts
  export const getPromptSettings = () =>
    invoke<PromptSettings>("get_prompt_settings");

  export const savePromptSettings = (settings: PromptSettings) =>
    invoke<ComposedPrompt>("save_prompt_settings", { settings });

  export const resetPromptSettings = () =>
    invoke<PromptSettings>("reset_prompt_settings");
  ```

- [ ] **Step 4: Implement the settings page**

  Render one editor per fragment with a checkbox, description, reset-this-fragment action, and textarea. Add a custom suffix editor, unsaved-change indicator, save button, and restore-all-defaults action. Display a read-only preview of the composed prompt IDs, not the API key or other unrelated settings.

- [ ] **Step 5: Run UI and Rust verification**

  ```powershell
  npm --prefix apps/desktop test -- PromptSettingsPage
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml prompt
  npm run desktop:build
  ```

  Expected: component tests pass, Rust prompt tests pass, and the desktop bundle builds.

- [ ] **Step 6: Manually verify persistence**

  Run `npm run desktop:dev`, edit the base prompt, disable the Genshin rule resource, save, restart the application, and confirm both settings persist. Restore defaults, restart again, and confirm packaged defaults return.

- [ ] **Step 7: Commit the settings UI**

  ```powershell
  git add apps/desktop/src apps/desktop/src-tauri
  git commit -m "feat: add editable prompt settings"
  ```

## Phase 1 Completion Gate

Run:

```powershell
npm test
npm run desktop:test
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
npm run desktop:build
git status --short
```

Completion requires:

- Existing skill validation tests still pass.
- Prompt composer and persistence tests pass.
- Prompt settings component tests pass.
- Windows desktop bundle builds.
- System prompt text is editable from settings and persists across restart.
- Reset restores packaged Markdown defaults.
- `skills/genshin-build-wiki/SKILL.md` remains the authoritative source copied by the build script.
- No API key, Cookie, or browser automation behavior has been introduced in Phase 1.
