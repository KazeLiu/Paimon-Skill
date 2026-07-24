#!/usr/bin/env node

/**
 * Genshin Build Wiki Skill - 依赖检测与自动安装脚本
 *
 * 用途：
 * - 检测 opencli 和 bili-cli 是否已安装
 * - 未安装时自动安装
 * - 安装失败时标记为降级模式
 *
 * 使用方式：
 * - AI 在首次加载 genshin-build-wiki skill 时自动调用
 * - 或手动运行：node check-dependencies.js
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// 颜色输出
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  blue: '\x1b[34m',
  dim: '\x1b[2m',
};

function log(color, symbol, message) {
  console.log(`${color}${symbol}${colors.reset} ${message}`);
}

function execCommand(command, silent = false) {
  try {
    const output = execSync(command, {
      encoding: 'utf-8',
      stdio: silent ? 'pipe' : 'inherit'
    });
    return { success: true, output: output?.trim() };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

function checkDependency(name, command, packageName) {
  log(colors.blue, '🔍', `检测 ${name}...`);

  const check = execCommand(command, true);

  if (check.success) {
    log(colors.green, '✓', `${name} 已安装: ${check.output}`);
    return { installed: true, version: check.output };
  }

  log(colors.yellow, '⚠', `${name} 未安装，开始自动安装...`);
  log(colors.dim, '  ', `执行: npm install -g ${packageName}`);

  const install = execCommand(`npm install -g ${packageName}`);

  if (install.success) {
    const verify = execCommand(command, true);
    if (verify.success) {
      log(colors.green, '✓', `${name} 安装成功: ${verify.output}`);
      return { installed: true, version: verify.output, justInstalled: true };
    }
  }

  log(colors.red, '✗', `${name} 安装失败，将使用降级模式`);
  return { installed: false };
}

async function main() {
  console.log('\n' + colors.blue + '═'.repeat(60) + colors.reset);
  log(colors.blue, '🎮', 'Genshin Build Wiki Skill - 依赖检测');
  console.log(colors.blue + '═'.repeat(60) + colors.reset + '\n');

  const results = {
    opencli: checkDependency(
      'OpenCLI',
      'opencli --version',
      '@panniantong/opencli'
    ),
  };

  console.log('\n' + colors.blue + '─'.repeat(60) + colors.reset);
  log(colors.blue, '📊', '依赖检测结果汇总');
  console.log(colors.blue + '─'.repeat(60) + colors.reset + '\n');

  const features = [
    {
      name: '官方 Wiki 查询 (BWiki + 观测枢)',
      status: 'available',
      color: colors.green,
      symbol: '✓',
      note: '使用 Jina Reader'
    },
    {
      name: '小红书/知乎/B站社区讨论',
      status: results.opencli.installed ? 'available' : 'unavailable',
      color: results.opencli.installed ? colors.green : colors.yellow,
      symbol: results.opencli.installed ? '✓' : '⚠',
      note: results.opencli.installed ? 'OpenCLI 可用（含 B站搜索）' : '降级模式：仅官方数据'
    },
  ];

  features.forEach(f => {
    log(f.color, f.symbol, `${f.name}: ${f.note}`);
  });

  console.log('\n' + colors.blue + '═'.repeat(60) + colors.reset + '\n');

  // 保存状态到文件（供 AI 读取）
  const statusFile = path.join(__dirname, '.dependency-status.json');
  const status = {
    timestamp: new Date().toISOString(),
    dependencies: {
      opencli: results.opencli,
    },
    features: {
      officialWiki: true,
      socialPlatforms: results.opencli.installed,
      videoRecommendation: results.opencli.installed, // OpenCLI 自带 bilibili 功能
    },
  };

  fs.writeFileSync(statusFile, JSON.stringify(status, null, 2));
  log(colors.green, '✓', `状态已保存到 ${path.basename(statusFile)}`);

  console.log('\n' + colors.dim + '提示：重新加载 skill 时会自动重新检测依赖' + colors.reset + '\n');

  // 返回状态码（0=全部成功，1=部分失败）
  if (results.opencli.installed) {
    process.exit(0);
  } else {
    process.exit(1);
  }
}

main().catch(err => {
  log(colors.red, '✗', `检测过程出错: ${err.message}`);
  process.exit(2);
});
