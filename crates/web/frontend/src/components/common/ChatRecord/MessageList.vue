<script setup lang="ts">
/**
 * Virtualized chat message list.
 * Powered by @tanstack/vue-virtual to keep large sessions responsive.
 */
import { ref, watch, nextTick, toRaw, computed, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useVirtualizer } from '@tanstack/vue-virtual'
import dayjs from 'dayjs'
import MessageItem from './MessageItem.vue'
import type { ChatRecordMessage, ChatRecordQuery } from './types'
import { useSessionStore } from '@/stores/session'

// Insert a separator when the gap between adjacent messages exceeds this threshold.
const TIME_SEPARATOR_THRESHOLD = 5 * 60 // 5 minutes

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    /** Active query parameters. */
    query: ChatRecordQuery
    /** Optional externally supplied message list (disables internal loading). */
    externalMessages?: ChatRecordMessage[]
    /** Message ids to highlight in external mode. */
    hitMessageIds?: number[]
    /** Scroll strategy when external messages refresh. */
    externalScrollBehavior?: 'top' | 'preserve'
  }>(),
  {
    externalMessages: undefined,
    hitMessageIds: () => [],
    externalScrollBehavior: 'top',
  }
)

const emit = defineEmits<{
  /** Message count update. */
  (e: 'count-change', count: number): void
  /** Mid-viewport message changed (used by timeline sync). */
  (e: 'visible-message-change', payload: { id: number; timestamp: number }): void
  /** Request context jump for the selected message. */
  (e: 'jump-to-message', messageId: number): void
  /** External-mode pagination signal: reached bottom. */
  (e: 'reach-bottom'): void
  /** External-mode pagination signal: reached top. */
  (e: 'reach-top'): void
  /** Timestamp list update for session-timeline filtering. */
  (e: 'message-timestamps-change', timestamps: number[]): void
}>()

const sessionStore = useSessionStore()

// External mode bypasses internal fetch logic.
const isExternalMode = computed(() => !!props.externalMessages?.length)

// Context shortcut buttons are shown only in filtered internal mode.
const isFiltered = computed(() => {
  if (isExternalMode.value) return false
  const q = props.query
  return !!(q.memberId || q.keywords?.length)
})

// Message list state.
const messages = ref<ChatRecordMessage[]>([])
const isLoading = ref(false)
const isLoadingMore = ref(false)
const hasMoreBefore = ref(false)
const hasMoreAfter = ref(false)

// Search pagination state (used by keyword and semantic modes).
const isSearchMode = ref(false)
const searchOffset = ref(0)

// Scroll container reference.
const scrollContainerRef = ref<HTMLElement | null>(null)

// Deferred scroll target after initial fetch.
const pendingScrollToId = ref<number | null>(null)

// Initial row height estimate for virtualizer.
const ESTIMATED_MESSAGE_HEIGHT = 80

// Virtualizer instance.
const virtualizer = useVirtualizer(
  computed(() => ({
    count: messages.value.length,
    getScrollElement: () => scrollContainerRef.value,
    estimateSize: () => ESTIMATED_MESSAGE_HEIGHT,
    overscan: 10, // Render 10 rows above and below the viewport.
    getItemKey: (index: number) => messages.value[index]?.id ?? index,
  }))
)

// Current visible virtual rows.
const virtualItems = computed(() => virtualizer.value.getVirtualItems())

// Total virtualized list height.
const totalSize = computed(() => virtualizer.value.getTotalSize())

// Build normalized query params for data requests.
function buildFilterParams(query: ChatRecordQuery) {
  const mode: NonNullable<ChatRecordQuery['searchMode']> =
    query.searchMode === 'semantic' ? 'semantic' : 'keyword'
  return {
    filter: query.startTs || query.endTs ? { startTs: query.startTs, endTs: query.endTs } : undefined,
    senderId: query.memberId,
    keywords: query.keywords ? [...toRaw(query.keywords)] : undefined,
    searchMode: mode,
    semanticThreshold:
      typeof query.semanticThreshold === 'number' ? Math.max(-1, Math.min(1, query.semanticThreshold)) : 0.45,
  }
}

// Normalize missing nullable reply fields.
function mapMessages(messages: any[]): ChatRecordMessage[] {
  return messages.map((m) => ({
    ...m,
    replyToMessageId: m.replyToMessageId ?? null,
    replyToContent: m.replyToContent ?? null,
    replyToSenderName: m.replyToSenderName ?? null,
  })) as ChatRecordMessage[]
}

// External mode diffing helper.
let previousExternalMessageCount = 0

// Initial load for the current query/session.
async function loadInitialMessages() {
  // External mode: consume injected messages directly.
  if (isExternalMode.value) {
    const currentCount = props.externalMessages!.length
    const isExpanding = previousExternalMessageCount > 0 && currentCount > previousExternalMessageCount

    messages.value = props.externalMessages!
    hasMoreBefore.value = false
    hasMoreAfter.value = false
    isSearchMode.value = false
    emit('count-change', messages.value.length)

    previousExternalMessageCount = currentCount

    await nextTick()

    // Keep the view stable when appending in preserve mode.
    if (props.externalScrollBehavior === 'preserve' && isExpanding) {
    } else {
      scrollToTop()
    }
    return
  }

  const sessionId = sessionStore.currentSessionId
  if (!sessionId) {
    messages.value = []
    emit('count-change', 0)
    return
  }

  isLoading.value = true
  messages.value = []
  pendingScrollToId.value = null

  try {
    const query = toRaw(props.query)
    const { filter, senderId, keywords, searchMode, semanticThreshold } = buildFilterParams(query)
    const targetId = query.scrollToMessageId

    if (targetId) {
      // Center around the requested message id.
      const [beforeResult, afterResult] = await Promise.all([
        window.aiApi.getMessagesBefore(sessionId, targetId, 50, filter, senderId, keywords),
        window.aiApi.getMessagesAfter(sessionId, targetId, 50, filter, senderId, keywords),
      ])

      // Include the target message itself.
      const targetMessages = await window.aiApi.getMessageContext(sessionId, targetId, 0)

      // Merge final centered list.
      messages.value = mapMessages([...beforeResult.messages, ...targetMessages, ...afterResult.messages])

      hasMoreBefore.value = beforeResult.hasMore
      hasMoreAfter.value = afterResult.hasMore

      // Scroll to target after virtualizer settles.
      pendingScrollToId.value = targetId
    } else if (keywords && keywords.length > 0) {
      // Keyword or semantic search mode.
      isSearchMode.value = true
      searchOffset.value = 0
      if (searchMode === 'semantic') {
        const semanticQuery = keywords.join(' ').trim()
        const result = await window.aiApi.semanticSearchMessages(
          sessionId,
          semanticQuery,
          filter,
          semanticThreshold,
          100,
          0,
          senderId
        )
        messages.value = mapMessages(result.messages)
        hasMoreBefore.value = false
        searchOffset.value = result.messages.length
        const total = Number.isFinite(Number(result.totalCount)) ? Number(result.totalCount) : result.messages.length
        hasMoreAfter.value = total > searchOffset.value
      } else {
        const result = await window.aiApi.searchMessages(sessionId, keywords, filter, 100, 0, senderId)
        messages.value = mapMessages(result.messages)
        hasMoreBefore.value = false
        hasMoreAfter.value = result.messages.length >= 100
        searchOffset.value = result.messages.length
      }

      await nextTick()
      scrollToTop()
    } else {
      // Default mode: load newest messages.
      isSearchMode.value = false
      searchOffset.value = 0
      const result = await window.aiApi.getAllRecentMessages(sessionId, filter, 100)
      messages.value = mapMessages(result.messages)
      hasMoreBefore.value = result.messages.length >= 100
      hasMoreAfter.value = false

      // Jump to the newest message once virtualizer layout is stable.
      await nextTick()
      setTimeout(() => {
        scrollToBottom()
      }, 50)
    }

    emit('count-change', messages.value.length)

    // Keep timeline filter options aligned with loaded rows.
    const timestamps = messages.value.map((m) => m.timestamp)
    emit('message-timestamps-change', timestamps)

    // Finalize deferred scroll if needed.
    if (pendingScrollToId.value) {
      await nextTick()
      setTimeout(() => {
        if (pendingScrollToId.value) {
          scrollToMessage(pendingScrollToId.value)
          pendingScrollToId.value = null
        }
      }, 100)
    }
  } catch (e) {
    console.error('failed to load messages:', e)
    messages.value = []
    emit('count-change', 0)
  } finally {
    isLoading.value = false
  }
}

// Scroll to top.
function scrollToTop() {
  virtualizer.value.scrollToOffset(0)
}

// Scroll to bottom.
function scrollToBottom() {
  if (messages.value.length === 0) return

  // First move by virtual index.
  virtualizer.value.scrollToIndex(messages.value.length - 1, { align: 'end' })

  // Then force-scroll container in case row measurements changed.
  requestAnimationFrame(() => {
    const container = scrollContainerRef.value
    if (container) {
      container.scrollTop = container.scrollHeight
    }
  })
}

// Load older messages when user scrolls near the top.
async function loadMoreBefore() {
  if (isLoadingMore.value || !hasMoreBefore.value || messages.value.length === 0) return

  const sessionId = sessionStore.currentSessionId
  if (!sessionId) return

  const firstMessage = messages.value[0]
  if (!firstMessage) return

  isLoadingMore.value = true

  try {
    const query = toRaw(props.query)
    const { filter, senderId, keywords } = buildFilterParams(query)
    const result = await window.aiApi.getMessagesBefore(sessionId, firstMessage.id, 50, filter, senderId, keywords)

    if (result.messages.length > 0) {
      // Preserve viewport after prepending rows.
      const currentOffset = virtualizer.value.scrollOffset ?? 0

      const newMessages = [...mapMessages(result.messages), ...messages.value]
      messages.value = newMessages

      await nextTick()
      const addedCount = result.messages.length
      const estimatedAddedHeight = addedCount * ESTIMATED_MESSAGE_HEIGHT
      virtualizer.value.scrollToOffset(currentOffset + estimatedAddedHeight)

      emit('count-change', messages.value.length)
      emit(
        'message-timestamps-change',
        messages.value.map((m) => m.timestamp)
      )
    }

    hasMoreBefore.value = result.hasMore
  } catch (e) {
    console.error('failed to load older messages:', e)
  } finally {
    isLoadingMore.value = false
  }
}

// Load newer messages when user scrolls near the bottom.
async function loadMoreAfter() {
  if (isLoadingMore.value || !hasMoreAfter.value || messages.value.length === 0) return

  const sessionId = sessionStore.currentSessionId
  if (!sessionId) return

  isLoadingMore.value = true

  try {
    const query = toRaw(props.query)
    const { filter, senderId, keywords, searchMode, semanticThreshold } = buildFilterParams(query)

    if (isSearchMode.value && keywords && keywords.length > 0) {
      if (searchMode === 'semantic') {
        const semanticQuery = keywords.join(' ').trim()
        const result = await window.aiApi.semanticSearchMessages(
          sessionId,
          semanticQuery,
          filter,
          semanticThreshold,
          50,
          searchOffset.value,
          senderId
        )
        if (result.messages.length > 0) {
          messages.value = [...messages.value, ...mapMessages(result.messages)]
          searchOffset.value += result.messages.length
          emit('count-change', messages.value.length)
          emit(
            'message-timestamps-change',
            messages.value.map((m) => m.timestamp)
          )
        }
        const total = Number.isFinite(Number(result.totalCount)) ? Number(result.totalCount) : 0
        hasMoreAfter.value = total > searchOffset.value
      } else {
        const result = await window.aiApi.searchMessages(sessionId, keywords, filter, 50, searchOffset.value, senderId)

        if (result.messages.length > 0) {
          messages.value = [...messages.value, ...mapMessages(result.messages)]
          searchOffset.value += result.messages.length
          emit('count-change', messages.value.length)
          emit(
            'message-timestamps-change',
            messages.value.map((m) => m.timestamp)
          )
        }

        hasMoreAfter.value = result.messages.length >= 50
      }
    } else {
      // Default paged flow by message id.
      const lastMessage = messages.value[messages.value.length - 1]
      if (!lastMessage) return

      const result = await window.aiApi.getMessagesAfter(sessionId, lastMessage.id, 50, filter, senderId, keywords)

      if (result.messages.length > 0) {
        messages.value = [...messages.value, ...mapMessages(result.messages)]
        emit('count-change', messages.value.length)
        emit(
          'message-timestamps-change',
          messages.value.map((m) => m.timestamp)
        )
      }

      hasMoreAfter.value = result.hasMore
    }
  } catch (e) {
    console.error('failed to load more messages:', e)
  } finally {
    isLoadingMore.value = false
  }
}

// Scroll to a specific message id.
function scrollToMessage(messageId: number) {
  const index = messages.value.findIndex((m) => m.id === messageId)
  if (index !== -1) {
    virtualizer.value.scrollToIndex(index, { align: 'center' })
  }
}

// Scroll handler for lazy loading and timeline sync.
function handleScroll() {
  const container = scrollContainerRef.value
  if (!container) return

  const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight

  // External mode emits pagination boundaries to parent.
  if (isExternalMode.value) {
    if (container.scrollTop < 50) {
      emit('reach-top')
    }
    if (distanceFromBottom < 50) {
      emit('reach-bottom')
    }
  }

  // Internal mode triggers lazy loading near boundaries.
  if (!isExternalMode.value && !isLoadingMore.value) {
    if (container.scrollTop < 100 && hasMoreBefore.value) {
      loadMoreBefore()
    }

    if (distanceFromBottom < 100 && hasMoreAfter.value) {
      loadMoreAfter()
    }
  }

  // Debounce timeline synchronization updates.
  scheduleVisibleMessageUpdate()
}

// Debounce timer for visible-row reporting.
let visibleMessageTimer: ReturnType<typeof setTimeout> | null = null
let lastEmittedMessageId = 0

// Schedule visible-row update once per debounce window.
function scheduleVisibleMessageUpdate() {
  if (visibleMessageTimer) return

  visibleMessageTimer = setTimeout(() => {
    visibleMessageTimer = null
    updateVisibleMessage()
  }, 150)
}

// Report a representative visible message to parent for timeline alignment.
function updateVisibleMessage() {
  const items = virtualItems.value
  if (items.length === 0) return

  // Use the middle visible row as the active anchor.
  const middleIndex = Math.floor(items.length / 2)
  const middleItem = items[middleIndex]
  if (!middleItem) return

  const message = messages.value[middleItem.index]
  if (message && message.id !== lastEmittedMessageId) {
    lastEmittedMessageId = message.id
    emit('visible-message-change', { id: message.id, timestamp: message.timestamp })
  }
}

// Target highlight helper.
function isTargetMessage(msgId: number): boolean {
  if (isExternalMode.value && props.hitMessageIds?.length) {
    return props.hitMessageIds.includes(msgId)
  }
  return msgId === props.query.scrollToMessageId
}

/**
 * Return the time-separator label for a message row if needed.
 */
function getTimeSeparator(index: number): string | null {
  const currentMsg = messages.value[index]
  if (!currentMsg) return null

  const prevMsg = index > 0 ? messages.value[index - 1] : null

  // Always show separator for the first row.
  if (!prevMsg) {
    return formatSeparatorTime(currentMsg.timestamp)
  }

  const currentTs = currentMsg.timestamp
  const prevTs = prevMsg.timestamp
  const gap = currentTs - prevTs

  // Add separator when day changes or idle gap is large.
  const currentDay = dayjs.unix(currentTs).startOf('day')
  const prevDay = dayjs.unix(prevTs).startOf('day')
  const isDifferentDay = !currentDay.isSame(prevDay)

  if (isDifferentDay || gap >= TIME_SEPARATOR_THRESHOLD) {
    return formatSeparatorTime(currentTs)
  }

  return null
}

/**
 * Format separator timestamps.
 */
function formatSeparatorTime(timestamp: number): string {
  const msgTime = dayjs.unix(timestamp)
  const today = dayjs().startOf('day')

  if (msgTime.isSame(today, 'day')) {
    return msgTime.format('HH:mm')
  }
  return msgTime.format('YYYY-MM-DD HH:mm')
}

// Callback for dynamic row measurement.
function measureElement(el: Element | null) {
  if (el) {
    virtualizer.value.measureElement(el)
  }
}

// React to internal query updates.
watch(
  () => props.query,
  () => {
    if (!isExternalMode.value) {
      loadInitialMessages()
    }
  },
  { deep: true }
)

// React to external message list updates.
watch(
  () => props.externalMessages,
  () => {
    if (isExternalMode.value) {
      loadInitialMessages()
    }
  },
  { deep: true, immediate: true }
)

// Cleanup pending timers.
onUnmounted(() => {
  if (visibleMessageTimer) {
    clearTimeout(visibleMessageTimer)
    visibleMessageTimer = null
  }
})

// Public methods exposed to parent component.
defineExpose({
  refresh: loadInitialMessages,
  scrollToMessage,
})
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden">
    <!-- Loading state -->
    <div v-if="isLoading" class="flex h-full items-center justify-center">
      <div class="text-center">
        <UIcon name="i-heroicons-arrow-path" class="h-8 w-8 animate-spin text-gray-400" />
        <p class="mt-2 text-sm text-gray-500">{{ t('records.messageList.loading') }}</p>
      </div>
    </div>

    <!-- Empty state -->
    <div v-else-if="messages.length === 0" class="flex h-full items-center justify-center">
      <div class="text-center">
        <UIcon name="i-heroicons-chat-bubble-left-right" class="h-12 w-12 text-gray-300 dark:text-gray-600" />
        <p class="mt-2 text-sm text-gray-500">{{ t('records.messageList.noMessages') }}</p>
        <p class="mt-1 text-xs text-gray-400">{{ t('records.messageList.tryAdjustFilter') }}</p>
      </div>
    </div>

    <!-- Virtual scroll container -->
    <div v-else ref="scrollContainerRef" class="h-full overflow-y-auto" @scroll="handleScroll">
      <!-- Top loading indicator -->
      <div v-if="hasMoreBefore" class="flex justify-center py-2">
        <span v-if="isLoadingMore" class="text-xs text-gray-400">
          <UIcon name="i-heroicons-arrow-path" class="mr-1 inline h-3 w-3 animate-spin" />
          {{ t('records.messageList.loadingMore') }}
        </span>
        <span v-else class="text-xs text-gray-400">{{ t('records.messageList.scrollUpForMore') }}</span>
      </div>

      <!-- Virtualized list -->
      <div class="relative w-full" :style="{ height: `${totalSize}px` }">
        <div
          v-for="virtualItem in virtualItems"
          :key="String(virtualItem.key)"
          :ref="(el) => measureElement(el as Element)"
          class="absolute left-0 top-0 w-full"
          :style="{
            transform: `translateY(${virtualItem.start}px)`,
          }"
          :data-index="virtualItem.index"
        >
          <!-- Time separator -->
          <div v-if="getTimeSeparator(virtualItem.index)" class="flex items-center justify-center py-2">
            <div class="flex items-center gap-2 text-xs text-gray-400">
              <div class="h-px w-8 bg-gray-200 dark:bg-gray-700" />
              <span>{{ getTimeSeparator(virtualItem.index) }}</span>
              <div class="h-px w-8 bg-gray-200 dark:bg-gray-700" />
            </div>
          </div>

          <!-- Message row -->
          <MessageItem
            :data-message-id="messages[virtualItem.index]?.id"
            :message="messages[virtualItem.index]!"
            :is-target="isTargetMessage(messages[virtualItem.index]?.id ?? 0)"
            :highlight-keywords="query.highlightKeywords"
            :is-filtered="isFiltered"
            @view-context="(id) => emit('jump-to-message', id)"
          />
        </div>
      </div>

      <!-- Bottom loading indicator -->
      <div v-if="hasMoreAfter" class="flex justify-center py-2">
        <span v-if="isLoadingMore" class="text-xs text-gray-400">
          <UIcon name="i-heroicons-arrow-path" class="mr-1 inline h-3 w-3 animate-spin" />
          {{ t('records.messageList.loadingMore') }}
        </span>
        <span v-else class="text-xs text-gray-400">{{ t('records.messageList.scrollDownForMore') }}</span>
      </div>
    </div>
  </div>
</template>
