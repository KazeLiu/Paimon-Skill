# Genshin Build Wiki Skill - 安装指南

本 skill 需要一些外部依赖才能正常工作。AI 会在首次使用时自动检测并安装这些依赖。

## 自动安装依赖

当 AI 首次加载本 skill 时，会自动执行以下检查与安装：

### 1. OpenCLI（小红书/知乎/B站查询）

```bash
# 检测
opencli --version

# 未安装时自动执行
npm install -g @panniantong/opencli
```

**用途**：
- 查询小红书和知乎的社区讨论
- 查询B站视频（OpenCLI 自带 `bilibili` 子命令）

**功能**：`xiaohongshu`, `zhihu`, `bilibili` 等 60+ 平台

### 2. 网页阅读工具（可选）

- **Jina Reader**：无需安装，公开免费接口
- **web-reader MCP**：可选，AI 会在首次失败时询问是否安装

---

## 手动安装（可选）

如果你希望提前安装这些依赖，可以手动执行：

```bash
# 安装 OpenCLI（包含小红书/知乎/B站支持）
npm install -g @panniantong/opencli

# 验证安装
opencli --version
```

---

## 依赖检测流程

AI 会按以下流程自动处理依赖：

```
加载 genshin-build-wiki skill
    ↓
检测 opencli --version
    ├─ 成功 → 标记为可用（小红书/知乎/B站全部可用）
    └─ 失败 → 自动安装 npm install -g @panniantong/opencli
              ↓
              安装后再次验证
              ├─ 成功 → 标记为可用
              └─ 失败 → 记录错误，降级到仅官方数据模式
    ↓
开始正常使用 skill
```

---

## 降级模式

即使依赖安装失败，skill 仍可正常工作（功能受限）：

| 依赖 | 失败后影响 | 降级方案 |
|------|-----------|---------|
| OpenCLI | 无法查询小红书/知乎/B站 | 仅使用官方数据（BWiki + 观测枢） |
| web-reader MCP | OBC 解析可能失败 | 使用 Jina Reader 兜底 |

---

## 常见问题

### Q: 为什么需要这些依赖？

A: 本 skill 综合官方 Wiki 和社区讨论，OpenCLI 用于访问社交平台获取玩家实战反馈。

### Q: 不想安装这些依赖可以吗？

A: 可以！skill 会降级到仅使用官方数据模式，只查询 BWiki 和观测枢。

### Q: npm install 失败怎么办？

A: 确认已安装 Node.js（>=16）和 npm。或者使用降级模式，仅用官方数据。

### Q: OpenCLI 需要登录吗？

A: 小红书和知乎的搜索功能需要登录态。AI 会引导你使用 Cookie-Editor 导出 Cookie 进行配置。

### Q: OpenCLI 能查 B站吗？

A: 能！OpenCLI 自带 `bilibili` 子命令，支持搜索视频、获取字幕等功能，无需单独安装 bili-cli。

---

## 环境要求

- **Node.js**: >= 16.0.0
- **npm**: >= 7.0.0
- **网络**: 需要访问外网（BWiki、观测枢、小红书、知乎、B站）

---

## 卸载

如果不再需要这些依赖：

```bash
npm uninstall -g @panniantong/opencli
```

Skill 会自动降级到仅官方数据模式。
