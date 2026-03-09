<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSessionStore } from '@/stores/session'
import { useVirtualizer } from '@tanstack/vue-virtual'

const { t, locale } = useI18n()
const sessionStore = useSessionStore()

const selectedIds = defineModel<number[]>('selectedIds', { default: () => [] })

const selectedSet = ref<Set<number>>(new Set())

watch(
  selectedIds,
  (newIds) => {
    selectedSet.value = new Set(newIds)
  },
  { immediate: true }
)

interface ChatSession {
  id: number
  startTs: number
  endTs: number
  messageCount: number
  firstMessageId: number
}

const sessions = ref<ChatSession[]>([])
const isLoading = ref(false)

const scrollContainerRef = ref<HTMLElement | null>(null)

interface ListItem {
  type: 'date' | 'session'
  date?: string
  session?: ChatSession
  sessionCount?: number
}

const flatItems = computed<ListItem[]>(() => {
  const groups: Record<string, ChatSession[]> = {}

  for (const session of sessions.value) {
    const date = new Date(session.startTs * 1000).toLocaleDateString(locale.value)
    if (!groups[date]) {
      groups[date] = []
    }
    groups[date].push(session)
  }

  const sortedDates = Object.keys(groups).sort((a, b) => {
    return new Date(b).getTime() - new Date(a).getTime()
  })

  const items: ListItem[] = []
  for (const date of sortedDates) {
    items.push({ type: 'date', date, sessionCount: groups[date].length })
    for (const session of groups[date]) {
      items.push({ type: 'session', session, date })
    }
  }

  return items
})

const virtualizer = useVirtualizer(
  computed(() => ({
    count: flatItems.value.length,
    getScrollElement: () => scrollContainerRef.value,
    estimateSize: (index) => (flatItems.value[index]?.type === 'date' ? 36 : 56),
    overscan: 10,
  }))
)

const virtualItems = computed(() => virtualizer.value.getVirtualItems())

async function loadSessions() {
  const sessionId = sessionStore.currentSessionId
  if (!sessionId) return

  isLoading.value = true
  try {
    sessions.value = await window.sessionApi.getSessions(sessionId)
  } catch (error) {
    console.error('[SessionPanel] Failed to load sessions:', error)
  } finally {
    isLoading.value = false
  }
}

function isSelected(id: number): boolean {
  return selectedSet.value.has(id)
}

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

function getSessionIdsForDate(date: string): number[] {
  return flatItems.value.filter((item) => item.type === 'session' && item.date === date).map((item) => item.session!.id)
}

function selectDate(date: string) {
  const sessionIds = getSessionIdsForDate(date)
  const allSelected = sessionIds.every((id) => selectedSet.value.has(id))

  const newSet = new Set(selectedSet.value)
  if (allSelected) {
    for (const id of sessionIds) {
      newSet.delete(id)
    }
  } else {
    for (const id of sessionIds) {
      newSet.add(id)
    }
  }
  selectedSet.value = newSet
  selectedIds.value = Array.from(newSet)
}

function isDateFullySelected(date: string): boolean {
  const sessionIds = getSessionIdsForDate(date)
  return sessionIds.length > 0 && sessionIds.every((id) => selectedSet.value.has(id))
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString(locale.value, {
    hour: '2-digit',
    minute: '2-digit',
  })
}

function formatDuration(startTs: number, endTs: number): string {
  const diff = endTs - startTs
  if (diff < 60) return t('analysis.filter.durationSeconds', { count: diff })
  if (diff < 3600) return t('analysis.filter.durationMinutes', { count: Math.floor(diff / 60) })
  return t('analysis.filter.durationHoursMinutes', {
    hours: Math.floor(diff / 3600),
    minutes: Math.floor((diff % 3600) / 60),
  })
}

onMounted(() => {
  loadSessions()
})
</script>

<template>
  <div class="xeno-session-shell flex h-full min-h-0 flex-col p-4">
    <div v-if="selectedIds.length > 0" class="flex-none mb-2 text-sm text-primary-500">
      {{ t('analysis.filter.selectedSessions', { count: selectedIds.length }) }}
    </div>

    <div class="flex-1 min-h-0">
      <div v-if="isLoading" class="flex items-center justify-center h-full">
        <UIcon name="i-heroicons-arrow-path" class="w-6 h-6 animate-spin text-gray-400" />
      </div>

      <div v-else-if="sessions.length === 0" class="xeno-session-empty flex items-center justify-center h-full text-gray-500 text-sm">
        {{ t('analysis.filter.noSessions') }}
      </div>

      <div v-else ref="scrollContainerRef" class="xeno-session-scroll h-full overflow-y-auto rounded-2xl">
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
            <div
              v-if="flatItems[virtualRow.index].type === 'date'"
              class="xeno-session-date flex cursor-pointer items-center justify-between rounded-xl px-3 py-2"
              @click="selectDate(flatItems[virtualRow.index].date!)"
            >
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                {{ flatItems[virtualRow.index].date }}
              </span>
              <div class="flex items-center gap-2">
                <span class="text-xs text-gray-500">
                  {{ t('analysis.filter.historySessionCount', { count: flatItems[virtualRow.index].sessionCount }) }}
                </span>
                <input
                  type="checkbox"
                  :checked="isDateFullySelected(flatItems[virtualRow.index].date!)"
                  class="text-primary-500 rounded"
                  @click.stop
                  @change="selectDate(flatItems[virtualRow.index].date!)"
                />
              </div>
            </div>

            <label
              v-else
              class="xeno-session-item ml-2 flex cursor-pointer items-center gap-3 rounded-xl border border-transparent px-3 py-2"
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
                  <span>{{ t('analysis.filter.sessionMessageCount', { count: flatItems[virtualRow.index].session!.messageCount }) }}</span>
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

<style scoped>
.xeno-session-shell {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.01), transparent 120%);
}

.xeno-session-scroll {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(7, 16, 24, 0.52);
}

.xeno-session-empty {
  border: 1px dashed rgba(139, 166, 189, 0.16);
  border-radius: 1.2rem;
  background: rgba(8, 18, 28, 0.44);
}

.xeno-session-date {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 120%),
    rgba(12, 24, 34, 0.78);
  transition: background-color 140ms ease;
}

.xeno-session-date:hover {
  background:
    radial-gradient(circle at top left, rgba(84, 214, 255, 0.08), transparent 30%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 120%),
    rgba(14, 28, 38, 0.84);
}

.xeno-session-item {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(9, 18, 28, 0.6);
  transition:
    background-color 140ms ease,
    border-color 140ms ease,
    transform 140ms ease;
}

.xeno-session-item:hover {
  transform: translateY(-1px);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 120%),
    rgba(12, 24, 34, 0.76);
}
</style>
