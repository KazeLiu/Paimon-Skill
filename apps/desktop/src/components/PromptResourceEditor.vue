<script setup lang="ts">
import { RotateCcw } from "@lucide/vue";

import type { PromptFragment } from "@/types/prompts";

const props = defineProps<{
  resource: PromptFragment;
  enabled: boolean;
  modelValue: string;
}>();

const emit = defineEmits<{
  "update:enabled": [value: boolean];
  "update:modelValue": [value: string];
}>();

function resetResource() {
  emit("update:modelValue", props.resource.content);
  emit("update:enabled", props.resource.enabledByDefault);
}
</script>

<template>
  <article class="prompt-resource" :data-resource-id="resource.id">
    <header class="prompt-resource__header">
      <label class="toggle-row">
        <input
          type="checkbox"
          :checked="enabled"
          @change="emit('update:enabled', ($event.target as HTMLInputElement).checked)"
        />
        <span class="toggle-control" aria-hidden="true"></span>
        <span>
          <strong>{{ resource.title }}</strong>
          <small>{{ resource.description }}</small>
        </span>
      </label>
      <button class="icon-text-button" type="button" @click="resetResource">
        <RotateCcw :size="15" />
        恢复此项
      </button>
    </header>
    <textarea
      :value="modelValue"
      :disabled="!enabled"
      :aria-label="`${resource.title}内容`"
      rows="8"
      @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
    ></textarea>
  </article>
</template>
