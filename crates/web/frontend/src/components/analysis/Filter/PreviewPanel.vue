<script setup lang="ts">
/**
 * English note.
 * English note.
 * English note.
 * English note.
 */

import { computed, ref, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useVirtualizer } from '@tanstack/vue-virtual'
import MessageList from '@/components/common/ChatRecord/MessageList.vue'
import LoadingState from '@/components/UI/LoadingState.vue'
import type { ChatRecordMessage } from '@/types/format'

const { t } = useI18n()

// English engineering note.
interface PaginationInfo {
  page: number
  pageSize: number
  totalBlocks: number
  totalHits: number
  hasMore: boolean
}

// Props
const props = defineProps<{
  result: {
    blocks: Array<{
      startTs: number
      endTs: number
      messages: Array<{
        id: number
        senderName: string
        senderPlatformId: string
        senderAliases: string[]
        senderAvatar: string | null
        content: string
        timestamp: number
        type: number
        replyToMessageId: string | null
        replyToContent: string | null
        replyToSenderName: string | null
        isHit: boolean
      }>
      hitCount: number
    }>
    stats: {
      totalMessages: number
      hitMessages: number
      totalChars: number
    }
    pagination?: PaginationInfo
  } | null
  isLoading: boolean
  isLoadingMore?: boolean
  estimatedTokens: number
  tokenStatus: 'green' | 'yellow' | 'red'
}>()

// Emits
const emit = defineEmits<{
  (e: 'load-more'): void
}>()

// English engineering note.
const selectedBlockIndex = ref(0)

// English engineering note.
let isBlockSwitching = false

// English engineering note.
const pendingScrollToMessageId = ref<number | null>(null)

// English engineering note.
const messageListRef = ref<InstanceType<typeof MessageList> | null>(null)

// English engineering note.
const blockListRef = ref<HTMLElement | null>(null)

// English engineering note.
function getBlockAtReversedIndex(index: number) {
  if (!props.result) return null
  const originalIndex = props.result.blocks.length - 1 - index
  return props.result.blocks[originalIndex]
}

// English engineering note.
const blockCount = computed(() => props.result?.blocks.length ?? 0)

// English engineering note.
const blockVirtualizer = useVirtualizer(
  computed(() => ({
    count: blockCount.value,
    getScrollElement: () => blockListRef.value,
    estimateSize: () => 72, // English engineering note.
    overscan: 5,
  }))
)

const virtualBlocks = computed(() => blockVirtualizer.value.getVirtualItems())

// English engineering note.
const tokenProgressColor = computed(() => {
  switch (props.tokenStatus) {
    case 'green':
      return 'bg-green-500'
    case 'yellow':
      return 'bg-yellow-500'
    case 'red':
      return 'bg-red-500'
    default:
      return 'bg-gray-400'
  }
})

// English engineering note.
const tokenProgressPercent = computed(() => {
  return Math.min((props.estimatedTokens / 100000) * 100, 100)
})

// English engineering note.
const currentBlockMessages = computed<ChatRecordMessage[]>(() => {
  if (blockCount.value === 0) return []
  const block = getBlockAtReversedIndex(selectedBlockIndex.value)
  if (!block) return []

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
  }))
})

// English engineering note.
const hitMessageIds = computed<number[]>(() => {
  if (blockCount.value === 0) return []
  const block = getBlockAtReversedIndex(selectedBlockIndex.value)
  if (!block) return []

  return block.messages.filter((msg) => msg.isHit).map((msg) => msg.id)
})

// English engineering note.
const emptyQuery = { startTs: 0, endTs: 0 }

// English engineering note.
// English engineering note.
const shouldShowYear = computed(() => {
  if (!props.result || props.result.blocks.length === 0) return false

  const blocks = props.result.blocks
  const firstYear = new Date(blocks[0].startTs * 1000).getFullYear()
  const lastYear = new Date(blocks[blocks.length - 1].endTs * 1000).getFullYear()

  return firstYear !== lastYear
})

// English engineering note.
function formatDateTime(ts: number): string {
  const options: Intl.DateTimeFormatOptions = {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }

  if (shouldShowYear.value) {
    options.year = 'numeric'
  }

  return new Date(ts * 1000).toLocaleString('zh-CN', options)
}

function formatDuration(startTs: number, endTs: number): string {
  const diff = endTs - startTs
  if (diff < 60) return `${diff}秒`
  if (diff < 3600) return `${Math.floor(diff / 60)}分钟`
  return `${Math.floor(diff / 3600)}h${Math.floor((diff % 3600) / 60)}m`
}

// English engineering note.
function selectBlock(index: number) {
  selectedBlockIndex.value = index

  // English engineering note.
  const block = getBlockAtReversedIndex(index)
  if (block) {
    const firstHitMessage = block.messages.find((msg) => msg.isHit)
    if (firstHitMessage) {
      pendingScrollToMessageId.value = firstHitMessage.id
    }
  }
}

// English engineering note.
function goToNextBlock() {
  if (isBlockSwitching) return // English engineering note.
  if (blockCount.value === 0) return
  if (selectedBlockIndex.value < blockCount.value - 1) {
    isBlockSwitching = true
    selectedBlockIndex.value++
    scrollToBlockInList(selectedBlockIndex.value)
    // English engineering note.
    setTimeout(() => {
      isBlockSwitching = false
    }, 300)
  }
}

// English engineering note.

function goToPrevBlock() {
  // English engineering note.
}

// English engineering note.
function scrollToBlockInList(index: number) {
  blockVirtualizer.value.scrollToIndex(index, { align: 'center' })
}

// English engineering note.
watch(
  () => props.result,
  () => {
    selectedBlockIndex.value = 0
    pendingScrollToMessageId.value = null
  }
)

// English engineering note.
watch(pendingScrollToMessageId, async (messageId) => {
  if (messageId !== null) {
    await nextTick()
    // English engineering note.
    setTimeout(() => {
      messageListRef.value?.scrollToMessage(messageId)
      pendingScrollToMessageId.value = null
    }, 100)
  }
})

// English engineering note.
function handleBlockListScroll(event: Event) {
  const target = event.target as HTMLElement
  if (!target || !props.result?.pagination?.hasMore || props.isLoadingMore) return

  // English engineering note.
  const threshold = 100
  const { scrollTop, scrollHeight, clientHeight } = target
  if (scrollHeight - scrollTop - clientHeight < threshold) {
    emit('load-more')
  }
}
</script>

<template>
  <div class="flex-1 flex flex-col overflow-hidden bg-gray-50 dark:bg-gray-900">
    <!-- English UI note -->
    <div
      v-if="result && result.blocks.length > 0"
      class="flex-none px-4 py-3 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700"
    >
      <div class="flex items-center justify-between mb-2">
        <div class="flex items-center gap-6 text-sm">
          <span class="text-gray-600 dark:text-gray-400">
            {{ t('analysis.filter.stats.blocks') }}:
            <span class="font-medium text-gray-900 dark:text-white">
              {{ result.blocks.length }}
              <template v-if="result.pagination && result.pagination.totalBlocks > result.blocks.length">
                / {{ result.pagination.totalBlocks }}
              </template>
            </span>
          </span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ t('analysis.filter.stats.messages') }}:
            <span class="font-medium text-gray-900 dark:text-white">{{ result.stats.totalMessages }}</span>
          </span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ t('analysis.filter.stats.hits') }}:
            <span class="font-medium text-primary-500">
              {{ result.pagination?.totalHits ?? result.stats.hitMessages }}
            </span>
          </span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ t('analysis.filter.stats.chars') }}:
            <span class="font-medium text-gray-900 dark:text-white">
              {{ result.stats.totalChars.toLocaleString() }}
            </span>
          </span>
        </div>
      </div>

      <!-- English UI note -->
      <div class="flex items-center gap-3">
        <span class="text-sm text-gray-600 dark:text-gray-400 whitespace-nowrap">
          {{ t('analysis.filter.stats.tokens') }}:
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
        <div class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-300"
            :class="tokenProgressColor"
            :style="{ width: `${tokenProgressPercent}%` }"
          />
        </div>
        <span class="text-xs text-gray-500 whitespace-nowrap">10K</span>
      </div>

      <!-- English UI note -->
      <div v-if="tokenStatus === 'yellow'" class="mt-2 text-xs text-yellow-600 dark:text-yellow-400">
        {{ t('analysis.filter.tokenWarning.yellow') }}
      </div>
      <div v-if="tokenStatus === 'red'" class="mt-2 text-xs text-red-600 dark:text-red-400">
        {{ t('analysis.filter.tokenWarning.red') }}
      </div>
    </div>

    <!-- English UI note -->
    <div class="flex-1 min-h-0 flex overflow-hidden">
      <!-- English UI note -->
      <LoadingState v-if="isLoading" variant="page" :text="t('analysis.filter.filtering')" />

      <!-- English UI note -->
      <div v-else-if="!result" class="w-full h-full flex items-center justify-center">
        <div class="text-center text-gray-400">
          <UIcon name="i-heroicons-funnel" class="w-12 h-12 mb-3 mx-auto" />
          <p>{{ t('analysis.filter.emptyHint') }}</p>
        </div>
      </div>

      <!-- English UI note -->
      <div v-else-if="result.blocks.length === 0" class="flex-1 flex items-center justify-center">
        <div class="text-center text-gray-400">
          <UIcon name="i-heroicons-magnifying-glass" class="w-12 h-12 mb-3 mx-auto" />
          <p>{{ t('analysis.filter.noResults') }}</p>
        </div>
      </div>

      <!-- English UI note -->
      <template v-else>
        <!-- English UI note -->
        <div
          class="w-64 flex-none border-r border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 flex flex-col"
        >
          <div class="flex-none px-3 py-2 border-b border-gray-200 dark:border-gray-700">
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
              {{ t('analysis.filter.stats.blocks') }}
              ({{ result.blocks.length }}
              <template v-if="result.pagination && result.pagination.totalBlocks > result.blocks.length">
                /{{ result.pagination.totalBlocks }}
              </template>
              )
            </span>
          </div>

          <div ref="blockListRef" class="flex-1 overflow-y-auto" @scroll="handleBlockListScroll">
            <div :style="{ height: `${blockVirtualizer.getTotalSize()}px`, position: 'relative' }">
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
                  class="px-3 py-2 cursor-pointer border-b border-gray-100 dark:border-gray-700 transition-colors"
                  :class="
                    selectedBlockIndex === virtualItem.index
                      ? 'bg-primary-50 dark:bg-primary-900/30 border-l-2 border-l-primary-500'
                      : 'hover:bg-gray-50 dark:hover:bg-gray-700/50'
                  "
                  @click="selectBlock(virtualItem.index)"
                >
                  <div class="flex items-center justify-between mb-1">
                    <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                      #{{ virtualItem.index + 1 }}
                    </span>
                    <span
                      v-if="(getBlockAtReversedIndex(virtualItem.index)?.hitCount ?? 0) > 0"
                      class="text-xs text-primary-500"
                    >
                      含 {{ getBlockAtReversedIndex(virtualItem.index)?.hitCount }} 个结果
                    </span>
                  </div>
                  <div class="text-xs text-gray-500">
                    {{ formatDateTime(getBlockAtReversedIndex(virtualItem.index)?.startTs ?? 0) }}
                  </div>
                  <div class="flex items-center gap-2 text-xs text-gray-400 mt-1">
                    <span>{{ getBlockAtReversedIndex(virtualItem.index)?.messages.length ?? 0 }} 条</span>
                    <span>·</span>
                    <span>
                      {{
                        formatDuration(
                          getBlockAtReversedIndex(virtualItem.index)?.startTs ?? 0,
                          getBlockAtReversedIndex(virtualItem.index)?.endTs ?? 0
                        )
                      }}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- English UI note -->
            <div
              v-if="result.pagination?.hasMore"
              class="py-3 text-center text-sm text-gray-500 dark:text-gray-400 border-t border-gray-100 dark:border-gray-700"
            >
              <template v-if="isLoadingMore">
                <UIcon name="i-heroicons-arrow-path" class="w-4 h-4 animate-spin inline mr-1" />
                {{ t('common.loading') }}
              </template>
              <template v-else>
                <button class="text-primary-500 hover:text-primary-600" @click="emit('load-more')">
                  {{ t('analysis.filter.loadMore') }}
                </button>
              </template>
            </div>
            <div
              v-else-if="result.pagination && result.blocks.length >= result.pagination.totalBlocks"
              class="py-3 text-center text-xs text-gray-400 dark:text-gray-500"
            >
              {{ t('analysis.filter.allLoaded') }}
            </div>
          </div>
        </div>

        <!-- English UI note -->
        <div class="flex-1 overflow-hidden">
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
          <div v-else class="flex items-center justify-center h-full text-gray-400">
            {{ t('analysis.filter.noResults') }}
          </div>
        </div>
      </template>
    </div>
  </div>
</template>
