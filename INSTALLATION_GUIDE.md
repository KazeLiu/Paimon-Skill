# Paimon-Skill 全局链接与依赖指南

> **多数情况用不到本文。** 在 Paimon-Skill 目录里打开 Agent，直接问原神问题即可——根目录 [AGENTS.md](AGENTS.md) 已内联全部规则，进目录即生效。
>
> 本文只在两种情况下需要：(1) 用户想在**任意目录**唤起派蒙，需把 skill 链接到全局 skills 目录；(2) 需要查小红书/知乎/B站社区内容，需安装可选依赖 OpenCLI。链接 skill 与安装依赖是两个独立步骤，都要先征得用户同意。

## 1. 检查环境

```bash
git --version
node --version
npm --version
```

Node.js 18+ 仅用于依赖检测与项目测试。缺少 Node.js 时，仍可安装纯文档 skill，但无法运行检测脚本。

## 2. 获取仓库

若 Agent 已在本仓库目录内运行，跳过本步。否则克隆到用户指定的新目录：

```bash
git clone https://github.com/KazeLiu/Paimon-Skill.git
cd Paimon-Skill
```

目标目录已存在时不要覆盖或直接拉取。先确认它是 Git 仓库、远程地址属于本项目且工作树没有未提交改动；任一条件不满足就停止并询问用户。

## 3. 检测可选依赖

```bash
node skills/genshin-build-wiki/check-dependencies.js --json
```

检测器不安装软件、不写状态文件。若 OpenCLI 缺失，告诉用户社区查询不可用，但 BWiki/OBC 仍可使用，并询问是否安装。若 OpenCLI 存在但所需平台能力为 `false`，说明缺少的平台子命令并询问是否更新或修复。

只有用户明确同意安装、更新或修复后才执行：

```bash
npm install -g @jackwener/opencli
```

OpenCLI 还需要 Chrome 浏览器扩展（Browser Bridge）复用登录态才能真正查询——**完整两步安装（含扩展的应用商店 / 离线两种装法）见 [skills/genshin-build-wiki/INSTALL.md](skills/genshin-build-wiki/INSTALL.md)**。安装完成后重新运行检测。用户拒绝时跳过，不把它当作 skill 安装失败。

## 4. 链接到 Agent skills 目录

根据实际 Agent 选择目标目录：

```text
Claude Code: ~/.claude/skills/genshin-build-wiki
Codex:       ~/.codex/skills/genshin-build-wiki
通用 Agent:  ~/.agents/skills/genshin-build-wiki
```

### Windows

以下示例使用 Claude Code 目标；其他 Agent 只替换 `$target`。它只会移除指向同一源码的现有链接，绝不会递归删除普通目录。

```powershell
$source = [IO.Path]::GetFullPath("D:\code\Paimon-Skill\skills\genshin-build-wiki")
$target = [IO.Path]::GetFullPath("$env:USERPROFILE\.claude\skills\genshin-build-wiki")
$parent = Split-Path -Parent $target
New-Item -ItemType Directory -Force -Path $parent | Out-Null

if (Test-Path -LiteralPath $target) {
    $item = Get-Item -LiteralPath $target -Force
    if ($item.LinkType -notin @('Junction', 'SymbolicLink')) {
        throw "目标已存在且不是链接，停止安装：$target"
    }

    $linkedTarget = [IO.Path]::GetFullPath([string]@($item.Target)[0])
    if ($linkedTarget -ne $source) {
        throw "目标链接指向其他位置，停止安装：$linkedTarget"
    }

    Remove-Item -LiteralPath $target -Force
}

New-Item -ItemType Junction -Path $target -Target $source | Out-Null
```

### macOS/Linux

```bash
source_path="$(pwd)/skills/genshin-build-wiki"
target_path="$HOME/.claude/skills/genshin-build-wiki"
mkdir -p "$(dirname "$target_path")"

if [ -e "$target_path" ] || [ -L "$target_path" ]; then
  if [ ! -L "$target_path" ]; then
    echo "目标已存在且不是符号链接，停止安装：$target_path" >&2
    exit 1
  fi
  if [ "$(readlink "$target_path")" != "$source_path" ]; then
    echo "目标链接指向其他位置，停止安装" >&2
    exit 1
  fi
  rm "$target_path"
fi

ln -s "$source_path" "$target_path"
```

## 5. 验证

确认目标是链接且 `SKILL.md` 可读，然后运行：

```bash
node skills/genshin-build-wiki/check-dependencies.js --json
npm test
```

`npm test` 适用于仓库开发安装；仅链接单个 skill 时，只需检查 `SKILL.md` 和依赖检测输出。

## 6. 完成反馈

向用户说明：

- skill 链接到哪个 Agent 目录
- OpenCLI 是否可用；若未安装，社区查询会被跳过
- 检测器没有写入机器状态文件
- 可用测试问题，如“胡桃怎么配”或“当前卡池是什么”

## 更新

更新前检查远程和工作树，存在本地改动时停止并交给用户决定：

```bash
git remote get-url origin
git status --short
git pull --ff-only
node skills/genshin-build-wiki/check-dependencies.js --json
npm test
```

## 卸载

只移除确认属于本项目的 junction/symlink。普通目录、指向其他位置的链接和仓库源码都不得由安装流程自动删除。OpenCLI 也只在用户明确要求时单独卸载。
