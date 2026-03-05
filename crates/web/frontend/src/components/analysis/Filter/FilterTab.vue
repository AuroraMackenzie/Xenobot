<script setup lang="ts">
/**
 * English note.
 * English note.
 *
 * English note.
 * English note.
 * English note.
 *
 * English note.
 */

import { ref, computed, watch, toRaw } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from '@nuxt/ui/runtime/composables/useToast.js'
import { useSessionStore } from '@/stores/session'
import ConditionPanel from './ConditionPanel.vue'
import SessionPanel from './SessionPanel.vue'
import PreviewPanel from './PreviewPanel.vue'
import FilterHistory from './FilterHistory.vue'
import LocalAnalysisModal from './LocalAnalysisModal.vue'

const { t } = useI18n()
const toast = useToast()
const sessionStore = useSessionStore()

// English engineering note.
const filterMode = ref<'condition' | 'session'>('condition')

// English engineering note.
const conditionFilter = ref<{
  keywords: string[]
  timeRange: { start: number; end: number } | null
  senderIds: number[]
  contextSize: number
}>({
  keywords: [],
  timeRange: null,
  senderIds: [],
  contextSize: 10,
})

// English engineering note.
const selectedSessionIds = ref<number[]>([])

// English engineering note.
interface FilterMessage {
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
}

// English engineering note.
interface PaginationInfo {
  page: number
  pageSize: number
  totalBlocks: number
  totalHits: number
  hasMore: boolean
}

// English engineering note.
const filterResult = ref<{
  blocks: Array<{
    startTs: number
    endTs: number
    messages: FilterMessage[]
    hitCount: number
  }>
  stats: {
    totalMessages: number
    hitMessages: number
    totalChars: number
  }
  pagination: PaginationInfo
} | null>(null)

// English engineering note.
const isFiltering = ref(false)
const isLoadingMore = ref(false)
const showHistory = ref(false)
const showAnalysisModal = ref(false)

// English engineering note.
const PAGE_SIZE = 50

// English engineering note.
// English engineering note.
// English engineering note.
const estimatedTokens = computed(() => {
  if (!filterResult.value) return 0
  return Math.ceil(filterResult.value.stats.totalChars * 1.5)
})

// English engineering note.
// English engineering note.
const tokenStatus = computed(() => {
  const tokens = estimatedTokens.value
  if (tokens < 50000) return 'green'
  if (tokens < 100000) return 'yellow'
  return 'red'
})

// English engineering note.
const canExecuteFilter = computed(() => {
  if (isFiltering.value) return false

  if (filterMode.value === 'condition') {
    // English engineering note.
    return (
      conditionFilter.value.keywords.length > 0 ||
      conditionFilter.value.senderIds.length > 0 ||
      conditionFilter.value.timeRange !== null
    )
  } else {
    // English engineering note.
    return selectedSessionIds.value.length > 0
  }
})

// English engineering note.
async function executeFilter() {
  const sessionId = sessionStore.currentSessionId
  if (!sessionId) return

  isFiltering.value = true
  filterResult.value = null

  try {
    if (filterMode.value === 'condition') {
      // English engineering note.
      const rawFilter = toRaw(conditionFilter.value)
      const keywords = rawFilter.keywords.length > 0 ? [...rawFilter.keywords] : undefined
      const timeFilter = rawFilter.timeRange
        ? { startTs: rawFilter.timeRange.start, endTs: rawFilter.timeRange.end }
        : undefined
      const senderIds = rawFilter.senderIds.length > 0 ? [...rawFilter.senderIds] : undefined
      const contextSize = rawFilter.contextSize

      const result = await window.aiApi.filterMessagesWithContext(
        sessionId,
        keywords,
        timeFilter,
        senderIds,
        contextSize,
        1, // English engineering note.
        PAGE_SIZE
      )
      filterResult.value = result
    } else {
      // English engineering note.
      if (selectedSessionIds.value.length === 0) return
      const sessionIds = [...toRaw(selectedSessionIds.value)]
      const result = await window.aiApi.getMultipleSessionsMessages(sessionId, sessionIds, 1, PAGE_SIZE)
      filterResult.value = result
    }
  } catch (error) {
    console.error('筛选失败:', error)
  } finally {
    isFiltering.value = false
  }
}

// English engineering note.
async function loadMoreBlocks() {
  const sessionId = sessionStore.currentSessionId
  if (!sessionId || !filterResult.value || !filterResult.value.pagination.hasMore || isLoadingMore.value) return

  isLoadingMore.value = true
  const nextPage = filterResult.value.pagination.page + 1

  try {
    let result
    if (filterMode.value === 'condition') {
      const rawFilter = toRaw(conditionFilter.value)
      const keywords = rawFilter.keywords.length > 0 ? [...rawFilter.keywords] : undefined
      const timeFilter = rawFilter.timeRange
        ? { startTs: rawFilter.timeRange.start, endTs: rawFilter.timeRange.end }
        : undefined
      const senderIds = rawFilter.senderIds.length > 0 ? [...rawFilter.senderIds] : undefined
      const contextSize = rawFilter.contextSize

      result = await window.aiApi.filterMessagesWithContext(
        sessionId,
        keywords,
        timeFilter,
        senderIds,
        contextSize,
        nextPage,
        PAGE_SIZE
      )
    } else {
      const sessionIds = [...toRaw(selectedSessionIds.value)]
      result = await window.aiApi.getMultipleSessionsMessages(sessionId, sessionIds, nextPage, PAGE_SIZE)
    }

    // English engineering note.
    if (result && result.blocks.length > 0) {
      filterResult.value = {
        blocks: [...filterResult.value.blocks, ...result.blocks],
        stats: filterResult.value.stats, // English engineering note.
        pagination: result.pagination,
      }
    }
  } catch (error) {
    console.error('加载更多失败:', error)
  } finally {
    isLoadingMore.value = false
  }
}

// English engineering note.
const isExporting = ref(false)
const exportProgress = ref<{
  percentage: number
  message: string
} | null>(null)

// English engineering note.
let unsubscribeExportProgress: (() => void) | null = null

function startExportProgressListener() {
  unsubscribeExportProgress = window.aiApi.onExportProgress((progress) => {
    exportProgress.value = {
      percentage: progress.percentage,
      message: progress.message,
    }
    // English engineering note.
    if (progress.stage === 'done' || progress.stage === 'error') {
      exportProgress.value = null
    }
  })
}

function stopExportProgressListener() {
  if (unsubscribeExportProgress) {
    unsubscribeExportProgress()
    unsubscribeExportProgress = null
  }
  exportProgress.value = null
}

// English engineering note.
async function exportFeedPack() {
  if (!filterResult.value || filterResult.value.blocks.length === 0) return

  const sessionId = sessionStore.currentSessionId
  if (!sessionId) return

  const sessionInfo = sessionStore.currentSession
  const sessionName = sessionInfo?.name || '未知会话'

  // English engineering note.
  const dialogResult = await window.api.dialog.showOpenDialog({
    title: '选择保存目录',
    properties: ['openDirectory', 'createDirectory'],
  })
  if (dialogResult.canceled || !dialogResult.filePaths[0]) return
  const outputDir = dialogResult.filePaths[0]

  isExporting.value = true
  exportProgress.value = { percentage: 0, message: t('analysis.filter.exportPreparing') }

  // English engineering note.
  startExportProgressListener()

  try {
    // English engineering note.
    const rawFilter = toRaw(conditionFilter.value)
    const exportParams = {
      sessionId,
      sessionName,
      outputDir,
      filterMode: filterMode.value,
      keywords: rawFilter.keywords.length > 0 ? [...rawFilter.keywords] : undefined,
      timeFilter: rawFilter.timeRange
        ? { startTs: rawFilter.timeRange.start, endTs: rawFilter.timeRange.end }
        : undefined,
      senderIds: rawFilter.senderIds.length > 0 ? [...rawFilter.senderIds] : undefined,
      contextSize: rawFilter.contextSize,
      chatSessionIds: filterMode.value === 'session' ? [...toRaw(selectedSessionIds.value)] : undefined,
    }

    // English engineering note.
    const exportResult = await window.aiApi.exportFilterResultToFile(exportParams)

    if (exportResult.success && exportResult.filePath) {
      // English engineering note.
      toast.add({
        title: t('analysis.filter.exportSuccess'),
        description: exportResult.filePath,
        color: 'green',
        icon: 'i-heroicons-check-circle',
      })
    } else {
      // English engineering note.
      toast.add({
        title: t('analysis.filter.exportFailed'),
        description: exportResult.error || t('common.error.unknown'),
        color: 'red',
        icon: 'i-heroicons-x-circle',
      })
    }
  } catch (error) {
    console.error('导出失败:', error)
    toast.add({
      title: t('analysis.filter.exportFailed'),
      description: String(error),
      color: 'red',
      icon: 'i-heroicons-x-circle',
    })
  } finally {
    stopExportProgressListener()
    isExporting.value = false
  }
}

// English engineering note.
function openLocalAnalysis() {
  if (!filterResult.value || filterResult.value.blocks.length === 0) return
  showAnalysisModal.value = true
}

// English engineering note.
watch(filterMode, () => {
  filterResult.value = null
})

// English engineering note.
function loadHistoryCondition(condition: {
  mode: 'condition' | 'session'
  conditionFilter?: typeof conditionFilter.value
  selectedSessionIds?: number[]
}) {
  filterMode.value = condition.mode
  if (condition.mode === 'condition' && condition.conditionFilter) {
    conditionFilter.value = { ...condition.conditionFilter }
  } else if (condition.mode === 'session' && condition.selectedSessionIds) {
    selectedSessionIds.value = [...condition.selectedSessionIds]
  }
  showHistory.value = false
}
</script>

<template>
  <div class="xeno-filter-shell h-full flex flex-col">
    <!-- English UI note -->
    <div class="xeno-filter-header flex-none flex items-center justify-between px-4 py-3">
      <div class="flex items-center gap-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{{ t('analysis.filter.title') }}</h2>

        <!-- English UI note -->
        <div class="xeno-filter-mode flex items-center gap-1 p-1 rounded-lg">
          <button
            class="px-3 py-1.5 text-sm font-medium rounded-md transition-colors"
            :class="
              filterMode === 'condition'
                ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
            "
            @click="filterMode = 'condition'"
          >
            {{ t('analysis.filter.conditionMode') }}
          </button>
          <button
            class="px-3 py-1.5 text-sm font-medium rounded-md transition-colors"
            :class="
              filterMode === 'session'
                ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
            "
            @click="filterMode = 'session'"
          >
            {{ t('analysis.filter.sessionMode') }}
          </button>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <!-- English UI note -->
        <UButton variant="ghost" icon="i-heroicons-clock" size="sm" @click="showHistory = true">
          {{ t('analysis.filter.history') }}
        </UButton>
      </div>
    </div>

    <!-- English UI note -->
    <div class="flex-1 flex overflow-hidden">
      <!-- English UI note -->
      <div class="xeno-filter-left w-80 flex-none flex flex-col">
        <!-- English UI note -->
        <div class="flex-1 min-h-0 overflow-y-auto">
          <Transition name="xeno-filter-panel" mode="out-in">
            <ConditionPanel
              v-if="filterMode === 'condition'"
              v-model:keywords="conditionFilter.keywords"
              v-model:time-range="conditionFilter.timeRange"
              v-model:sender-ids="conditionFilter.senderIds"
              v-model:context-size="conditionFilter.contextSize"
            />
            <SessionPanel v-else v-model:selected-ids="selectedSessionIds" />
          </Transition>
        </div>

        <!-- English UI note -->
        <div class="xeno-filter-left-footer flex-none p-4">
          <UButton block color="primary" :loading="isFiltering" :disabled="!canExecuteFilter" @click="executeFilter">
            {{ t('analysis.filter.execute') }}
          </UButton>
        </div>
      </div>

      <!-- English UI note -->
      <div class="flex-1 flex flex-col overflow-hidden">
        <PreviewPanel
          :result="filterResult"
          :is-loading="isFiltering"
          :is-loading-more="isLoadingMore"
          :estimated-tokens="estimatedTokens"
          :token-status="tokenStatus"
          @load-more="loadMoreBlocks"
        />

        <!-- English UI note -->
        <Transition name="xeno-filter-action">
          <div
            v-if="filterResult && filterResult.blocks.length > 0"
            class="xeno-filter-actions flex-none flex flex-col gap-2 px-4 py-3"
          >
            <!-- English UI note -->
            <div v-if="isExporting && exportProgress" class="w-full">
              <div class="mb-1 flex items-center justify-between text-sm text-gray-600 dark:text-gray-400">
                <span>{{ exportProgress.message }}</span>
                <span>{{ exportProgress.percentage }}%</span>
              </div>
              <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  class="h-full bg-primary-500 transition-all duration-300"
                  :style="{ width: `${exportProgress.percentage}%` }"
                />
              </div>
            </div>
            <!-- English UI note -->
            <div class="flex items-center justify-end gap-3">
              <UButton
                variant="outline"
                icon="i-heroicons-document-arrow-down"
                :loading="isExporting"
                :disabled="isExporting"
                @click="exportFeedPack"
              >
                {{ t('analysis.filter.export') }}
              </UButton>
              <UButton color="primary" icon="i-heroicons-sparkles" @click="openLocalAnalysis">
                {{ t('analysis.filter.localAnalysis') }}
              </UButton>
            </div>
          </div>
        </Transition>
      </div>
    </div>

    <!-- English UI note -->
    <FilterHistory v-model:open="showHistory" @load="loadHistoryCondition" />

    <!-- English UI note -->
    <LocalAnalysisModal v-model:open="showAnalysisModal" :filter-result="filterResult" :filter-mode="filterMode" />
  </div>
</template>

<style scoped>
.xeno-filter-shell {
  background: linear-gradient(180deg, transparent, var(--xeno-surface-muted));
}

.xeno-filter-header {
  border-bottom: 1px solid var(--xeno-border-soft);
  background: var(--xeno-surface-muted);
  backdrop-filter: blur(12px) saturate(126%);
}

.xeno-filter-mode {
  border: 1px solid var(--xeno-border-soft);
  background: var(--xeno-surface-main);
}

.xeno-filter-left {
  border-right: 1px solid var(--xeno-border-soft);
}

.xeno-filter-left-footer {
  border-top: 1px solid var(--xeno-border-soft);
  background: var(--xeno-surface-main);
}

.xeno-filter-actions {
  border-top: 1px solid var(--xeno-border-soft);
  background: var(--xeno-surface-muted);
  backdrop-filter: blur(12px) saturate(125%);
}

.xeno-filter-panel-enter-active,
.xeno-filter-panel-leave-active {
  transition:
    opacity 0.24s cubic-bezier(0.22, 0.92, 0.3, 1),
    transform 0.24s cubic-bezier(0.22, 0.92, 0.3, 1),
    filter 0.24s cubic-bezier(0.22, 0.92, 0.3, 1);
}

.xeno-filter-panel-enter-from,
.xeno-filter-panel-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.996);
  filter: blur(5px);
}

.xeno-filter-action-enter-active,
.xeno-filter-action-leave-active {
  transition:
    opacity 0.22s cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.xeno-filter-action-enter-from,
.xeno-filter-action-leave-to {
  opacity: 0;
  transform: translateY(8px);
}

@media (prefers-reduced-motion: reduce) {
  .xeno-filter-panel-enter-active,
  .xeno-filter-panel-leave-active,
  .xeno-filter-action-enter-active,
  .xeno-filter-action-leave-active {
    transition-duration: 0.01ms !important;
  }

  .xeno-filter-panel-enter-from,
  .xeno-filter-panel-leave-to,
  .xeno-filter-action-enter-from,
  .xeno-filter-action-leave-to {
    opacity: 1;
    transform: none;
    filter: none;
  }
}
</style>
