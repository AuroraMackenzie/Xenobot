<script setup lang="ts">
/**
 * Chat record message row with sender identity, reply context, and keyword highlights.
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { ChatRecordMessage } from "./types";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();

const props = defineProps<{
  /** Normalized chat message payload. */
  message: ChatRecordMessage;
  /** True when the row is the current scroll/jump target. */
  isTarget?: boolean;
  /** Keywords or semantic terms highlighted in the bubble body. */
  highlightKeywords?: string[];
  /** Shows context jump affordance in filtered mode. */
  isFiltered?: boolean;
}>();

defineEmits<{
  (e: "view-context", messageId: number): void;
}>();

const sessionStore = useSessionStore();

// English engineering note.
const isOwner = computed(() => {
  const ownerId = sessionStore.currentSession?.ownerId;
  if (!ownerId) return false;
  return props.message.senderPlatformId === ownerId;
});

// English engineering note.
const colorIndex = computed(() => {
  const name = props.message.senderName || "";
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return Math.abs(hash) % 16;
});

// Build a stable palette index from the sender label.
const colorPalette = [
  {
    avatar: "bg-rose-400 dark:bg-rose-500",
    name: "text-rose-600 dark:text-rose-400",
  },
  {
    avatar: "bg-pink-400 dark:bg-pink-500",
    name: "text-pink-600 dark:text-pink-400",
  },
  {
    avatar: "bg-fuchsia-400 dark:bg-fuchsia-500",
    name: "text-fuchsia-600 dark:text-fuchsia-400",
  },
  {
    avatar: "bg-purple-400 dark:bg-purple-500",
    name: "text-purple-600 dark:text-purple-400",
  },
  {
    avatar: "bg-violet-400 dark:bg-violet-500",
    name: "text-violet-600 dark:text-violet-400",
  },
  {
    avatar: "bg-indigo-400 dark:bg-indigo-500",
    name: "text-indigo-600 dark:text-indigo-400",
  },
  {
    avatar: "bg-blue-400 dark:bg-blue-500",
    name: "text-blue-600 dark:text-blue-400",
  },
  {
    avatar: "bg-sky-400 dark:bg-sky-500",
    name: "text-sky-600 dark:text-sky-400",
  },
  {
    avatar: "bg-cyan-400 dark:bg-cyan-500",
    name: "text-cyan-600 dark:text-cyan-400",
  },
  {
    avatar: "bg-teal-400 dark:bg-teal-500",
    name: "text-teal-600 dark:text-teal-400",
  },
  {
    avatar: "bg-emerald-400 dark:bg-emerald-500",
    name: "text-emerald-600 dark:text-emerald-400",
  },
  {
    avatar: "bg-green-400 dark:bg-green-500",
    name: "text-green-600 dark:text-green-400",
  },
  {
    avatar: "bg-lime-500 dark:bg-lime-600",
    name: "text-lime-600 dark:text-lime-400",
  },
  {
    avatar: "bg-amber-400 dark:bg-amber-500",
    name: "text-amber-600 dark:text-amber-400",
  },
  {
    avatar: "bg-orange-400 dark:bg-orange-500",
    name: "text-orange-600 dark:text-orange-400",
  },
  {
    avatar: "bg-red-400 dark:bg-red-500",
    name: "text-red-600 dark:text-red-400",
  },
];

const currentColor = computed(() => colorPalette[colorIndex.value]);
const avatarColor = computed(() => currentColor.value.avatar);
const nameColor = computed(() => currentColor.value.name);

// English engineering note.
const bubbleColor = computed(() =>
  isOwner.value
    ? "bg-green-100 dark:bg-green-900/40"
    : "bg-gray-100 dark:bg-gray-800",
);

// English engineering note.
const displayName = computed(() => {
  const name = props.message.senderName || "";
  const aliases = props.message.senderAliases || [];

  // English engineering note.
  if (aliases.length > 0) {
    return `${name}（${aliases[0]}）`;
  }
  return name;
});

// English engineering note.
const avatarLetter = computed(() => {
  const name = props.message.senderName || "";
  if (!name) return "?";

  try {
    const segmenter = new Intl.Segmenter("zh", { granularity: "grapheme" });
    const segments = [...segmenter.segment(name)];
    if (segments.length > 0) {
      return segments[0].segment;
    }
  } catch {
    // English engineering note.
    const chars = [...name];
    if (chars.length > 0) {
      const firstChar = chars[0];
      if (/^[a-zA-Z]$/.test(firstChar)) {
        return firstChar.toUpperCase();
      }
      return firstChar;
    }
  }

  return "?";
});

// English engineering note.
function highlightContent(content: string): string {
  if (!props.highlightKeywords?.length || !content) return content;

  const pattern = props.highlightKeywords
    .map((k) => k.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("|");
  const regex = new RegExp(`(${pattern})`, "gi");
  return content.replace(
    regex,
    '<mark class="bg-transparent border-b-2 border-yellow-400 dark:border-yellow-500">$1</mark>',
  );
}
</script>

<template>
  <div
    class="group px-4 py-2 transition-colors"
    :class="{
      'bg-yellow-50/50 dark:bg-yellow-900/10': isTarget,
    }"
  >
    <!-- English UI note -->
    <div class="flex gap-3" :class="isOwner ? 'flex-row-reverse' : ''">
      <!-- English UI note -->
      <div
        class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-sm font-medium text-white overflow-hidden"
        :class="message.senderAvatar ? '' : avatarColor"
      >
        <img
          v-if="message.senderAvatar"
          :src="message.senderAvatar"
          :alt="message.senderName"
          class="h-full w-full object-cover"
        />
        <span v-else>{{ avatarLetter }}</span>
      </div>

      <!-- English UI note -->
      <div
        class="min-w-0 flex-1"
        :class="isOwner ? 'flex flex-col items-end' : ''"
      >
        <!-- English UI note -->
        <div
          class="mb-1 flex items-center gap-2"
          :class="isOwner ? 'flex-row-reverse' : ''"
        >
          <span class="text-sm font-medium" :class="nameColor">
            {{ displayName }}
          </span>
        </div>

        <!-- English UI note -->
        <!-- English UI note -->
        <div
          class="flex min-w-0 items-start gap-1"
          :class="isOwner ? 'flex-row-reverse' : ''"
        >
          <div
            class="xeno-record-bubble relative inline-block w-fit max-w-[42rem] rounded-lg px-3 py-2 transition-shadow"
            :class="[
              bubbleColor,
              isTarget ? 'ring-2 ring-yellow-400 dark:ring-yellow-500' : '',
            ]"
          >
            <!-- English UI note -->
            <div
              v-if="message.replyToMessageId"
              class="mb-2 border-l-2 border-gray-300 dark:border-gray-600 pl-2 text-xs text-gray-500 dark:text-gray-400"
            >
              <span class="font-medium">{{
                t("records.messageItem.replyTo")
              }}</span>
              <span
                v-if="message.replyToSenderName"
                class="ml-1 text-gray-600 dark:text-gray-300"
              >
                {{ message.replyToSenderName }}
              </span>
              <p
                v-if="message.replyToContent"
                class="mt-0.5 line-clamp-2 italic"
              >
                {{ message.replyToContent }}
              </p>
            </div>
            <p
              class="whitespace-pre-wrap break-words text-sm text-gray-700 dark:text-gray-200"
              v-html="highlightContent(message.content || '')"
            />
          </div>

          <!-- English UI note -->
          <button
            v-if="isFiltered"
            class="mt-1 flex h-6 w-6 items-center justify-center rounded opacity-0 transition-opacity hover:bg-gray-200 group-hover:opacity-100 dark:hover:bg-gray-700"
            :title="t('records.messageItem.viewContext')"
            @click="$emit('view-context', message.id)"
          >
            <UIcon
              name="i-heroicons-chat-bubble-left-ellipsis"
              class="h-4 w-4 text-gray-400"
            />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-record-bubble {
  border: 1px solid rgba(139, 166, 189, 0.12);
  box-shadow: 0 10px 22px rgba(2, 8, 16, 0.12);
}
</style>
