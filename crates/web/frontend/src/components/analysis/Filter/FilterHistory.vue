<script setup lang="ts">
/**
 * English note.
 * English note.
 */

import { ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSessionStore } from '@/stores/session'

const { t } = useI18n()
const sessionStore = useSessionStore()

// Props
const open = defineModel<boolean>('open', { default: false })

// Emits
const emit = defineEmits<{
  load: [
    condition: {
      mode: 'condition' | 'session'
      conditionFilter?: {
        keywords: string[]
        timeRange: { start: number; end: number } | null
        senderIds: number[]
        contextSize: number
      }
      selectedSessionIds?: number[]
    },
  ]
}>()

// English engineering note.
interface FilterHistoryItem {
  id: string
  sessionId: string
  createdAt: number
  name: string
  mode: 'condition' | 'session'
  conditionFilter?: {
    keywords: string[]
    timeRange: { start: number; end: number } | null
    senderIds: number[]
    contextSize: number
  }
  selectedSessionIds?: number[]
}

// English engineering note.
const historyList = ref<FilterHistoryItem[]>([])

// English engineering note.
const editingId = ref<string | null>(null)
const editingName = ref('')

// English engineering note.
const STORAGE_KEY = 'xenobot_filter_history'

// English engineering note.
function loadHistory() {
  try {
    const data = localStorage.getItem(STORAGE_KEY)
    if (data) {
      const allHistory: FilterHistoryItem[] = JSON.parse(data)
      // English engineering note.
      historyList.value = allHistory.filter((h) => h.sessionId === sessionStore.currentSessionId)
    }
  } catch (error) {
    console.error('加载历史记录失败:', error)
  }
}

// English engineering note.
function saveHistory() {
  try {
    const data = localStorage.getItem(STORAGE_KEY)
    let allHistory: FilterHistoryItem[] = data ? JSON.parse(data) : []

    // English engineering note.
    allHistory = allHistory.filter((h) => h.sessionId !== sessionStore.currentSessionId)
    allHistory = [...allHistory, ...historyList.value]

    localStorage.setItem(STORAGE_KEY, JSON.stringify(allHistory))
  } catch (error) {
    console.error('保存历史记录失败:', error)
  }
}

// English engineering note.
function deleteHistory(id: string) {
  historyList.value = historyList.value.filter((h) => h.id !== id)
  saveHistory()
}

// English engineering note.
function loadCondition(item: FilterHistoryItem) {
  emit('load', {
    mode: item.mode,
    conditionFilter: item.conditionFilter,
    selectedSessionIds: item.selectedSessionIds,
  })
}

// English engineering note.
function startEdit(item: FilterHistoryItem) {
  editingId.value = item.id
  editingName.value = item.name
}

// English engineering note.
function saveName(item: FilterHistoryItem) {
  item.name = editingName.value || item.name
  editingId.value = null
  saveHistory()
}

// English engineering note.
function cancelEdit() {
  editingId.value = null
}

// English engineering note.
function formatTime(ts: number): string {
  return new Date(ts).toLocaleString()
}

// English engineering note.
function formatSummary(item: FilterHistoryItem): string {
  if (item.mode === 'condition') {
    const parts: string[] = []
    if (item.conditionFilter?.keywords.length) {
      parts.push(`关键词: ${item.conditionFilter.keywords.join(', ')}`)
    }
    if (item.conditionFilter?.senderIds.length) {
      parts.push(`${item.conditionFilter.senderIds.length} 位成员`)
    }
    return parts.join(' | ') || '无条件'
  } else {
    return `${item.selectedSessionIds?.length || 0} 个会话`
  }
}

// English engineering note.
watch(open, (val) => {
  if (val) {
    loadHistory()
  }
})

onMounted(() => {
  loadHistory()
})

// English engineering note.
defineExpose({
  saveCondition(condition: Omit<FilterHistoryItem, 'id' | 'sessionId' | 'createdAt' | 'name'>) {
    const newItem: FilterHistoryItem = {
      id: `filter_${Date.now()}`,
      sessionId: sessionStore.currentSessionId || '',
      createdAt: Date.now(),
      name: `筛选 ${historyList.value.length + 1}`,
      ...condition,
    }
    historyList.value.unshift(newItem)
    // English engineering note.
    historyList.value = historyList.value.slice(0, 20)
    saveHistory()
  },
})
</script>

<template>
  <UModal v-model:open="open">
    <template #content>
      <UCard>
        <template #header>
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold">{{ t('analysis.filter.historyTitle') }}</h3>
            <UButton variant="ghost" icon="i-heroicons-x-mark" size="sm" @click="open = false" />
          </div>
        </template>

        <div class="max-h-96 overflow-y-auto">
          <div v-if="historyList.length === 0" class="py-8 text-center text-gray-500">
            {{ t('analysis.filter.noHistory') }}
          </div>

          <div v-else class="divide-y divide-gray-200 dark:divide-gray-700">
            <div
              v-for="item in historyList"
              :key="item.id"
              class="py-3 hover:bg-gray-50 dark:hover:bg-gray-800 px-2 rounded-md"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="flex-1 min-w-0">
                  <!-- English UI note -->
                  <div v-if="editingId === item.id" class="flex items-center gap-2 mb-1">
                    <UInput v-model="editingName" size="sm" class="flex-1" @keydown.enter="saveName(item)" />
                    <UButton size="xs" @click="saveName(item)">{{ t('common.save') }}</UButton>
                    <UButton size="xs" variant="ghost" @click="cancelEdit">{{ t('common.cancel') }}</UButton>
                  </div>
                  <div v-else class="flex items-center gap-2 mb-1">
                    <span class="font-medium text-gray-900 dark:text-white">{{ item.name }}</span>
                    <UBadge :color="item.mode === 'condition' ? 'primary' : 'green'" size="xs">
                      {{ item.mode === 'condition' ? '条件' : '会话' }}
                    </UBadge>
                    <button class="text-gray-400 hover:text-gray-600" @click="startEdit(item)">
                      <UIcon name="i-heroicons-pencil" class="w-3 h-3" />
                    </button>
                  </div>

                  <!-- English UI note -->
                  <p class="text-sm text-gray-600 dark:text-gray-400 truncate">
                    {{ formatSummary(item) }}
                  </p>

                  <!-- English UI note -->
                  <p class="text-xs text-gray-400 mt-1">
                    {{ formatTime(item.createdAt) }}
                  </p>
                </div>

                <!-- English UI note -->
                <div class="flex items-center gap-1">
                  <UButton size="xs" variant="soft" @click="loadCondition(item)">
                    {{ t('common.load') }}
                  </UButton>
                  <UButton size="xs" variant="ghost" color="red" @click="deleteHistory(item.id)">
                    <UIcon name="i-heroicons-trash" class="w-4 h-4" />
                  </UButton>
                </div>
              </div>
            </div>
          </div>
        </div>
      </UCard>
    </template>
  </UModal>
</template>
