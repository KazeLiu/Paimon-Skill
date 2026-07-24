# Paimon-Skill 自动安装指南

本文档供 AI Agent 使用，用于自动化安装 Paimon-Skill 及其依赖。

## 安装流程

当用户说「帮我安装 Paimon-Skill：https://github.com/KazeLiu/Paimon-Skill」时，按以下步骤执行：

### 1. 检测环境

```bash
# 检测 Node.js 版本
node --version  # 要求 >= 16.0.0

# 检测 npm 版本
npm --version   # 要求 >= 7.0.0

# 检测 git（如果需要克隆）
git --version
```

如果 Node.js 或 npm 版本不符合要求，提示用户升级。

---

### 2. 获取仓库

**方案 A：克隆（推荐）**

```bash
# 克隆到用户指定位置或默认位置
git clone https://github.com/KazeLiu/Paimon-Skill.git
cd Paimon-Skill
```

**方案 B：下载 ZIP（备选）**

```bash
# 如果用户没有 git 或克隆失败
curl -L https://github.com/KazeLiu/Paimon-Skill/archive/refs/heads/main.zip -o paimon-skill.zip
unzip paimon-skill.zip
cd Paimon-Skill-main
```

---

### 3. 安装依赖

运行自动依赖检测脚本：

```bash
cd skills/genshin-build-wiki
node check-dependencies.js
```

此脚本会：
- 检测 `opencli` 是否已安装
- 未安装时自动执行 `npm install -g @panniantong/opencli`
- 保存依赖状态到 `.dependency-status.json`

**注意**：OpenCLI 自带 `bilibili` 子命令，支持小红书、知乎、B站查询，无需单独安装其他工具。

**预期输出：**

```
═══════════════════════════════════════════════════════════
🎮 Genshin Build Wiki Skill - 依赖检测
═══════════════════════════════════════════════════════════

🔍 检测 OpenCLI...
✓ OpenCLI 已安装: 1.8.6

───────────────────────────────────────────────────────────
📊 依赖检测结果汇总
───────────────────────────────────────────────────────────

✓ 官方 Wiki 查询 (BWiki + 观测枢): 使用 Jina Reader
✓ 小红书/知乎/B站社区讨论: OpenCLI 可用（含 B站搜索）

═══════════════════════════════════════════════════════════

✓ 状态已保存到 .dependency-status.json
```

**处理失败情况：**

如果依赖安装失败（返回码 1），告知用户：

```
⚠️ OpenCLI 安装失败，但不影响基础功能。

Paimon-Skill 会使用降级模式：
- ✅ 官方 Wiki 查询（BWiki + 观测枢）正常工作
- ⚠️ 社区讨论功能（小红书/知乎/B站）不可用

你可以稍后手动安装依赖：
npm install -g @panniantong/opencli
```

---

### 4. 链接到 Agent skills 目录

**检测当前 Agent 类型：**

```bash
# Claude Code
if [ -d "$HOME/.claude" ]; then
  AGENT_TYPE="claude"
  SKILLS_DIR="$HOME/.claude/skills"
fi

# Codex
if [ -d "$HOME/.codex" ]; then
  AGENT_TYPE="codex"
  SKILLS_DIR="$HOME/.codex/skills"
fi

# 通用
if [ -d "$HOME/.agents" ]; then
  AGENT_TYPE="generic"
  SKILLS_DIR="$HOME/.agents/skills"
fi
```

**创建链接（Windows）：**

```powershell
# 确保目标目录存在
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\skills"

# 创建 junction（目录联接）
$source = "D:\code\Paimon-Skill\skills\genshin-build-wiki"
$target = "$env:USERPROFILE\.claude\skills\genshin-build-wiki"

# 如果已存在，先删除
if (Test-Path $target) {
    Remove-Item $target -Recurse -Force
}

New-Item -ItemType Junction -Path $target -Target $source
```

**创建链接（macOS/Linux）：**

```bash
# 确保目标目录存在
mkdir -p ~/.claude/skills

# 创建符号链接
ln -sf "$(pwd)/skills/genshin-build-wiki" ~/.claude/skills/genshin-build-wiki
```

---

### 5. 验证安装

```bash
# 检查链接是否创建成功
ls -la ~/.claude/skills/genshin-build-wiki

# 检查 SKILL.md 是否可读
cat ~/.claude/skills/genshin-build-wiki/SKILL.md | head -5

# 检查依赖状态
cat ~/.claude/skills/genshin-build-wiki/.dependency-status.json
```

---

### 6. 完成反馈

告知用户安装成功，并提供测试建议：

```markdown
✅ Paimon-Skill 安装完成！

📦 安装内容：
- ✓ Skill 已链接到 ~/.claude/skills/genshin-build-wiki
- ✓ 依赖已安装：opencli (小红书/知乎) + bili-cli (B站)
- ✓ 自动触发已配置：提到原神/派蒙/角色名时自动加载

🎮 测试一下：
- "派蒙！"
- "胡桃怎么配？"
- "绝缘之旗印适合哪些角色？"

💡 提示：
- 派蒙会用可爱的语气回答，综合官方 Wiki 和社区讨论
- 如果你不想查社区讨论，可以说「只要官方推荐」
- 详细文档：https://github.com/KazeLiu/Paimon-Skill
```

---

## 常见问题处理

### Q: npm install -g 权限不足

**Windows：**
```powershell
# 以管理员身份运行 PowerShell
Start-Process powershell -Verb runAs
npm install -g @panniantong/opencli
```

**macOS/Linux：**
```bash
sudo npm install -g @panniantong/opencli
```

### Q: Node.js 版本过低

提示用户升级：

```
⚠️ 当前 Node.js 版本过低（需要 >= 16.0.0）

请访问 https://nodejs.org/ 下载最新 LTS 版本，或使用 nvm/nvm-windows 管理多版本。
```

### Q: 用户想卸载

```bash
# 删除链接
rm ~/.claude/skills/genshin-build-wiki  # macOS/Linux
Remove-Item "$env:USERPROFILE\.claude\skills\genshin-build-wiki"  # Windows

# 卸载依赖（可选）
npm uninstall -g @panniantong/opencli

# 删除仓库（可选）
rm -rf /path/to/Paimon-Skill
```

---

## AI 安装脚本模板

以下是完整的自动化脚本（供 AI 参考）：

```bash
#!/bin/bash
set -e

echo "🎮 开始安装 Paimon-Skill..."

# 1. 检测环境
echo "🔍 检测环境..."
node --version || { echo "❌ 需要 Node.js >= 16"; exit 1; }
npm --version || { echo "❌ 需要 npm >= 7"; exit 1; }

# 2. 克隆仓库
echo "📦 克隆仓库..."
if [ -d "Paimon-Skill" ]; then
    echo "⚠️  目录已存在，拉取最新代码..."
    cd Paimon-Skill && git pull && cd ..
else
    git clone https://github.com/KazeLiu/Paimon-Skill.git
fi

cd Paimon-Skill

# 3. 安装依赖
echo "🔧 安装依赖..."
cd skills/genshin-build-wiki
node check-dependencies.js
cd ../..

# 4. 链接到 skills 目录
echo "🔗 创建链接..."
SKILLS_DIR="$HOME/.claude/skills"
mkdir -p "$SKILLS_DIR"
ln -sf "$(pwd)/skills/genshin-build-wiki" "$SKILLS_DIR/genshin-build-wiki"

echo "✅ 安装完成！"
echo "🎮 试试问派蒙：胡桃怎么配？"
```

---

## 更新 Skill

当用户说「更新 Paimon-Skill」时：

```bash
cd /path/to/Paimon-Skill
git pull
cd skills/genshin-build-wiki
node check-dependencies.js  # 重新检测依赖
```

---

## 注意事项

1. **首次使用前必须运行依赖检测脚本**
2. **降级模式不影响基础功能**（官方 Wiki 查询始终可用）
3. **OpenCLI 某些功能需要登录态**（AI 会在使用时引导配置）
4. **仓库用 junction/symlink 而不是复制**（避免多份副本）
