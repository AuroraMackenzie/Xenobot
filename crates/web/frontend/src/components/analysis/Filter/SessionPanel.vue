<script setup lang="ts">
/**
 * English note.
 * English note.
 * English note.
 */

import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSessionStore } from '@/stores/session'
import { useVirtualizer } from '@tanstack/vue-virtual'

const { t } = useI18n()
const sessionStore = useSessionStore()

// English engineering note.
const selectedIds = defineModel<number[]>('selectedIds', { default: () => [] })

// English engineering note.
const selectedSet = ref<Set<number>>(new Set())

// English engineering note.
watch(
  selectedIds,
  (newIds) => {
    selectedSet.value = new Set(newIds)
  },
  { immediate: true }
)

// English engineering note.
interface ChatSession {
  id: number
  startTs: number
  endTs: number
  messageCount: number
  firstMessageId: number
}

const sessions = ref<ChatSession[]>([])
const isLoading = ref(false)

// English engineering note.
const scrollContainerRef = ref<HTMLElement | null>(null)

// English engineering note.
interface ListItem {
  type: 'date' | 'session'
  date?: string
  session?: ChatSession
  sessionCount?: number
}

// English engineering note.
const flatItems = computed<ListItem[]>(() => {
  const groups: Record<string, ChatSession[]> = {}

  // English engineering note.
  for (const session of sessions.value) {
    const date = new Date(session.startTs * 1000).toLocaleDateString()
    if (!groups[date]) {
      groups[date] = []
    }
    groups[date].push(session)
  }

  // English engineering note.
  const sortedDates = Object.keys(groups).sort((a, b) => {
    return new Date(b).getTime() - new Date(a).getTime()
  })

  // English engineering note.
  const items: ListItem[] = []
  for (const date of sortedDates) {
    items.push({ type: 'date', date, sessionCount: groups[date].length })
    for (const session of groups[date]) {
      items.push({ type: 'session', session, date })
    }
  }

  return items
})

// English engineering note.
const virtualizer = useVirtualizer(
  computed(() => ({
    count: flatItems.value.length,
    getScrollElement: () => scrollContainerRef.value,
    estimateSize: (index) => (flatItems.value[index]?.type === 'date' ? 36 : 56),
    overscan: 10,
  }))
)

const virtualItems = computed(() => virtualizer.value.getVirtualItems())

// English engineering note.
async function loadSessions() {
  const sessionId = sessionStore.currentSessionId
  if (!sessionId) return

  isLoading.value = true
  try {
    sessions.value = await window.sessionApi.getSessions(sessionId)
  } catch (error) {
    console.error('加载会话失败:', error)
  } finally {
    isLoading.value = false
  }
}

// English engineering note.
function isSelected(id: number): boolean {
  return selectedSet.value.has(id)
}

// English engineering note.
function toggleSession(id: number) {
  const newSet = new Set(selectedSet.value)
  if (newSet.has(id)) {
    newSet.delete(id)
  } else {
    newSet.add(id)
  }
  selectedSet.value = newSet
  selectedIds.value = Array.from(newSet)
}

// English engineering note.
function getSessionIdsForDate(date: string): number[] {
  return flatItems.value.filter((item) => item.type === 'session' && item.date === date).map((item) => item.session!.id)
}

// English engineering note.
function selectDate(date: string) {
  const sessionIds = getSessionIdsForDate(date)
  const allSelected = sessionIds.every((id) => selectedSet.value.has(id))

  const newSet = new Set(selectedSet.value)
  if (allSelected) {
    // English engineering note.
    for (const id of sessionIds) {
      newSet.delete(id)
    }
  } else {
    // English engineering note.
    for (const id of sessionIds) {
      newSet.add(id)
    }
  }
  selectedSet.value = newSet
  selectedIds.value = Array.from(newSet)
}

// English engineering note.
function isDateFullySelected(date: string): boolean {
  const sessionIds = getSessionIdsForDate(date)
  return sessionIds.length > 0 && sessionIds.every((id) => selectedSet.value.has(id))
}

// English engineering note.
function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  })
}

// English engineering note.
function formatDuration(startTs: number, endTs: number): string {
  const diff = endTs - startTs
  if (diff < 60) return `${diff}秒`
  if (diff < 3600) return `${Math.floor(diff / 60)}分钟`
  return `${Math.floor(diff / 3600)}小时${Math.floor((diff % 3600) / 60)}分钟`
}

onMounted(() => {
  loadSessions()
})
</script>

<template>
  <div class="p-4 flex flex-col h-full min-h-0">
    <!-- English UI note -->
    <div v-if="selectedIds.length > 0" class="flex-none mb-2 text-sm text-primary-500">
      {{ t('analysis.filter.selectedSessions', { count: selectedIds.length }) }}
    </div>

    <!-- English UI note -->
    <div class="flex-1 min-h-0">
      <div v-if="isLoading" class="flex items-center justify-center h-full">
        <UIcon name="i-heroicons-arrow-path" class="w-6 h-6 animate-spin text-gray-400" />
      </div>

      <div v-else-if="sessions.length === 0" class="flex items-center justify-center h-full text-gray-500 text-sm">
        {{ t('analysis.filter.noSessions') }}
      </div>

      <div v-else ref="scrollContainerRef" class="h-full overflow-y-auto">
        <div :style="{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }">
          <div
            v-for="virtualRow in virtualItems"
            :key="String(virtualRow.key)"
            :style="{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              transform: `translateY(${virtualRow.start}px)`,
            }"
          >
            <!-- English UI note -->
            <div
              v-if="flatItems[virtualRow.index].type === 'date'"
              class="flex items-center justify-between px-2 py-1.5 bg-gray-100 dark:bg-gray-800 rounded-md cursor-pointer hover:bg-gray-200 dark:hover:bg-gray-700"
              @click="selectDate(flatItems[virtualRow.index].date!)"
            >
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                {{ flatItems[virtualRow.index].date }}
              </span>
              <div class="flex items-center gap-2">
                <span class="text-xs text-gray-500">{{ flatItems[virtualRow.index].sessionCount }} 个会话</span>
                <input
                  type="checkbox"
                  :checked="isDateFullySelected(flatItems[virtualRow.index].date!)"
                  class="text-primary-500 rounded"
                  @click.stop
                  @change="selectDate(flatItems[virtualRow.index].date!)"
                />
              </div>
            </div>

            <!-- English UI note -->
            <label
              v-else
              class="flex items-center gap-3 px-3 py-2 ml-2 rounded-md hover:bg-gray-50 dark:hover:bg-gray-800 cursor-pointer border border-transparent"
              :class="{
                'border-primary-300 bg-primary-50 dark:border-primary-600 dark:bg-primary-900/20': isSelected(
                  flatItems[virtualRow.index].session!.id
                ),
              }"
            >
              <input
                type="checkbox"
                :checked="isSelected(flatItems[virtualRow.index].session!.id)"
                class="text-primary-500 rounded"
                @change="toggleSession(flatItems[virtualRow.index].session!.id)"
              />
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm text-gray-700 dark:text-gray-300">
                    {{ formatTime(flatItems[virtualRow.index].session!.startTs) }} -
                    {{ formatTime(flatItems[virtualRow.index].session!.endTs) }}
                  </span>
                </div>
                <div class="flex items-center gap-2 text-xs text-gray-500">
                  <span>{{ flatItems[virtualRow.index].session!.messageCount }} 条消息</span>
                  <span>·</span>
                  <span>
                    {{
                      formatDuration(
                        flatItems[virtualRow.index].session!.startTs,
                        flatItems[virtualRow.index].session!.endTs
                      )
                    }}
                  </span>
                </div>
              </div>
            </label>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
