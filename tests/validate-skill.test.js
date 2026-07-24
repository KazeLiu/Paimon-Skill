const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const root = path.resolve(__dirname, '..');
const read = (...parts) => fs.readFileSync(path.join(root, ...parts), 'utf8');
const skill = read('skills', 'genshin-build-wiki', 'SKILL.md');

test('SKILL frontmatter is portable and intent-based', () => {
  const frontmatter = skill.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1];
  assert.ok(frontmatter, 'frontmatter not found');
  const topLevelKeys = [...frontmatter.matchAll(/^([a-z][a-z0-9-]*):/gm)].map((match) => match[1]);
  assert.deepEqual(topLevelKeys, ['name', 'description']);
  assert.match(frontmatter, /description:\s*>?-?\s*\r?\n?\s*Use when/i);
  assert.doesNotMatch(frontmatter, /AUTO-TRIGGER|ANY form/i);
});

test('SKILL markdown code fences are balanced', () => {
  const fences = skill.match(/^\s*```/gm) ?? [];
  assert.equal(fences.length % 2, 0, `found ${fences.length} code fences`);
});

test('community commands use the single declared OpenCLI dependency', () => {
  const social = read('skills', 'genshin-build-wiki', 'references', 'social-platforms.md');
  for (const source of [skill, social]) {
    assert.doesNotMatch(source, /\bbili-cli\b|\bbili\s+search\b/i);
  }
  assert.match(`${skill}\n${social}`, /opencli\s+bilibili\s+search/i);
});

test('Xiaohongshu detail lookup preserves the complete search URL', () => {
  const social = read('skills', 'genshin-build-wiki', 'references', 'social-platforms.md');
  assert.match(social, /opencli\s+xiaohongshu\s+note\s+["']NOTE_URL["']\s+-f\s+json/i);
  assert.match(social, /xsec_token/);
  assert.doesNotMatch(social, /note\s+["']?(?:<)?note_id/i);
});

test('source authority and freshness are explicit', () => {
  assert.match(skill, /第一方/);
  assert.match(skill, /社区维护/);
  assert.match(skill, /查询时间/);
  assert.match(skill, /Asia\/Shanghai|时区/);
  assert.doesNotMatch(skill, /4\.x\s*版本/i);
});

test('stable facts do not require community lookup', () => {
  assert.match(skill, /稳定事实[\s\S]{0,300}(?:不查|无需查询)社区/);
});

test('all maintained references are discoverable from SKILL.md', () => {
  for (const file of ['sources.md', 'query-cheatsheet.md', 'social-platforms.md']) {
    assert.match(skill, new RegExp(`references/${file.replace('.', '\\.')}\\b`));
  }
});

test('generated dependency state is not committed as source', () => {
  assert.equal(
    fs.existsSync(path.join(root, 'skills', 'genshin-build-wiki', '.dependency-status.json')),
    false,
  );
});

test('installation docs avoid unsafe recursive replacement', () => {
  const guide = read('INSTALLATION_GUIDE.md');
  assert.doesNotMatch(guide, /Remove-Item\s+\$target\s+-Recurse\s+-Force/i);
  assert.match(guide, /LinkType/);
});

test('license and dependency claims are consistent', () => {
  assert.equal(fs.existsSync(path.join(root, 'LICENSE')), true);
  for (const source of [read('README.md'), read('INSTALLATION_GUIDE.md')]) {
    assert.doesNotMatch(source, /bili-cli/i);
    assert.doesNotMatch(source, /自动安装依赖|自动执行\s*`?npm install/i);
  }
});

test('all install docs require consent before repairing a missing platform capability', () => {
  for (const source of [
    read('README.md'),
    read('INSTALLATION_GUIDE.md'),
    read('skills', 'genshin-build-wiki', 'INSTALL.md'),
  ]) {
    assert.match(source, /平台能力|平台子命令/);
    assert.match(source, /明确同意[^。\n]*(?:更新|修复)|(?:更新|修复)[^。\n]*明确同意/);
  }
});
