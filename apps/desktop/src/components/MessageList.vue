<script setup lang="ts">
import { Bot, UserRound } from "@lucide/vue";

import MarkdownMessage from "@/components/MarkdownMessage.vue";
import type { Message } from "@/types/chat";

defineProps<{ messages: Message[]; streaming: boolean }>();
</script>

<template>
  <div class="message-list" aria-live="polite">
    <article v-for="message in messages" :key="message.id" class="chat-message" :class="`is-${message.role}`">
      <div class="message-avatar" aria-hidden="true">
        <UserRound v-if="message.role === 'user'" :size="18" />
        <Bot v-else :size="18" />
      </div>
      <div class="message-body">
        <span class="message-author">{{ message.role === "user" ? "旅行者" : "派蒙" }}</span>
        <MarkdownMessage v-if="message.content" :content="message.content" />
        <span v-else-if="streaming" class="typing-indicator" aria-label="正在生成">
          <i></i><i></i><i></i>
        </span>
      </div>
    </article>
  </div>
</template>
