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

test('character story and lore questions trigger a dedicated lookup route', () => {
  const frontmatter = skill.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1] ?? '';
  assert.match(frontmatter, /stor(?:y|ies)|lore|quest/i);
  assert.match(skill, /人物故事|角色故事|剧情考据/);
  assert.match(skill, /references\/story-research\.md\b/);
});

test('story research discovers and verifies evidence across wiki pages', () => {
  const storyPath = path.join(
    root,
    'skills',
    'genshin-build-wiki',
    'references',
    'story-research.md',
  );
  assert.equal(fs.existsSync(storyPath), true, 'story-research.md not found');
  const story = fs.readFileSync(storyPath, 'utf8');

  assert.match(story, /searchwiki\.biligame\.com\/ys\/index\.php\?search=/);
  assert.match(story, /URL 编码|URL-encode/i);
  assert.match(story, /offset|后\s*\d+\s*个|下一页/);
  for (const pageType of ['魔神任务', '传说任务', '世界任务', '邀约任务', '活动任务', '物品描述']) {
    assert.match(story, new RegExp(pageType));
  }
  assert.match(story, /搜索结果[^。\n]*(?:候选|线索)/);
  assert.match(story, /(?:打开|读取)[^。\n]*(?:具体|命中)[^。\n]*页面/);
  assert.match(story, /(?:明确事实|直接证据)[\s\S]{0,200}(?:推断|推测)/);
});

test('lore research supports multilingual names and cultural references', () => {
  const story = read('skills', 'genshin-build-wiki', 'references', 'story-research.md');
  const sources = read('skills', 'genshin-build-wiki', 'references', 'sources.md');

  for (const name of ['Genshin Impact Wiki', 'Project Amber', 'Etymology', 'Other Languages']) {
    assert.match(`${story}\n${sources}`, new RegExp(name, 'i'));
  }
  assert.match(story, /英语|英文/);
  assert.match(story, /俄语|俄文/);
  assert.match(story, /本地化[^。\n]*(?:原文|源文本)|(?:原文|源文本)[^。\n]*本地化/);
  assert.match(story, /游戏文本[^。\n]*(?:词源|文化原型|历史对应)/);
  assert.match(story, /(?:明确事实|直接证据)[\s\S]{0,300}(?:社区推测|玩家推测)/);
});

test('Zhihu research reads individual answers without write commands', () => {
  const social = read('skills', 'genshin-build-wiki', 'references', 'social-platforms.md');

  assert.match(social, /opencli\s+zhihu\s+search\s+["'][^"']+["'][^\n]*--type\s+answer/i);
  assert.match(social, /opencli\s+zhihu\s+question\s+["']?QUESTION_ID["']?[^\n]*--limit/i);
  assert.match(social, /opencli\s+zhihu\s+answer-detail\s+["']?ANSWER_ID["']?[^\n]*-f\s+json/i);
  assert.doesNotMatch(social, /opencli\s+zhihu\s+answer\s/i);
  for (const field of ['作者', '赞同', '发布时间', '更新时间', '完整回答']) {
    assert.match(social, new RegExp(field));
  }
  assert.match(social, /问题页[^。\n]*(?:不能|不足|不可)[^。\n]*(?:代替|替代)[^。\n]*回答/);
});

test('all maintained references are discoverable from SKILL.md', () => {
  for (const file of [
    'sources.md',
    'query-cheatsheet.md',
    'social-platforms.md',
    'story-research.md',
  ]) {
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
