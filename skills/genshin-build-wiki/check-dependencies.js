#!/usr/bin/env node

const { spawnSync } = require('node:child_process');

function checkCommand(command, args) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    shell: process.platform === 'win32',
    timeout: 10_000,
    windowsHide: true,
  });

  const version = `${result.stdout || ''}${result.stderr || ''}`.trim();
  if (result.status === 0) {
    return { installed: true, version: version || 'available' };
  }

  const notFound =
    result.error?.code === 'ENOENT' ||
    result.status === 127 ||
    /not recognized as an internal or external command|command not found|不是内部或外部命令/i.test(version);

  return {
    installed: false,
    reason: notFound ? 'not_found' : 'unavailable',
  };
}

function createReport() {
  const opencli = checkCommand('opencli', ['--version']);
  const capabilities = {
    xiaohongshu: opencli.installed && checkCommand('opencli', ['xiaohongshu', '--help']).installed,
    zhihu: opencli.installed && checkCommand('opencli', ['zhihu', '--help']).installed,
    bilibili: opencli.installed && checkCommand('opencli', ['bilibili', '--help']).installed,
  };

  return {
    checkedAt: new Date().toISOString(),
    dependencies: {
      opencli: {
        ...opencli,
        capabilities,
      },
    },
    features: {
      wikiLookup: true,
      xiaohongshuLookup: capabilities.xiaohongshu,
      zhihuLookup: capabilities.zhihu,
      bilibiliLookup: capabilities.bilibili,
      communityLookup: Object.values(capabilities).some(Boolean),
    },
  };
}

function printHumanReadable(report) {
  console.log('Genshin Build Wiki dependency check');
  console.log('-----------------------------------');
  console.log('Wiki lookup: available (HTTP/web reader)');

  if (report.dependencies.opencli.installed) {
    console.log(`OpenCLI: available (${report.dependencies.opencli.version})`);
    console.log(`Xiaohongshu lookup: ${report.features.xiaohongshuLookup ? 'available' : 'unavailable'}`);
    console.log(`Zhihu lookup: ${report.features.zhihuLookup ? 'available' : 'unavailable'}`);
    console.log(`Bilibili lookup: ${report.features.bilibiliLookup ? 'available' : 'unavailable'}`);
    if (!report.features.xiaohongshuLookup || !report.features.zhihuLookup || !report.features.bilibiliLookup) {
      console.log('Ask the user before installing or updating OpenCLI to repair missing platform capabilities.');
    }
    return;
  }

  const state = report.dependencies.opencli.reason === 'not_found' ? 'not found' : 'unavailable or broken';
  console.log(`OpenCLI: ${state}`);
  console.log('Community lookup: unavailable until the user approves installation');
  console.log('This detector never installs software or writes status files.');
}

const report = createReport();
if (process.argv.includes('--json')) {
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
} else {
  printHumanReadable(report);
}
