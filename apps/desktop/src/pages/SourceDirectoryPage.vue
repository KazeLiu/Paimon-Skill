<script setup lang="ts">
import { onMounted, reactive } from "vue";
import { Check, CircleAlert, ExternalLink, Globe2, Power, RefreshCw } from "@lucide/vue";
import { checkSourceConnection, checkSourceLogin, openBrowserTab } from "@/api/browser";
import { useBrowserStore } from "@/stores/browserStore";

type SourceConnection = "unknown" | "checking" | "connected" | "failed";
type SourceLogin = "unknown" | "logged-in" | "logged-out";

interface FixedSource {
  id: string;
  name: string;
  description: string;
  category: string;
  authority: string;
  purpose: string;
  url: string;
  iconUrl: string;
  requiresLogin?: boolean;
}

interface SourceState {
  enabled: boolean;
  connection: SourceConnection;
  login: SourceLogin;
  message: string;
}

const sources: FixedSource[] = [
  {
    id: "obc",
    name: "米游社观测枢（OBC）",
    description: "米哈游原神第一方百科",
    category: "第一方资料",
    authority: "第一方资料",
    purpose: "基础档案、材料、数值与官方说明",
    url: "https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header",
    iconUrl: "https://baike.mihoyo.com/ys/obc/favicon.ico",
  },
  {
    id: "bwiki",
    name: "B站原神 WIKI（BWiki）",
    description: "社区维护的原神 Wiki",
    category: "社区维护 Wiki",
    authority: "社区维护 Wiki 数据",
    purpose: "图鉴、机制、角色攻略与筛选工具；剧情事实和数值优先与 OBC 交叉核对",
    url: "https://wiki.biligame.com/ys/",
    iconUrl: "https://static.hdslb.com/images/favicon.ico",
  },
  {
    id: "genshin-wiki-en",
    name: "Genshin Impact Wiki（英文）",
    description: "英文多语言考据资料",
    category: "多语言考据资料",
    authority: "多语言社区维护 Wiki",
    purpose: "Etymology、Historical References、Other Languages 与文化原型线索",
    url: "https://genshin-impact.fandom.com/wiki/Genshin_Impact_Wiki",
    iconUrl: "https://genshin-impact.fandom.com/favicon.ico",
  },
  {
    id: "genshin-wiki-ru",
    name: "Genshin Impact Wiki（俄语）",
    description: "俄语多语言考据资料",
    category: "多语言考据资料",
    authority: "多语言社区维护 Wiki",
    purpose: "俄语词形、Etymology、Historical References 与斯拉夫文化相关条目",
    url: "https://genshin-impact.fandom.com/ru/wiki/Genshin_Impact_%D0%92%D0%B8%D0%BA%D0%B8",
    iconUrl: "https://genshin-impact.fandom.com/favicon.ico",
  },
  {
    id: "project-amber-en",
    name: "Project Amber（英文）",
    description: "英文第三方游戏文本数据库",
    category: "多语言考据资料",
    authority: "第三方游戏文本数据库",
    purpose: "对照任务、书籍、物品和角色的英文本地化文本",
    url: "https://ambr.top/en/archive",
    iconUrl: "https://ambr.top/favicon.ico",
  },
  {
    id: "project-amber-ru",
    name: "Project Amber（俄语）",
    description: "俄语第三方游戏文本数据库",
    category: "多语言考据资料",
    authority: "第三方游戏文本数据库",
    purpose: "对照任务、书籍、物品和角色的俄文本地化文本",
    url: "https://ambr.top/ru/archive",
    iconUrl: "https://ambr.top/favicon.ico",
  },
  {
    id: "xiaohongshu",
    name: "小红书",
    description: "玩家经验与实战讨论",
    category: "玩家经验",
    authority: "社区经验",
    purpose: "充能手感、低成本平替、新手误区与深渊实战；不能替代事实来源",
    url: "https://www.xiaohongshu.com/explore",
    iconUrl: "https://www.xiaohongshu.com/favicon.ico",
    requiresLogin: true,
  },
  {
    id: "zhihu",
    name: "知乎",
    description: "中文社区分析",
    category: "社区分析",
    authority: "中文社区分析",
    purpose: "机制推导、资料整理、剧情与文化考据线索；引用前必须读取具体回答",
    url: "https://www.zhihu.com/",
    iconUrl: "https://www.zhihu.com/favicon.ico",
    requiresLogin: true,
  },
  {
    id: "reddit-genshin-lore",
    name: "Reddit / r/Genshin_Lore",
    description: "英语社区考据讨论",
    category: "社区分析",
    authority: "英语社区分析",
    purpose: "词源、神话、文学和历史原型讨论线索；仅作分析参考，不是官方设定",
    url: "https://www.reddit.com/r/Genshin_Lore/",
    iconUrl: "https://www.reddit.com/favicon.ico",
  },
  {
    id: "bilibili-video",
    name: "B站视频",
    description: "玩家攻略视频与字幕",
    category: "玩家经验",
    authority: "玩家经验（社区经验）",
    purpose: "视频攻略参考、解谜解法与实战内容；使用前核对视频详情、字幕和适用版本",
    url: "https://www.bilibili.com/",
    iconUrl: "https://www.bilibili.com/favicon.ico",
  },
];

const recommendSourceUrl = `https://github.com/KazeLiu/Paimon-Skill/issues/new?title=${encodeURIComponent("推荐信息源")}&body=${encodeURIComponent("请按下面的格式填写新的信息源适配建议：\n\n- 信息源名称：\n- 网站 URL：\n- 是否需要登录：\n- 希望读取的内容：\n- 是否需要评论或点击操作：\n- 其他说明：")}`;

const store = useBrowserStore();
const sourceStates = reactive<Record<string, SourceState>>({});
const sourceTabs = reactive<Record<string, string>>({});

function enabledKey(source: FixedSource) {
  return `paimon.source.enabled.${source.id}`;
}

function initialState(source: FixedSource): SourceState {
  return {
    enabled: localStorage.getItem(enabledKey(source)) !== "false",
    connection: "unknown",
    login: "unknown",
    message: "尚未检测",
  };
}

onMounted(async () => {
  for (const source of sources) sourceStates[source.id] = initialState(source);
  await Promise.all([store.initialize(), store.bindEvents().catch(() => undefined)]);
  for (const source of sources) await checkConnection(source);
});

function toggleSource(source: FixedSource) {
  const state = sourceStates[source.id];
  state.enabled = !state.enabled;
  localStorage.setItem(enabledKey(source), String(state.enabled));
}

function connectionText(connection: SourceConnection) {
  return { unknown: "未检测", checking: "检测中", connected: "可连接", failed: "连接失败" }[connection];
}

function loginText(login: SourceLogin) {
  return { unknown: "未检测", "logged-in": "已登录", "logged-out": "未登录" }[login];
}

function matchingTab(source: FixedSource) {
  const hostname = new URL(source.url).hostname;
  return store.tabs.find((tab) => {
    try {
      return new URL(tab.url).hostname === hostname;
    } catch {
      return false;
    }
  });
}

async function ensureSourceTab(source: FixedSource, focus = true) {
  const existingId = sourceTabs[source.id];
  if (existingId && !store.tabs.some((tab) => tab.id === existingId)) {
    delete sourceTabs[source.id];
  }
  const currentId = sourceTabs[source.id];
  const existing = (currentId && store.tabs.find((tab) => tab.id === currentId)) || matchingTab(source);
  if (existing) {
    sourceTabs[source.id] = existing.id;
    if (focus) await store.select(existing.id);
    return existing;
  }
  const tab = await store.open(source.url);
  sourceTabs[source.id] = tab.id;
  return tab;
}

async function checkConnection(source: FixedSource) {
  const state = sourceStates[source.id];
  state.connection = "checking";
  try {
    const result = await checkSourceConnection(source.url);
    state.connection = result.reachable ? "connected" : "failed";
    state.message = result.message;
  } catch (error) {
    state.connection = "failed";
    state.message = error instanceof Error ? error.message : String(error);
  }
}

async function refreshLogin(source: FixedSource): Promise<SourceLogin> {
  const state = sourceStates[source.id];
  state.login = "unknown";
  try {
    const result = await checkSourceLogin(source.id);
    state.login = result.status;
    state.message = result.message;
    return result.status;
  } catch (error) {
    state.login = "unknown";
    state.message = error instanceof Error ? error.message : String(error);
    return "unknown";
  }
}

async function openLogin(source: FixedSource) {
  await ensureSourceTab(source, true);
}

async function handleLogin(source: FixedSource) {
  const login = await refreshLogin(source);
  if (login !== "logged-in") await openLogin(source);
}

function openRecommendation(event: MouseEvent) {
  event.preventDefault();
  void openBrowserTab(recommendSourceUrl);
}

function handleIconError(event: Event) {
  (event.currentTarget as HTMLImageElement).hidden = true;
}
</script>

<template>
  <section class="source-directory-page">
    <header class="settings-page-header source-directory-header">
      <div>
        <span class="eyebrow">固定适配</span>
        <h2>信息源</h2>
        <p>登录状态保存在应用独立的 WebView2 Profile 中。</p>
      </div>
    </header>

    <div class="source-grid">
      <article v-for="source in sources" :key="source.id" class="source-card" :data-role="'source-card'" :data-source-id="source.id">
        <div class="source-card-heading">
          <div class="source-icon" data-role="source-icon" aria-hidden="true">
            <Globe2 :size="19" />
            <img :src="source.iconUrl" alt="" loading="lazy" @error="handleIconError" />
          </div>
          <div>
            <h3>{{ source.name }}</h3>
            <p>{{ source.description }}</p>
          </div>
        </div>
        <div class="source-information">
          <span class="source-category">分类：{{ source.category }}</span>
          <span class="source-authority">权威层级：{{ source.authority }}</span>
        </div>
        <p class="source-purpose"><strong>用途：</strong>{{ source.purpose }}</p>
        <div class="source-status-list">
          <button
            type="button"
            class="source-status"
            :class="`is-${sourceStates[source.id]?.connection || 'unknown'}`"
            data-status="connection"
            :title="`重新检测 ${source.name} 的连接`"
            @click="checkConnection(source)"
          >
            <Check v-if="sourceStates[source.id]?.connection === 'connected'" :size="14" />
            <CircleAlert v-else-if="sourceStates[source.id]?.connection === 'failed'" :size="14" />
            <i v-else class="status-dot" aria-hidden="true"></i>
            <RefreshCw v-if="sourceStates[source.id]?.connection === 'checking'" :size="13" />
            连接：{{ connectionText(sourceStates[source.id]?.connection || 'unknown') }}
          </button>
          <button
            v-if="source.requiresLogin"
            type="button"
            class="source-status"
            :class="`is-${sourceStates[source.id]?.login || 'unknown'}`"
            data-status="login"
            :title="`读取 ${source.name} 登录 Cookie；未登录时打开登录页面`"
            @click="handleLogin(source)"
          >
            <Check v-if="sourceStates[source.id]?.login === 'logged-in'" :size="14" />
            <CircleAlert v-else-if="sourceStates[source.id]?.login === 'logged-out'" :size="14" />
            <i v-else class="status-dot" aria-hidden="true"></i>
            登录：{{ loginText(sourceStates[source.id]?.login || 'unknown') }}
          </button>
        </div>
        <small class="source-message">{{ sourceStates[source.id]?.message || "尚未检测" }}</small>
        <div class="source-card-actions">
          <button
            class="source-toggle"
            type="button"
            data-action="toggle-source"
            :aria-pressed="sourceStates[source.id]?.enabled ?? true"
            @click="toggleSource(source)"
          >
            <Power :size="15" /> {{ sourceStates[source.id]?.enabled ? "已启用" : "已停用" }}
          </button>
        </div>
      </article>

      <a
        class="source-recommendation"
        data-action="recommend-source"
        :href="recommendSourceUrl"
        target="_blank"
        rel="noopener noreferrer"
        @click="openRecommendation"
      >
        <span class="source-recommendation-icon" aria-hidden="true"><ExternalLink :size="18" /></span>
        <span>
          <strong>推荐一个信息源</strong>
          <small>进入项目 Issue，按模板填写网站地址、登录要求和希望读取的内容。</small>
        </span>
        <ExternalLink :size="16" aria-hidden="true" />
      </a>
    </div>

  </section>
</template>
