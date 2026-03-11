<script setup lang="ts">
import { computed, ref, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { useVirtualizer } from "@tanstack/vue-virtual";
import MessageList from "@/components/common/ChatRecord/MessageList.vue";
import LoadingState from "@/components/UI/LoadingState.vue";
import type { ChatRecordMessage } from "@/types/format";

const { t, locale } = useI18n();

interface PaginationInfo {
  page: number;
  pageSize: number;
  totalBlocks: number;
  totalHits: number;
  hasMore: boolean;
}

const props = defineProps<{
  result: {
    blocks: Array<{
      startTs: number;
      endTs: number;
      messages: Array<{
        id: number;
        senderName: string;
        senderPlatformId: string;
        senderAliases: string[];
        senderAvatar: string | null;
        content: string;
        timestamp: number;
        type: number;
        replyToMessageId: string | null;
        replyToContent: string | null;
        replyToSenderName: string | null;
        isHit: boolean;
      }>;
      hitCount: number;
    }>;
    stats: {
      totalMessages: number;
      hitMessages: number;
      totalChars: number;
    };
    pagination?: PaginationInfo;
  } | null;
  isLoading: boolean;
  isLoadingMore?: boolean;
  estimatedTokens: number;
  tokenStatus: "green" | "yellow" | "red";
}>();

const emit = defineEmits<{
  (e: "load-more"): void;
}>();

const selectedBlockIndex = ref(0);
let isBlockSwitching = false;
const pendingScrollToMessageId = ref<number | null>(null);
const messageListRef = ref<InstanceType<typeof MessageList> | null>(null);
const blockListRef = ref<HTMLElement | null>(null);

function getBlockAtReversedIndex(index: number) {
  if (!props.result) return null;
  const originalIndex = props.result.blocks.length - 1 - index;
  return props.result.blocks[originalIndex];
}

const blockCount = computed(() => props.result?.blocks.length ?? 0);

const blockVirtualizer = useVirtualizer(
  computed(() => ({
    count: blockCount.value,
    getScrollElement: () => blockListRef.value,
    estimateSize: () => 84,
    overscan: 5,
  })),
);

const virtualBlocks = computed(() => blockVirtualizer.value.getVirtualItems());

const tokenProgressColor = computed(() => {
  switch (props.tokenStatus) {
    case "green":
      return "bg-green-500";
    case "yellow":
      return "bg-yellow-500";
    case "red":
      return "bg-red-500";
    default:
      return "bg-gray-400";
  }
});

const tokenProgressPercent = computed(() => {
  return Math.min((props.estimatedTokens / 100000) * 100, 100);
});

const currentBlockMessages = computed<ChatRecordMessage[]>(() => {
  if (blockCount.value === 0) return [];
  const block = getBlockAtReversedIndex(selectedBlockIndex.value);
  if (!block) return [];

  return block.messages.map((msg) => ({
    id: msg.id,
    senderName: msg.senderName,
    senderPlatformId: msg.senderPlatformId,
    senderAliases: msg.senderAliases,
    senderAvatar: msg.senderAvatar,
    content: msg.content,
    timestamp: msg.timestamp,
    type: msg.type,
    replyToMessageId: msg.replyToMessageId,
    replyToContent: msg.replyToContent,
    replyToSenderName: msg.replyToSenderName,
  }));
});

const hitMessageIds = computed<number[]>(() => {
  if (blockCount.value === 0) return [];
  const block = getBlockAtReversedIndex(selectedBlockIndex.value);
  if (!block) return [];

  return block.messages.filter((msg) => msg.isHit).map((msg) => msg.id);
});

const emptyQuery = { startTs: 0, endTs: 0 };

const shouldShowYear = computed(() => {
  if (!props.result || props.result.blocks.length === 0) return false;

  const blocks = props.result.blocks;
  const firstYear = new Date(blocks[0].startTs * 1000).getFullYear();
  const lastYear = new Date(
    blocks[blocks.length - 1].endTs * 1000,
  ).getFullYear();

  return firstYear !== lastYear;
});

function formatDateTime(ts: number): string {
  const options: Intl.DateTimeFormatOptions = {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  };

  if (shouldShowYear.value) {
    options.year = "numeric";
  }

  return new Date(ts * 1000).toLocaleString(locale.value, options);
}

function formatDuration(startTs: number, endTs: number): string {
  const diff = endTs - startTs;
  if (diff < 60) return t("analysis.filter.durationSeconds", { count: diff });
  if (diff < 3600)
    return t("analysis.filter.durationMinutes", {
      count: Math.floor(diff / 60),
    });
  return t("analysis.filter.durationHoursMinutes", {
    hours: Math.floor(diff / 3600),
    minutes: Math.floor((diff % 3600) / 60),
  });
}

function selectBlock(index: number) {
  selectedBlockIndex.value = index;

  const block = getBlockAtReversedIndex(index);
  if (block) {
    const firstHitMessage = block.messages.find((msg) => msg.isHit);
    if (firstHitMessage) {
      pendingScrollToMessageId.value = firstHitMessage.id;
    }
  }
}

function goToNextBlock() {
  if (isBlockSwitching) return;
  if (blockCount.value === 0) return;
  if (selectedBlockIndex.value < blockCount.value - 1) {
    isBlockSwitching = true;
    selectedBlockIndex.value++;
    scrollToBlockInList(selectedBlockIndex.value);
    setTimeout(() => {
      isBlockSwitching = false;
    }, 300);
  }
}

function goToPrevBlock() {
  if (isBlockSwitching) return;
  if (blockCount.value === 0) return;
  if (selectedBlockIndex.value > 0) {
    isBlockSwitching = true;
    selectedBlockIndex.value--;
    scrollToBlockInList(selectedBlockIndex.value);
    setTimeout(() => {
      isBlockSwitching = false;
    }, 300);
  }
}

function scrollToBlockInList(index: number) {
  blockVirtualizer.value.scrollToIndex(index, { align: "center" });
}

watch(
  () => props.result,
  () => {
    selectedBlockIndex.value = 0;
    pendingScrollToMessageId.value = null;
  },
);

watch(pendingScrollToMessageId, async (messageId) => {
  if (messageId !== null) {
    await nextTick();
    setTimeout(() => {
      messageListRef.value?.scrollToMessage(messageId);
      pendingScrollToMessageId.value = null;
    }, 100);
  }
});

function handleBlockListScroll(event: Event) {
  const target = event.target as HTMLElement;
  if (!target || !props.result?.pagination?.hasMore || props.isLoadingMore)
    return;

  const threshold = 100;
  const { scrollTop, scrollHeight, clientHeight } = target;
  if (scrollHeight - scrollTop - clientHeight < threshold) {
    emit("load-more");
  }
}
</script>

<template>
  <div class="xeno-preview-shell flex-1 flex flex-col overflow-hidden">
    <div
      v-if="result && result.blocks.length > 0"
      class="xeno-preview-header flex-none px-4 py-3"
    >
      <div class="flex items-center justify-between mb-2">
        <div class="flex flex-wrap items-center gap-4 text-sm">
          <span class="text-gray-600 dark:text-gray-400">
            {{ t("analysis.filter.stats.blocks") }}:
            <span class="font-medium text-gray-900 dark:text-white">
              {{ result.blocks.length }}
              <template
                v-if="
                  result.pagination &&
                  result.pagination.totalBlocks > result.blocks.length
                "
              >
                / {{ result.pagination.totalBlocks }}
              </template>
            </span>
          </span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ t("analysis.filter.stats.messages") }}:
            <span class="font-medium text-gray-900 dark:text-white">{{
              result.stats.totalMessages
            }}</span>
          </span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ t("analysis.filter.stats.hits") }}:
            <span class="font-medium text-primary-500">
              {{ result.pagination?.totalHits ?? result.stats.hitMessages }}
            </span>
          </span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ t("analysis.filter.stats.chars") }}:
            <span class="font-medium text-gray-900 dark:text-white">
              {{ result.stats.totalChars.toLocaleString() }}
            </span>
          </span>
        </div>
      </div>

      <!-- English UI note -->
      <div class="flex items-center gap-3">
        <span
          class="text-sm text-gray-600 dark:text-gray-400 whitespace-nowrap"
        >
          {{ t("analysis.filter.stats.tokens") }}:
          <span
            class="font-medium"
            :class="{
              'text-green-600': tokenStatus === 'green',
              'text-yellow-600': tokenStatus === 'yellow',
              'text-red-600': tokenStatus === 'red',
            }"
          >
            ~{{ estimatedTokens.toLocaleString() }}
          </span>
        </span>
        <div
          class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden"
        >
          <div
            class="h-full rounded-full transition-all duration-300"
            :class="tokenProgressColor"
            :style="{ width: `${tokenProgressPercent}%` }"
          />
        </div>
        <span class="text-xs text-gray-500 whitespace-nowrap">100K</span>
      </div>

      <div
        v-if="tokenStatus === 'yellow'"
        class="mt-2 text-xs text-yellow-600 dark:text-yellow-400"
      >
        {{ t("analysis.filter.tokenWarning.yellow") }}
      </div>
      <div
        v-if="tokenStatus === 'red'"
        class="mt-2 text-xs text-red-600 dark:text-red-400"
      >
        {{ t("analysis.filter.tokenWarning.red") }}
      </div>
    </div>

    <div class="flex-1 min-h-0 flex overflow-hidden">
      <LoadingState
        v-if="isLoading"
        variant="page"
        :text="t('analysis.filter.filtering')"
      />

      <div
        v-else-if="!result"
        class="w-full h-full flex items-center justify-center"
      >
        <div class="xeno-preview-empty text-center text-gray-400">
          <UIcon name="i-heroicons-funnel" class="w-12 h-12 mb-3 mx-auto" />
          <p>{{ t("analysis.filter.emptyHint") }}</p>
        </div>
      </div>

      <div
        v-else-if="result.blocks.length === 0"
        class="flex-1 flex items-center justify-center"
      >
        <div class="xeno-preview-empty text-center text-gray-400">
          <UIcon
            name="i-heroicons-magnifying-glass"
            class="w-12 h-12 mb-3 mx-auto"
          />
          <p>{{ t("analysis.filter.noResults") }}</p>
        </div>
      </div>

      <template v-else>
        <div class="xeno-preview-sidebar w-72 flex-none flex flex-col">
          <div class="flex-none px-3 py-2 border-b border-white/10">
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
              {{ t("analysis.filter.stats.blocks") }}
              ({{ result.blocks.length }}
              <template
                v-if="
                  result.pagination &&
                  result.pagination.totalBlocks > result.blocks.length
                "
              >
                /{{ result.pagination.totalBlocks }}
              </template>
              )
            </span>
          </div>

          <div
            ref="blockListRef"
            class="flex-1 overflow-y-auto"
            @scroll="handleBlockListScroll"
          >
            <div
              :style="{
                height: `${blockVirtualizer.getTotalSize()}px`,
                position: 'relative',
              }"
            >
              <div
                v-for="virtualItem in virtualBlocks"
                :key="String(virtualItem.key)"
                :style="{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${virtualItem.start}px)`,
                }"
              >
                <div
                  class="xeno-preview-block cursor-pointer px-3 py-3 transition-colors"
                  :class="
                    selectedBlockIndex === virtualItem.index
                      ? 'xeno-preview-block-active'
                      : 'hover:bg-white/5'
                  "
                  @click="selectBlock(virtualItem.index)"
                >
                  <div class="flex items-center justify-between mb-1">
                    <span
                      class="text-sm font-medium text-gray-700 dark:text-gray-300"
                    >
                      #{{ virtualItem.index + 1 }}
                    </span>
                    <span
                      v-if="
                        (getBlockAtReversedIndex(virtualItem.index)?.hitCount ??
                          0) > 0
                      "
                      class="text-xs text-primary-500"
                    >
                      {{
                        t("analysis.filter.previewHitCount", {
                          count:
                            getBlockAtReversedIndex(virtualItem.index)
                              ?.hitCount ?? 0,
                        })
                      }}
                    </span>
                  </div>
                  <div class="text-xs text-gray-500">
                    {{
                      formatDateTime(
                        getBlockAtReversedIndex(virtualItem.index)?.startTs ??
                          0,
                      )
                    }}
                  </div>
                  <div
                    class="flex items-center gap-2 text-xs text-gray-400 mt-1"
                  >
                    <span>
                      {{
                        t("analysis.filter.previewMessageCount", {
                          count:
                            getBlockAtReversedIndex(virtualItem.index)?.messages
                              .length ?? 0,
                        })
                      }}
                    </span>
                    <span>·</span>
                    <span>
                      {{
                        formatDuration(
                          getBlockAtReversedIndex(virtualItem.index)?.startTs ??
                            0,
                          getBlockAtReversedIndex(virtualItem.index)?.endTs ??
                            0,
                        )
                      }}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <div
              v-if="result.pagination?.hasMore"
              class="py-3 text-center text-sm text-gray-500 dark:text-gray-400 border-t border-white/10"
            >
              <template v-if="isLoadingMore">
                <UIcon
                  name="i-heroicons-arrow-path"
                  class="w-4 h-4 animate-spin inline mr-1"
                />
                {{ t("common.loading") }}
              </template>
              <template v-else>
                <button
                  class="text-primary-500 hover:text-primary-600"
                  @click="emit('load-more')"
                >
                  {{ t("analysis.filter.loadMore") }}
                </button>
              </template>
            </div>
            <div
              v-else-if="
                result.pagination &&
                result.blocks.length >= result.pagination.totalBlocks
              "
              class="py-3 text-center text-xs text-gray-400 dark:text-gray-500"
            >
              {{ t("analysis.filter.allLoaded") }}
            </div>
          </div>
        </div>

        <div class="flex-1 overflow-hidden min-w-0">
          <MessageList
            v-if="currentBlockMessages.length > 0"
            ref="messageListRef"
            :query="emptyQuery"
            :external-messages="currentBlockMessages"
            :hit-message-ids="hitMessageIds"
            class="h-full"
            @reach-bottom="goToNextBlock"
            @reach-top="goToPrevBlock"
          />
          <div
            v-else
            class="flex items-center justify-center h-full text-gray-400"
          >
            {{ t("analysis.filter.noResults") }}
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.xeno-preview-shell {
  background:
    radial-gradient(
      circle at top left,
      rgba(99, 102, 241, 0.12),
      transparent 36%
    ),
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 28%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.02), rgba(255, 255, 255, 0));
}

.xeno-preview-header,
.xeno-preview-sidebar,
.xeno-preview-empty {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(15, 23, 42, 0.72);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.06),
    0 18px 40px rgba(2, 6, 23, 0.22);
  backdrop-filter: blur(18px);
}

.xeno-preview-sidebar,
.xeno-preview-empty {
  margin: 0.75rem;
  border-radius: 1.25rem;
}

.xeno-preview-header {
  margin: 0.75rem 0.75rem 0;
  border-radius: 1.25rem;
}

.xeno-preview-block {
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.xeno-preview-block-active {
  background:
    linear-gradient(90deg, rgba(59, 130, 246, 0.18), rgba(59, 130, 246, 0.05)),
    rgba(255, 255, 255, 0.02);
  box-shadow: inset 3px 0 0 rgba(96, 165, 250, 0.88);
}

.xeno-preview-empty {
  padding: 2.5rem;
  max-width: 22rem;
}
</style>
