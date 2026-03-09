<script setup lang="ts">
/**
 * Session timeline rail used to jump between segmented chat windows.
 */
import { ref, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useVirtualizer } from '@tanstack/vue-virtual'
import BatchSummaryModal from './BatchSummaryModal.vue'

interface ChatSessionItem {
  id: number
  startTs: number
  endTs: number
  messageCount: number
  firstMessageId: number
  /** Optional AI-generated summary for the session window. */
  summary?: string | null
}

// English engineering note.
type FlatListItem =
  | { type: 'date'; date: string; label: string; count: number }
  | { type: 'session'; session: ChatSessionItem }

const props = defineProps<{
  sessionId: string
  /** Currently focused session id. */
  activeSessionId?: number
  /** Whether the rail is collapsed into the slim indicator mode. */
  collapsed?: boolean
  /** Visible filter lower bound. */
  filterStartTs?: number
  /** Visible filter upper bound. */
  filterEndTs?: number
  /** Session ids matched by the active content query. */
  filterMatchedSessionIds?: Set<number>
}>()

const emit = defineEmits<{
  /** Select a session and jump the message list to its first message. */
  (e: 'select', sessionId: number, firstMessageId: number): void
  /** Collapse/expand the rail. */
  (e: 'update:collapsed', value: boolean): void
}>()

const { t, locale } = useI18n()

// English engineering note.
const allSessions = ref<ChatSessionItem[]>([])
const isLoading = ref(true)
const scrollContainerRef = ref<HTMLElement | null>(null)

// English engineering note.
const generatingSummaryIds = ref<Set<number>>(new Set())

// English engineering note.
const showBatchSummaryModal = ref(false)

// English engineering note.
const isCollapsed = computed({
  get: () => props.collapsed ?? false,
  set: (v) => emit('update:collapsed', v),
})

// English engineering note.
const filteredSessions = computed(() => {
  let sessions = allSessions.value
  if (sessions.length === 0) return []

  // English engineering note.
  if (props.filterMatchedSessionIds && props.filterMatchedSessionIds.size > 0) {
    sessions = sessions.filter((session) => props.filterMatchedSessionIds!.has(session.id))
  }
  // English engineering note.
  else if (props.filterStartTs || props.filterEndTs) {
    sessions = sessions.filter((session) => {
      // English engineering note.
      const sessionStart = session.startTs
      const sessionEnd = session.endTs

      if (props.filterStartTs && sessionEnd < props.filterStartTs) return false
      if (props.filterEndTs && sessionStart > props.filterEndTs) return false

      return true
    })
  }

  return sessions
})

// English engineering note.
const flatList = computed<FlatListItem[]>(() => {
  const sessions = filteredSessions.value
  if (sessions.length === 0) return []

  const result: FlatListItem[] = []
  const dateGroups = new Map<string, { label: string; sessions: ChatSessionItem[] }>()

  // English engineering note.
  for (const session of sessions) {
    const dateKey = getDateKey(session.startTs)
    let group = dateGroups.get(dateKey)
    if (!group) {
      group = {
        label: formatDate(session.startTs),
        sessions: [],
      }
      dateGroups.set(dateKey, group)
    }
    group.sessions.push(session)
  }

  // English engineering note.
  const sortedDates = Array.from(dateGroups.entries()).sort((a, b) => a[0].localeCompare(b[0]))

  // English engineering note.
  for (const [dateKey, group] of sortedDates) {
    // English engineering note.
    result.push({
      type: 'date',
      date: dateKey,
      label: group.label,
      count: group.sessions.length,
    })

    // English engineering note.
    const sortedSessions = group.sessions.sort((a, b) => a.startTs - b.startTs)
    for (const session of sortedSessions) {
      result.push({ type: 'session', session })
    }
  }

  return result
})

// English engineering note.
const ESTIMATED_DATE_HEIGHT = 28 // Date separators are compact.
const ESTIMATED_SESSION_HEIGHT = 60 // Session rows include summary affordances.

// English engineering note.
const virtualizer = useVirtualizer(
  computed(() => ({
    count: flatList.value.length,
    getScrollElement: () => scrollContainerRef.value,
    estimateSize: (index: number) => {
      const item = flatList.value[index]
      return item?.type === 'date' ? ESTIMATED_DATE_HEIGHT : ESTIMATED_SESSION_HEIGHT
    },
    overscan: 10,
    getItemKey: (index: number) => {
      const item = flatList.value[index]
      if (!item) return index
      if (item.type === 'date') return `date-${item.date}`
      return `session-${item.session.id}`
    },
  }))
)

// English engineering note.
const virtualItems = computed(() => virtualizer.value.getVirtualItems())

// English engineering note.
const totalSize = computed(() => virtualizer.value.getTotalSize())

// English engineering note.
function formatDate(ts: number): string {
  const date = new Date(ts * 1000)
  return date.toLocaleDateString(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', { month: '2-digit', day: '2-digit' })
}

// English engineering note.
function formatTime(ts: number): string {
  const date = new Date(ts * 1000)
  return date.toLocaleTimeString(locale.value === 'zh-CN' ? 'zh-CN' : 'en-US', { hour: '2-digit', minute: '2-digit' })
}

// English engineering note.
function getDateKey(ts: number): string {
  const date = new Date(ts * 1000)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

// English engineering note.
async function loadSessions() {
  if (!props.sessionId) return

  isLoading.value = true
  try {
    const data = await window.sessionApi.getSessions(props.sessionId)
    allSessions.value = data
    // English engineering note.
    await nextTick()
    scrollToBottom()
  } catch (error) {
    console.error('[SessionTimeline] Failed to load session list:', error)
  } finally {
    isLoading.value = false
  }
}

// English engineering note.
function scrollToBottom() {
  if (flatList.value.length > 0) {
    virtualizer.value.scrollToIndex(flatList.value.length - 1, { align: 'end' })
  }
}

// English engineering note.
function scrollToSession(sessionId: number) {
  const index = flatList.value.findIndex((item) => item.type === 'session' && item.session.id === sessionId)
  if (index !== -1) {
    virtualizer.value.scrollToIndex(index, { align: 'center' })
  }
}

// English engineering note.
function handleSelectSession(session: ChatSessionItem) {
  emit('select', session.id, session.firstMessageId)
}

// English engineering note.
async function generateSummary(session: ChatSessionItem, event: Event) {
  event.stopPropagation() // Keep row selection from firing while generating summary.
  event.preventDefault()

  if (generatingSummaryIds.value.has(session.id)) {
    return
  }

  generatingSummaryIds.value.add(session.id)

  try {
    const result = await window.sessionApi.generateSummary(props.sessionId, session.id, locale.value)

    if (result.success && result.summary) {
      // English engineering note.
      const index = allSessions.value.findIndex((s) => s.id === session.id)
      if (index !== -1) {
        allSessions.value[index] = { ...allSessions.value[index], summary: result.summary }
      }
    }
  } catch (error) {
    console.error('[SessionTimeline] Failed to generate summary:', error)
  } finally {
    generatingSummaryIds.value.delete(session.id)
  }
}

// English engineering note.
function isGenerating(sessionId: number): boolean {
  return generatingSummaryIds.value.has(sessionId)
}

// English engineering note.
function measureElement(el: Element | null) {
  if (el) {
    virtualizer.value.measureElement(el)
  }
}

// English engineering note.
watch(
  () => props.activeSessionId,
  (newId) => {
    if (newId) {
      scrollToSession(newId)
    }
  }
)

// English engineering note.
watch(
  () => props.sessionId,
  () => {
    loadSessions()
  },
  { immediate: true }
)
</script>

<template>
  <!-- English UI note -->
  <div
    v-if="isCollapsed"
    class="xeno-timeline-collapsed flex h-full w-10 flex-col items-center py-2"
  >
    <UButton icon="i-heroicons-chevron-right" variant="ghost" size="xs" @click="isCollapsed = false" />
    <div class="mt-2 flex flex-1 items-center">
      <span class="vertical-text text-xs text-gray-400">{{ t('records.timeline.timeline') }}</span>
    </div>
  </div>

  <!-- English UI note -->
  <div
    v-else
    class="xeno-timeline-shell flex h-full w-44 flex-col"
  >
    <!-- English UI note -->
    <div class="xeno-timeline-header flex items-center justify-between px-2 py-1.5">
      <span class="text-xs font-medium text-gray-600 dark:text-gray-300">{{ t('records.timeline.timeline') }}</span>
      <div class="flex items-center gap-0.5">
        <UTooltip :text="t('records.batchSummary.title')">
          <UButton icon="i-heroicons-sparkles" variant="ghost" size="xs" @click="showBatchSummaryModal = true" />
        </UTooltip>
        <UButton icon="i-heroicons-chevron-left" variant="ghost" size="xs" @click="isCollapsed = true" />
      </div>
    </div>

    <!-- English UI note -->
    <div v-if="isLoading" class="flex flex-1 items-center justify-center">
      <UIcon name="i-heroicons-arrow-path" class="h-4 w-4 animate-spin text-gray-400" />
    </div>

    <!-- English UI note -->
    <div v-else-if="allSessions.length === 0" class="flex flex-1 items-center justify-center p-2">
      <span class="xeno-timeline-empty text-center text-xs text-gray-400">{{ t('records.timeline.noSessions') }}</span>
    </div>

    <!-- English UI note -->
    <div v-else ref="scrollContainerRef" class="xeno-timeline-scroll flex-1 overflow-y-auto py-1">
      <div class="relative w-full" :style="{ height: `${totalSize}px` }">
        <div
          v-for="virtualItem in virtualItems"
          :key="String(virtualItem.key)"
          :ref="(el) => measureElement(el as Element)"
          class="absolute left-0 top-0 w-full"
          :style="{ transform: `translateY(${virtualItem.start}px)` }"
        >
          <!-- English UI note -->
          <template v-if="flatList[virtualItem.index]?.type === 'date'">
            <div class="flex w-full items-center gap-1 px-2 py-1">
              <span class="text-xs font-medium text-gray-700 dark:text-gray-200">
                {{ (flatList[virtualItem.index] as { label: string }).label }}
              </span>
              <span class="text-xs text-gray-400">
                ({{ (flatList[virtualItem.index] as { count: number }).count }})
              </span>
            </div>
          </template>

          <!-- English UI note -->
          <template v-else-if="flatList[virtualItem.index]?.type === 'session'">
            <button
              class="xeno-timeline-session flex w-full flex-col rounded px-2 py-1 pl-4 text-left transition-colors"
              :class="[
                activeSessionId === (flatList[virtualItem.index] as { session: ChatSessionItem }).session.id
                  ? 'xeno-timeline-session-active text-blue-700 dark:text-blue-300'
                  : 'hover:bg-gray-100/70 dark:hover:bg-gray-700/50',
              ]"
              @click="handleSelectSession((flatList[virtualItem.index] as { session: ChatSessionItem }).session)"
            >
              <!-- English UI note -->
              <div class="flex w-full items-center justify-between">
                <span class="text-xs text-gray-600 dark:text-gray-300">
                  {{ formatTime((flatList[virtualItem.index] as { session: ChatSessionItem }).session.startTs) }}
                </span>
                <span class="text-xs text-gray-400">
                  ({{ (flatList[virtualItem.index] as { session: ChatSessionItem }).session.messageCount }})
                </span>
              </div>

              <!-- English UI note -->
              <div class="mt-0.5 flex w-full items-center">
                <!-- English UI note -->
                <UTooltip
                  v-if="(flatList[virtualItem.index] as { session: ChatSessionItem }).session.summary"
                  :popper="{ placement: 'right' }"
                  :ui="{ content: 'z-[10001] h-auto max-h-80 overflow-y-auto' }"
                >
                  <span class="line-clamp-2 text-xs leading-tight text-gray-400 dark:text-gray-500">
                    {{ (flatList[virtualItem.index] as { session: ChatSessionItem }).session.summary }}
                  </span>
                  <template #content>
                    <div class="max-w-sm whitespace-pre-wrap break-words text-sm leading-relaxed">
                      {{ (flatList[virtualItem.index] as { session: ChatSessionItem }).session.summary }}
                    </div>
                  </template>
                </UTooltip>

                <!-- English UI note -->
                <span
                  v-else-if="(flatList[virtualItem.index] as { session: ChatSessionItem }).session.messageCount >= 3"
                  class="flex items-center gap-1 text-xs text-gray-400 hover:text-blue-500 dark:text-gray-500 dark:hover:text-blue-400"
                  @click="
                    generateSummary((flatList[virtualItem.index] as { session: ChatSessionItem }).session, $event)
                  "
                >
                  <UIcon
                    v-if="isGenerating((flatList[virtualItem.index] as { session: ChatSessionItem }).session.id)"
                    name="i-heroicons-arrow-path"
                    class="h-3 w-3 animate-spin"
                  />
                  <UIcon v-else name="i-heroicons-sparkles" class="h-3 w-3" />
                  <span>{{ t('records.timeline.generateSummary') }}</span>
                </span>

                <!-- English UI note -->
                <span v-else class="text-xs italic text-gray-300 dark:text-gray-600">
                  {{ t('records.timeline.tooFewMessages') }}
                </span>
              </div>
            </button>
          </template>
        </div>
      </div>
    </div>
  </div>

  <!-- English UI note -->
  <BatchSummaryModal v-model:open="showBatchSummaryModal" :session-id="sessionId" @completed="loadSessions" />
</template>

<style scoped>
.vertical-text {
  writing-mode: vertical-rl;
  text-orientation: mixed;
}

.xeno-timeline-collapsed,
.xeno-timeline-shell {
  border-right: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.5);
}

.xeno-timeline-header {
  border-bottom: 1px solid rgba(139, 166, 189, 0.14);
}

.xeno-timeline-scroll {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent 120%),
    rgba(5, 13, 21, 0.08);
}

.xeno-timeline-empty {
  border: 1px solid rgba(139, 166, 189, 0.12);
  border-radius: 0.9rem;
  padding: 0.8rem;
  background: rgba(7, 17, 26, 0.38);
}

.xeno-timeline-session {
  border: 1px solid transparent;
}

.xeno-timeline-session-active {
  border-color: rgba(116, 220, 255, 0.18);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(12, 38, 60, 0.58);
}
</style>
