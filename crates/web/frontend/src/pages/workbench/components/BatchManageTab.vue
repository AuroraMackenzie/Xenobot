<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import { useToast } from '@nuxt/ui/runtime/composables/useToast.js'
import { useSessionStore } from '@/stores/session'
import type { AnalysisSession } from '@/types/base'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import 'dayjs/locale/zh-cn'

dayjs.extend(relativeTime)
const toast = useToast()

const { t, locale } = useI18n()
const sessionStore = useSessionStore()
const { sessions } = storeToRefs(sessionStore)

// English engineering note.
const searchQuery = ref('')

// English engineering note.
const filteredSessions = computed(() => {
  if (!searchQuery.value.trim()) {
    return sessions.value
  }
  const query = searchQuery.value.toLowerCase().trim()
  return sessions.value.filter((s) => s.name.toLowerCase().includes(query) || s.platform.toLowerCase().includes(query))
})

// English engineering note.
const selectedIds = ref<Set<string>>(new Set())

// English engineering note.
const showDeleteModal = ref(false)

// English engineering note.
const isDeleting = ref(false)

// English engineering note.
const editingId = ref<string | null>(null)

// English engineering note.
const editingName = ref('')

// English engineering note.
const canMerge = computed(() => {
  if (selectedIds.value.size < 2) return false
  const selectedSessions = sessions.value.filter((s) => selectedIds.value.has(s.id))
  const platforms = new Set(selectedSessions.map((s) => s.platform))
  return platforms.size === 1
})

// English engineering note.
const selectedPlatform = computed(() => {
  if (selectedIds.value.size === 0) return null
  const selectedSessions = sessions.value.filter((s) => selectedIds.value.has(s.id))
  return selectedSessions[0]?.platform || null
})

// English engineering note.
const isAllSelected = computed(() => {
  return filteredSessions.value.length > 0 && filteredSessions.value.every((s) => selectedIds.value.has(s.id))
})

// English engineering note.
const isPartialSelected = computed(() => {
  const selectedInFiltered = filteredSessions.value.filter((s) => selectedIds.value.has(s.id)).length
  return selectedInFiltered > 0 && selectedInFiltered < filteredSessions.value.length
})

// English engineering note.
function toggleSelectAll() {
  if (isAllSelected.value) {
    // English engineering note.
    const filteredIds = new Set(filteredSessions.value.map((s) => s.id))
    selectedIds.value = new Set([...selectedIds.value].filter((id) => !filteredIds.has(id)))
  } else {
    // English engineering note.
    const newSet = new Set(selectedIds.value)
    for (const s of filteredSessions.value) {
      newSet.add(s.id)
    }
    selectedIds.value = newSet
  }
}

// English engineering note.
const lastClickedIndex = ref<number | null>(null)

// English engineering note.
function toggleSelect(id: string) {
  const newSet = new Set(selectedIds.value)
  if (newSet.has(id)) {
    newSet.delete(id)
  } else {
    newSet.add(id)
  }
  selectedIds.value = newSet
}

// English engineering note.
function handleRowClick(index: number, id: string, event: MouseEvent) {
  if (event.shiftKey && lastClickedIndex.value !== null) {
    // English engineering note.
    const start = Math.min(lastClickedIndex.value, index)
    const end = Math.max(lastClickedIndex.value, index)
    const newSet = new Set(selectedIds.value)
    for (let i = start; i <= end; i++) {
      const session = filteredSessions.value[i]
      if (session) {
        newSet.add(session.id)
      }
    }
    selectedIds.value = newSet
  } else {
    // English engineering note.
    toggleSelect(id)
  }
  // English engineering note.
  lastClickedIndex.value = index
}

// English engineering note.
function isSelected(id: string): boolean {
  return selectedIds.value.has(id)
}

// English engineering note.
function formatTime(timestamp: number): string {
  return dayjs
    .unix(timestamp)
    .locale(locale.value === 'zh-CN' ? 'zh-cn' : 'en')
    .fromNow()
}

// English engineering note.
function isPrivateChat(session: AnalysisSession): boolean {
  return session.type === 'private'
}

// English engineering note.
function getSessionAvatar(session: AnalysisSession): string | null {
  if (isPrivateChat(session)) {
    return session.memberAvatar || null
  }
  return session.groupAvatar || null
}

// English engineering note.
function getSessionAvatarText(session: AnalysisSession): string {
  const name = session.name || ''
  if (!name) return '?'
  if (isPrivateChat(session)) {
    return name.length <= 2 ? name : name.slice(-2)
  } else {
    return name.length <= 2 ? name : name.slice(0, 2)
  }
}

// English engineering note.
const PLATFORM_CONFIG: Record<string, { label: string; class: string }> = {
  qq: { label: 'QQ', class: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300' },
  weixin: { label: '微信', class: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300' },
  discord: { label: 'Discord', class: 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300' },
  whatsapp: {
    label: 'WhatsApp',
    class: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300',
  },
  instagram: { label: 'Instagram', class: 'bg-pink-100 text-pink-700 dark:bg-pink-900/30 dark:text-pink-300' },
  line: { label: 'LINE', class: 'bg-lime-100 text-lime-700 dark:bg-lime-900/30 dark:text-lime-300' },
  unknown: { label: '未知', class: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' },
}

// English engineering note.
function getPlatformLabel(platform: string): string {
  return PLATFORM_CONFIG[platform]?.label || platform
}

// English engineering note.
function getPlatformClass(platform: string): string {
  return PLATFORM_CONFIG[platform]?.class || PLATFORM_CONFIG.unknown.class
}

// English engineering note.
function startEdit(session: AnalysisSession, event: Event) {
  event.stopPropagation()
  editingId.value = session.id
  editingName.value = session.name
}

// English engineering note.
async function saveEdit() {
  if (!editingId.value || !editingName.value.trim()) {
    editingId.value = null
    editingName.value = ''
    return
  }

  const newName = editingName.value.trim()
  const session = sessions.value.find((s) => s.id === editingId.value)

  // English engineering note.
  if (session && session.name !== newName) {
    await sessionStore.renameSession(editingId.value, newName)
  }

  editingId.value = null
  editingName.value = ''
}

// English engineering note.
function cancelEdit() {
  editingId.value = null
  editingName.value = ''
}

// English engineering note.
function openDeleteModal() {
  if (selectedIds.value.size === 0) return
  showDeleteModal.value = true
}

// English engineering note.
const showMergeModal = ref(false)
const isMerging = ref(false)
const mergeProgress = ref('')

// English engineering note.
async function handleMerge() {
  if (!canMerge.value) return
  showMergeModal.value = true
}

// English engineering note.
async function executeMerge() {
  if (!canMerge.value) return

  isMerging.value = true
  mergeProgress.value = t('tools.batchManage.mergeSteps.exporting')

  const selectedSessionIds = Array.from(selectedIds.value)
  let tempFiles: string[] = []

  try {
    // English engineering note.
    const exportResult = await window.chatApi.exportSessionsToTempFiles(selectedSessionIds)
    if (!exportResult.success) {
      throw new Error(exportResult.error || '导出失败')
    }
    tempFiles = exportResult.tempFiles

    // English engineering note.
    mergeProgress.value = t('tools.batchManage.mergeSteps.parsing')
    for (const filePath of tempFiles) {
      await window.mergeApi.parseFileInfo(filePath)
    }

    // English engineering note.
    mergeProgress.value = t('tools.batchManage.mergeSteps.checking')
    const conflictResult = await window.mergeApi.checkConflicts(tempFiles)

    if (conflictResult.conflicts.length > 0) {
      // English engineering note.
      // English engineering note.
      console.log(`[BatchDelete] 检测到 ${conflictResult.conflicts.length} 个冲突，使用默认解决方案`)
    }

    // English engineering note.
    mergeProgress.value = t('tools.batchManage.mergeSteps.merging')
    const firstSession = sessions.value.find((s) => selectedIds.value.has(s.id))
    const baseName = firstSession?.name || '聊天记录'
    const mergedName = `${baseName}（${t('tools.batchManage.mergedSuffix')}）`
    const mergeResult = await window.mergeApi.mergeFiles({
      filePaths: tempFiles,
      outputName: mergedName,
      outputFormat: 'json',
      conflictResolutions: conflictResult.conflicts.map((c) => ({
        id: c.id,
        resolution: 'keep1' as const,
      })),
      andAnalyze: true, // English engineering note.
    })

    if (!mergeResult.success) {
      throw new Error(mergeResult.error || '合并失败')
    }

    // English engineering note.
    mergeProgress.value = t('tools.batchManage.mergeSteps.cleaning')
    for (const sessionId of selectedSessionIds) {
      await sessionStore.deleteSession(sessionId)
    }

    // English engineering note.
    await window.chatApi.cleanupTempExportFiles(tempFiles)

    // English engineering note.
    await sessionStore.loadSessions()

    // English engineering note.
    selectedIds.value = new Set()
    showMergeModal.value = false

    // English engineering note.
    toast.add({
      title: t('tools.batchManage.mergeSuccess', { count: selectedSessionIds.length }),
      icon: 'i-heroicons-check-circle',
      color: 'success',
    })
  } catch (error) {
    console.error('[BatchDelete] 合并失败:', error)
    toast.add({
      title: t('tools.batchManage.mergeError', { error: String(error) }),
      icon: 'i-heroicons-exclamation-circle',
      color: 'error',
    })

    // English engineering note.
    if (tempFiles.length > 0) {
      await window.chatApi.cleanupTempExportFiles(tempFiles)
    }
  } finally {
    isMerging.value = false
    mergeProgress.value = ''
  }
}

// English engineering note.
async function confirmBatchDelete() {
  if (selectedIds.value.size === 0) return

  isDeleting.value = true
  try {
    const idsToDelete = Array.from(selectedIds.value)

    // English engineering note.
    for (const id of idsToDelete) {
      await sessionStore.deleteSession(id)
    }

    // English engineering note.
    selectedIds.value = new Set()
    showDeleteModal.value = false
  } catch (error) {
    console.error('Batch delete failed:', error)
  } finally {
    isDeleting.value = false
  }
}

// English engineering note.
function closeDeleteModal() {
  showDeleteModal.value = false
}

// English engineering note.
onMounted(() => {
  sessionStore.loadSessions()
})
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- English UI note -->
    <div class="mb-4">
      <UInput
        v-model="searchQuery"
        :placeholder="t('tools.batchManage.searchPlaceholder')"
        icon="i-heroicons-magnifying-glass"
        size="md"
        class="max-w-md"
      />
    </div>

    <!-- English UI note -->
    <div class="mb-4 flex items-center justify-between">
      <div class="flex items-center gap-4">
        <!-- English UI note -->
        <UCheckbox
          :model-value="isAllSelected"
          :indeterminate="isPartialSelected"
          :label="t('tools.batchManage.selectAll')"
          @update:model-value="toggleSelectAll"
        />

        <!-- English UI note -->
        <span class="text-xs text-gray-400 dark:text-gray-500">
          {{ t('tools.batchManage.shiftClickHint') }}
        </span>

        <!-- English UI note -->
        <span v-if="selectedIds.size > 0" class="text-sm text-gray-500 dark:text-gray-400">
          {{ t('tools.batchManage.selected', { count: selectedIds.size }) }}
        </span>

        <!-- English UI note -->
        <span
          v-if="searchQuery.trim() && filteredSessions.length !== sessions.length"
          class="text-sm text-gray-500 dark:text-gray-400"
        >
          {{ t('tools.batchManage.searchResult', { count: filteredSessions.length, total: sessions.length }) }}
        </span>
      </div>

      <div class="flex gap-2">
        <!-- English UI note -->
        <UTooltip :text="canMerge ? '' : t('tools.batchManage.mergeHint')">
          <UButton color="primary" :disabled="!canMerge" icon="i-heroicons-document-duplicate" @click="handleMerge">
            {{ t('tools.batchManage.merge') }}
          </UButton>
        </UTooltip>

        <!-- English UI note -->
        <UButton color="primary" :disabled="selectedIds.size === 0" icon="i-heroicons-trash" @click="openDeleteModal">
          {{ t('tools.batchManage.delete') }}
        </UButton>
      </div>
    </div>

    <!-- English UI note -->
    <div v-if="sessions.length === 0" class="flex flex-1 items-center justify-center">
      <div class="text-center text-gray-500 dark:text-gray-400">
        <UIcon name="i-heroicons-inbox" class="mb-2 h-12 w-12" />
        <p>{{ t('tools.batchManage.empty') }}</p>
      </div>
    </div>

    <div v-else-if="filteredSessions.length === 0" class="flex flex-1 items-center justify-center">
      <div class="text-center text-gray-500 dark:text-gray-400">
        <UIcon name="i-heroicons-magnifying-glass" class="mb-2 h-12 w-12" />
        <p>{{ t('tools.batchManage.noSearchResult') }}</p>
      </div>
    </div>

    <div v-else class="flex-1 overflow-y-auto rounded-lg border border-gray-200/50 dark:border-gray-700/50">
      <!-- English UI note -->
      <div
        class="sticky top-0 z-[1] flex items-center gap-3 border-b border-gray-200 bg-gray-50 px-3 py-2 text-xs font-medium text-gray-500 dark:border-gray-700 dark:bg-gray-800/80 dark:text-gray-400"
      >
        <div class="w-6" />
        <div class="w-8" />
        <div class="min-w-0 flex-1">{{ t('tools.batchManage.columns.name') }}</div>
        <div class="w-20 text-center">{{ t('tools.batchManage.columns.platform') }}</div>
        <div class="w-24 text-right">{{ t('tools.batchManage.columns.messages') }}</div>
        <div class="w-16 text-right">{{ t('tools.batchManage.columns.summaries') }}</div>
        <div class="w-16 text-right">{{ t('tools.batchManage.columns.aiChats') }}</div>
        <div class="w-28 text-right">{{ t('tools.batchManage.columns.importedAt') }}</div>
      </div>

      <!-- English UI note -->
      <div
        v-for="(session, index) in filteredSessions"
        :key="session.id"
        class="flex cursor-pointer items-center gap-3 px-3 py-2 transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
        :class="[
          isSelected(session.id) ? 'bg-pink-50 dark:bg-pink-900/20' : '',
          index !== filteredSessions.length - 1 ? 'border-b border-gray-100 dark:border-gray-800' : '',
        ]"
        @click="handleRowClick(index, session.id, $event)"
      >
        <!-- English UI note -->
        <div class="w-6">
          <UCheckbox :model-value="isSelected(session.id)" @click.stop="handleRowClick(index, session.id, $event)" />
        </div>

        <!-- English UI note -->
        <div class="w-8">
          <img
            v-if="getSessionAvatar(session)"
            :src="getSessionAvatar(session)!"
            :alt="session.name"
            class="h-8 w-8 shrink-0 rounded-full object-cover"
          />
          <div
            v-else
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[10px] font-bold"
            :class="isPrivateChat(session) ? 'bg-pink-500 text-white' : 'bg-primary-500 text-white'"
          >
            {{ getSessionAvatarText(session) }}
          </div>
        </div>

        <!-- English UI note -->
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-1.5">
            <UIcon
              :name="isPrivateChat(session) ? 'i-heroicons-user' : 'i-heroicons-user-group'"
              class="h-3.5 w-3.5 shrink-0 text-gray-400"
            />
            <!-- English UI note -->
            <input
              v-if="editingId === session.id"
              v-model="editingName"
              type="text"
              class="w-full rounded border border-pink-300 bg-white px-2 py-0.5 text-sm font-medium text-gray-900 focus:border-pink-500 focus:outline-none focus:ring-1 focus:ring-pink-500 dark:border-pink-600 dark:bg-gray-800 dark:text-white"
              autofocus
              @blur="saveEdit"
              @keydown.enter="saveEdit"
              @keydown.escape="cancelEdit"
              @click.stop
            />
            <!-- English UI note -->
            <p
              v-else
              class="cursor-text truncate rounded px-1 text-sm font-medium text-gray-900 hover:bg-gray-200 dark:text-white dark:hover:bg-gray-700"
              :title="t('tools.batchManage.clickToEdit')"
              @click="startEdit(session, $event)"
            >
              {{ session.name }}
            </p>
          </div>
        </div>

        <!-- English UI note -->
        <div class="w-20 text-center">
          <span
            class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium"
            :class="getPlatformClass(session.platform)"
          >
            {{ getPlatformLabel(session.platform) }}
          </span>
        </div>

        <!-- English UI note -->
        <div class="w-24 text-right text-sm text-gray-600 dark:text-gray-300">
          {{ session.messageCount.toLocaleString() }}
        </div>

        <!-- English UI note -->
        <div class="w-16 text-right text-sm text-gray-600 dark:text-gray-300">
          {{ session.summaryCount || 0 }}
        </div>

        <!-- English UI note -->
        <div class="w-16 text-right text-sm text-gray-600 dark:text-gray-300">
          {{ session.aiConversationCount || 0 }}
        </div>

        <!-- English UI note -->
        <div class="w-28 text-right text-xs text-gray-500 dark:text-gray-400">
          {{ formatTime(session.importedAt) }}
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <UModal v-model:open="showMergeModal">
      <template #content>
        <div class="p-4">
          <div class="mb-4 flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-full bg-blue-100 dark:bg-blue-900/30">
              <UIcon name="i-heroicons-document-duplicate" class="h-5 w-5 text-blue-600 dark:text-blue-400" />
            </div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
              {{ t('tools.batchManage.mergeConfirmTitle') }}
            </h3>
          </div>

          <p class="mb-4 text-gray-600 dark:text-gray-400">
            {{ t('tools.batchManage.mergeConfirmMessage', { count: selectedIds.size }) }}
          </p>

          <!-- English UI note -->
          <div class="mb-4 max-h-40 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700">
            <div
              v-for="session in sessions.filter((s) => selectedIds.has(s.id))"
              :key="session.id"
              class="flex items-center gap-2 border-b border-gray-100 px-3 py-2 last:border-b-0 dark:border-gray-800"
            >
              <UIcon
                :name="isPrivateChat(session) ? 'i-heroicons-user' : 'i-heroicons-user-group'"
                class="h-4 w-4 text-gray-400"
              />
              <span class="text-sm text-gray-700 dark:text-gray-300">{{ session.name }}</span>
              <span class="text-xs text-gray-400">{{ session.messageCount.toLocaleString() }} 条</span>
            </div>
          </div>

          <!-- English UI note -->
          <div v-if="isMerging" class="mb-4 rounded-lg bg-blue-50 px-4 py-3 dark:bg-blue-900/20">
            <div class="flex items-center gap-2">
              <UIcon name="i-heroicons-arrow-path" class="h-4 w-4 animate-spin text-blue-600 dark:text-blue-400" />
              <span class="text-sm text-blue-700 dark:text-blue-300">{{ mergeProgress }}</span>
            </div>
          </div>

          <div class="flex justify-end gap-2">
            <UButton variant="soft" :disabled="isMerging" @click="showMergeModal = false">
              {{ t('common.cancel') }}
            </UButton>
            <UButton color="primary" :loading="isMerging" @click="executeMerge">
              {{ isMerging ? mergeProgress : t('tools.batchManage.merge') }}
            </UButton>
          </div>
        </div>
      </template>
    </UModal>

    <!-- English UI note -->
    <UModal v-model:open="showDeleteModal">
      <template #content>
        <div class="p-4">
          <div class="mb-4 flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-full bg-red-100 dark:bg-red-900/30">
              <UIcon name="i-heroicons-exclamation-triangle" class="h-5 w-5 text-red-600 dark:text-red-400" />
            </div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
              {{ t('tools.batchManage.confirmTitle') }}
            </h3>
          </div>

          <p class="mb-6 text-gray-600 dark:text-gray-400">
            {{ t('tools.batchManage.confirmMessage', { count: selectedIds.size }) }}
          </p>

          <div class="flex justify-end gap-2">
            <UButton variant="soft" :disabled="isDeleting" @click="closeDeleteModal">
              {{ t('common.cancel') }}
            </UButton>
            <UButton color="error" :loading="isDeleting" @click="confirmBatchDelete">
              {{ isDeleting ? t('tools.batchManage.deleting') : t('common.delete') }}
            </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
