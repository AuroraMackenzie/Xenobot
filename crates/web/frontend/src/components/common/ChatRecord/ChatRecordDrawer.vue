<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { ref, watch, toRaw, nextTick, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import FilterPanel from "./FilterPanel.vue";
import MessageList from "./MessageList.vue";
import SessionTimeline from "./SessionTimeline.vue";
import type { ChatRecordQuery } from "./types";
import { useLayoutStore } from "@/stores/layout";
import { useSessionStore } from "@/stores/session";
import { storeToRefs } from "pinia";

const { t } = useI18n();
const layoutStore = useLayoutStore();
const sessionStore = useSessionStore();
const { currentSessionId } = storeToRefs(sessionStore);

// English engineering note.
const isWindows = ref(false);

onMounted(() => {
  isWindows.value = navigator.platform.toLowerCase().includes("win");
});

// English engineering note.
const messageListRef = ref<InstanceType<typeof MessageList> | null>(null);

// English engineering note.
const localQuery = ref<ChatRecordQuery>({});

// English engineering note.
const messageCount = ref(0);

// English engineering note.
const timelineCollapsed = ref(false);

// English engineering note.
const activeSessionId = ref<number | undefined>(undefined);

// English engineering note.
const sessionsCache = ref<
  Array<{ id: number; startTs: number; endTs: number; firstMessageId: number }>
>([]);

// English engineering note.
const matchedSessionIds = ref<Set<number> | undefined>(undefined);

// English engineering note.
function handleApplyFilter(query: ChatRecordQuery) {
  localQuery.value = query;
}

// English engineering note.
function handleResetFilter() {
  localQuery.value = {};
  matchedSessionIds.value = undefined;
}

// English engineering note.
function handleCountChange(count: number) {
  messageCount.value = count;
}

// English engineering note.
function handleMessageTimestampsChange(timestamps: number[]) {
  // English engineering note.
  if (!localQuery.value.keywords?.length || !sessionsCache.value.length) {
    matchedSessionIds.value = undefined;
    return;
  }

  // English engineering note.
  const sessionIds = new Set<number>();
  for (const ts of timestamps) {
    for (const session of sessionsCache.value) {
      if (ts >= session.startTs && ts <= session.endTs) {
        sessionIds.add(session.id);
        break;
      }
    }
  }

  matchedSessionIds.value = sessionIds.size > 0 ? sessionIds : undefined;
}

// English engineering note.
function handleVisibleMessageChange(payload: {
  id: number;
  timestamp: number;
}) {
  if (!sessionsCache.value.length) return;

  // English engineering note.
  let targetSession: { id: number } | undefined;
  for (const session of sessionsCache.value) {
    if (
      payload.timestamp >= session.startTs &&
      payload.timestamp <= session.endTs
    ) {
      targetSession = session;
      break;
    }
  }

  // English engineering note.
  if (!targetSession) {
    for (const session of sessionsCache.value) {
      if (session.firstMessageId <= payload.id) {
        targetSession = session;
      } else {
        break;
      }
    }
  }

  if (targetSession && targetSession.id !== activeSessionId.value) {
    activeSessionId.value = targetSession.id;
  }
}

// English engineering note.
function handleSessionSelect(_sessionId: number, firstMessageId: number) {
  activeSessionId.value = _sessionId;

  // English engineering note.
  // English engineering note.
  // English engineering note.
  localQuery.value = {
    ...localQuery.value,
    scrollToMessageId: firstMessageId,
  };
}

// English engineering note.
function handleJumpToMessage(messageId: number) {
  // English engineering note.
  localQuery.value = {
    scrollToMessageId: messageId,
  };
}

// English engineering note.
async function loadSessionsCache() {
  if (!currentSessionId.value) return;

  try {
    const sessions = await window.sessionApi.getSessions(
      currentSessionId.value,
    );
    sessionsCache.value = sessions.map((s) => ({
      id: s.id,
      startTs: s.startTs,
      endTs: s.endTs,
      firstMessageId: s.firstMessageId,
    }));
  } catch {
    sessionsCache.value = [];
  }
}

// English engineering note.
watch(
  () => layoutStore.showChatRecordDrawer,
  async (isOpen) => {
    if (isOpen) {
      // English engineering note.
      const query = toRaw(layoutStore.chatRecordQuery);
      localQuery.value = query ? { ...query } : {};
      // English engineering note.
      await loadSessionsCache();
      // English engineering note.
      if (sessionsCache.value.length > 0) {
        activeSessionId.value =
          sessionsCache.value[sessionsCache.value.length - 1].id;
      }
      // English engineering note.
      await nextTick();
      messageListRef.value?.refresh();
    } else {
      // English engineering note.
      localQuery.value = {};
      messageCount.value = 0;
      activeSessionId.value = undefined;
      sessionsCache.value = [];
    }
  },
);
</script>

<template>
  <UDrawer
    v-model:open="layoutStore.showChatRecordDrawer"
    direction="right"
    :handle="false"
    :ui="{ content: 'z-50' }"
  >
    <template #content>
      <div
        class="xeno-record-drawer flex h-full w-[760px] max-w-[100vw] flex-col"
        style="-webkit-app-region: no-drag"
      >
        <!-- English UI note -->
        <div
          class="xeno-record-drawer-header flex items-center justify-between px-4"
          :class="isWindows ? 'pt-10 pb-3' : 'py-3'"
        >
          <h3
            class="break-words pr-3 text-lg font-semibold text-gray-900 dark:text-white"
          >
            {{ t("records.drawer.title") }}
          </h3>
          <UButton
            icon="i-heroicons-x-mark"
            color="neutral"
            variant="ghost"
            size="sm"
            @click="layoutStore.closeChatRecordDrawer()"
          />
        </div>

        <!-- English UI note -->
        <FilterPanel
          :query="localQuery"
          @apply="handleApplyFilter"
          @reset="handleResetFilter"
        />

        <!-- English UI note -->
        <div class="flex min-h-0 flex-1">
          <!-- English UI note -->
          <SessionTimeline
            v-if="currentSessionId"
            v-model:collapsed="timelineCollapsed"
            :session-id="currentSessionId"
            :active-session-id="activeSessionId"
            :filter-start-ts="localQuery.startTs"
            :filter-end-ts="localQuery.endTs"
            :filter-matched-session-ids="matchedSessionIds"
            @select="handleSessionSelect"
          />

          <!-- English UI note -->
          <div class="min-h-0 min-w-0 flex-1">
            <MessageList
              ref="messageListRef"
              :query="localQuery"
              @count-change="handleCountChange"
              @visible-message-change="handleVisibleMessageChange"
              @jump-to-message="handleJumpToMessage"
              @message-timestamps-change="handleMessageTimestampsChange"
            />
          </div>
        </div>

        <!-- English UI note -->
        <div
          v-if="messageCount > 0"
          class="xeno-record-drawer-footer shrink-0 px-4 py-2"
        >
          <span class="text-xs text-gray-500">{{
            t("records.drawer.loadedCount", { count: messageCount })
          }}</span>
        </div>
      </div>
    </template>
  </UDrawer>
</template>

<style scoped>
.xeno-record-drawer {
  background:
    radial-gradient(
      circle at top left,
      rgba(84, 214, 255, 0.1),
      transparent 24%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 22%),
    rgba(7, 18, 29, 0.97);
  border-left: 1px solid rgba(139, 166, 189, 0.14);
  box-shadow: -20px 0 56px rgba(2, 8, 16, 0.28);
  backdrop-filter: blur(22px) saturate(132%);
}

.xeno-record-drawer-header {
  border-bottom: 1px solid rgba(139, 166, 189, 0.14);
}

.xeno-record-drawer-footer {
  border-top: 1px solid rgba(139, 166, 189, 0.14);
}
</style>
