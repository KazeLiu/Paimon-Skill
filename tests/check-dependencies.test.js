const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const root = path.resolve(__dirname, '..');
const detector = path.join(root, 'skills', 'genshin-build-wiki', 'check-dependencies.js');

function runIsolated({ withOpenCli = false, communityCommands = [], json = true } = {}) {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'paimon-skill-test-'));
  const scriptPath = path.join(tempDir, 'check-dependencies.js');
  fs.copyFileSync(detector, scriptPath);

  if (withOpenCli) {
    if (process.platform === 'win32') {
      const commands = communityCommands
        .map((command) => `if "%1"=="${command}" exit /b 0\r\n`)
        .join('');
      fs.writeFileSync(
        path.join(tempDir, 'opencli.cmd'),
        `@echo off\r\nif "%1"=="--version" (echo 9.9.9-test & exit /b 0)\r\n${commands}exit /b 1\r\n`,
      );
    } else {
      const executable = path.join(tempDir, 'opencli');
      const commands = communityCommands.length
        ? `case "$1" in ${communityCommands.join('|')}) exit 0 ;; esac\n`
        : '';
      fs.writeFileSync(
        executable,
        `#!/bin/sh\nif [ "$1" = "--version" ]; then echo 9.9.9-test; exit 0; fi\n${commands}exit 1\n`,
        { mode: 0o755 },
      );
    }
  }

  const result = spawnSync(process.execPath, [scriptPath, ...(json ? ['--json'] : [])], {
    cwd: tempDir,
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: tempDir,
      Path: tempDir,
    },
  });

  const files = fs.readdirSync(tempDir).sort();
  fs.rmSync(tempDir, { recursive: true, force: true });
  return { ...result, files };
}

test('dependency detector contains no installation command', () => {
  const source = fs.readFileSync(detector, 'utf8');
  assert.doesNotMatch(source, /\b(?:npm|pnpm|yarn|pipx?|uv)\b[^\r\n]*(?:install|update|upgrade|add)\b/i);
});

test('missing OpenCLI is reported without failing or writing status', () => {
  const result = runIsolated();
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const report = JSON.parse(result.stdout);
  assert.equal(report.dependencies.opencli.installed, false);
  assert.deepEqual(result.files, ['check-dependencies.js']);
});

test('OpenCLI without required subcommands does not enable community lookup', () => {
  const result = runIsolated({ withOpenCli: true });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const report = JSON.parse(result.stdout);
  assert.equal(report.dependencies.opencli.installed, true);
  assert.match(report.dependencies.opencli.version, /9\.9\.9-test/);
  assert.equal(report.features.communityLookup, false);
  assert.equal(report.features.bilibiliLookup, false);
});

test('available community subcommands are returned as structured capabilities', () => {
  const result = runIsolated({
    withOpenCli: true,
    communityCommands: ['xiaohongshu', 'zhihu', 'bilibili', 'reddit'],
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const report = JSON.parse(result.stdout);
  assert.equal(report.features.xiaohongshuLookup, true);
  assert.equal(report.features.zhihuLookup, true);
  assert.equal(report.features.bilibiliLookup, true);
  assert.equal(report.features.redditLookup, true);
  assert.equal(report.features.communityLookup, true);
});

test('one missing platform capability is reported without hiding available platforms', () => {
  const result = runIsolated({
    withOpenCli: true,
    communityCommands: ['xiaohongshu', 'zhihu'],
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const report = JSON.parse(result.stdout);
  assert.equal(report.features.xiaohongshuLookup, true);
  assert.equal(report.features.zhihuLookup, true);
  assert.equal(report.features.bilibiliLookup, false);
  assert.equal(report.features.redditLookup, false);
  assert.equal(report.features.communityLookup, true);
});

test('human-readable output asks for consent when platform capabilities are missing', () => {
  const result = runIsolated({
    withOpenCli: true,
    communityCommands: ['xiaohongshu'],
    json: false,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /ask the user before (?:installing|updating)/i);
});
