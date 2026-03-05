<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { SchemaPanel, AIGenerateModal, AIHistoryModal, ResultTable } from './SQLLab'
import type { AIHistory, SQLResult } from './SQLLab'

const { t } = useI18n()

// Props
const props = defineProps<{
  sessionId: string
}>()

// English engineering note.
const schemaPanelRef = ref<InstanceType<typeof SchemaPanel> | null>(null)
const resultTableRef = ref<InstanceType<typeof ResultTable> | null>(null)

// English engineering note.
const sql = ref('SELECT * FROM message LIMIT 10')
const isExecuting = ref(false)
const error = ref<string | null>(null)
const result = ref<SQLResult | null>(null)
const lastPrompt = ref('') // English engineering note.

// English engineering note.
const showAIModal = ref(false)
const showHistoryModal = ref(false)

// English engineering note.
const aiHistory = ref<AIHistory[]>([])

const resultViewKey = computed(() => {
  if (isExecuting.value) return 'executing'
  if (error.value) return `error:${error.value}`
  if (result.value) {
    const rowCount = result.value.rows?.length ?? 0
    return `result:${rowCount}`
  }
  return 'idle'
})

// English engineering note.
function loadHistory() {
  try {
    const key = `sql-lab-history-${props.sessionId}`
    const data = localStorage.getItem(key)
    if (data) {
      aiHistory.value = JSON.parse(data)
    }
  } catch (err) {
    console.error('加载历史记录失败:', err)
  }
}

// English engineering note.
function saveHistory() {
  try {
    const key = `sql-lab-history-${props.sessionId}`
    localStorage.setItem(key, JSON.stringify(aiHistory.value))
  } catch (err) {
    console.error('保存历史记录失败:', err)
  }
}

// English engineering note.
function addToHistory(prompt: string, sqlStr: string, explanation: string) {
  const record: AIHistory = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    prompt,
    sql: sqlStr,
    explanation,
    timestamp: Date.now(),
  }
  aiHistory.value.unshift(record)
  if (aiHistory.value.length > 50) {
    aiHistory.value = aiHistory.value.slice(0, 50)
  }
  saveHistory()
}

// English engineering note.
function deleteHistory(id: string) {
  aiHistory.value = aiHistory.value.filter((r) => r.id !== id)
  saveHistory()
}

// English engineering note.
async function executeSQL() {
  if (!sql.value.trim()) {
    error.value = t('ai.sqlLab.editor.errorEmptySQL')
    return
  }

  isExecuting.value = true
  error.value = null
  result.value = null
  resultTableRef.value?.resetSort()

  try {
    result.value = await window.chatApi.executeSQL(props.sessionId, sql.value)
  } catch (err: any) {
    error.value = err.message || String(err)
  } finally {
    isExecuting.value = false
  }
}

// English engineering note.
function handleKeyDown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault()
    executeSQL()
  }
}

// English engineering note.
function handleInsertColumn(tableName: string, columnName: string) {
  sql.value += `${tableName}.${columnName}`
}

// English engineering note.
function handleAIGenerated(generatedSql: string, explanation: string, prompt: string) {
  addToHistory(prompt, generatedSql, explanation)
  lastPrompt.value = prompt // English engineering note.
}

// English engineering note.
function handleUseSQL(generatedSql: string) {
  sql.value = generatedSql
}

// English engineering note.
async function handleRunSQL(generatedSql: string) {
  sql.value = generatedSql
  await executeSQL()
}

// English engineering note.
async function executeFromHistory(record: AIHistory) {
  sql.value = record.sql
  lastPrompt.value = record.prompt // English engineering note.
  showHistoryModal.value = false
  await executeSQL()
}

onMounted(() => {
  loadHistory()
})
</script>

<template>
  <div class="main-content xeno-sql-shell flex h-full">
    <!-- English UI note -->
    <SchemaPanel ref="schemaPanelRef" :session-id="sessionId" @insert-column="handleInsertColumn" />

    <!-- English UI note -->
    <div class="xeno-sql-main flex flex-1 flex-col overflow-hidden">
      <!-- English UI note -->
      <div class="xeno-sql-editor-wrap flex flex-col p-4">
        <div class="mx-auto w-full max-w-3xl">
          <!-- English UI note -->
          <textarea
            v-model="sql"
            class="xeno-sql-editor h-32 w-full resize-none rounded-lg p-3 font-mono text-sm"
            :placeholder="t('ai.sqlLab.editor.placeholder')"
            spellcheck="false"
            @keydown="handleKeyDown"
          />

          <!-- English UI note -->
          <div class="mt-3 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="text-xs text-gray-400">{{ t('ai.sqlLab.editor.tip') }}</span>
            </div>
            <div class="flex items-center gap-2">
              <UButton variant="ghost" size="sm" :disabled="aiHistory.length === 0" @click="showHistoryModal = true">
                <UIcon name="i-heroicons-clock" class="mr-1 h-4 w-4" />
                {{ t('ai.sqlLab.editor.history') }}
                <span v-if="aiHistory.length > 0" class="ml-1 text-xs text-gray-400">({{ aiHistory.length }})</span>
              </UButton>
              <UButton variant="outline" size="sm" @click="showAIModal = true">
                <UIcon name="i-heroicons-sparkles" class="mr-1 h-4 w-4" />
                {{ t('ai.sqlLab.editor.aiGenerate') }}
              </UButton>
              <span class="text-xs text-gray-400">{{ t('ai.sqlLab.editor.shortcut') }}</span>
              <UButton color="primary" size="sm" :loading="isExecuting" @click="executeSQL">
                <UIcon name="i-heroicons-play" class="mr-1 h-4 w-4" />
                {{ t('ai.sqlLab.editor.run') }}
              </UButton>
            </div>
          </div>
        </div>
      </div>

      <Transition name="xeno-sql-result" mode="out-in">
        <div :key="resultViewKey" class="xeno-sql-result-host min-h-0 flex-1">
          <ResultTable ref="resultTableRef" :result="result" :error="error" :sql="sql" :prompt="lastPrompt" />
        </div>
      </Transition>
    </div>

    <!-- English UI note -->
    <AIGenerateModal
      v-model:open="showAIModal"
      :session-id="sessionId"
      @generated="handleAIGenerated"
      @use-s-q-l="handleUseSQL"
      @run-s-q-l="handleRunSQL"
    />

    <!-- English UI note -->
    <AIHistoryModal
      v-model:open="showHistoryModal"
      :history="aiHistory"
      @execute="executeFromHistory"
      @delete="deleteHistory"
    />
  </div>
</template>

<style scoped>
.xeno-sql-shell {
  background: linear-gradient(180deg, transparent, var(--xeno-surface-muted));
}

.xeno-sql-main {
  border-left: 1px solid var(--xeno-border-soft);
}

.xeno-sql-editor-wrap {
  border-bottom: 1px solid var(--xeno-border-soft);
  background: var(--xeno-surface-muted);
  backdrop-filter: blur(14px) saturate(128%);
}

.xeno-sql-editor {
  border: 1px solid var(--xeno-border-strong);
  background: var(--xeno-surface-emphasis);
  color: var(--xeno-text-main);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18);
}

.xeno-sql-editor::placeholder {
  color: var(--xeno-text-secondary);
  opacity: 0.85;
}

.xeno-sql-editor:focus {
  border-color: var(--xeno-active-border);
  outline: none;
  box-shadow:
    0 0 0 2px var(--xeno-focus-ring),
    inset 0 1px 0 rgba(255, 255, 255, 0.18);
}

.xeno-sql-result-host {
  min-height: 0;
}

.xeno-sql-result-enter-active,
.xeno-sql-result-leave-active {
  transition:
    opacity 0.26s cubic-bezier(0.22, 0.92, 0.3, 1),
    transform 0.26s cubic-bezier(0.22, 0.92, 0.3, 1),
    filter 0.26s cubic-bezier(0.22, 0.92, 0.3, 1);
}

.xeno-sql-result-enter-from,
.xeno-sql-result-leave-to {
  opacity: 0;
  transform: translateY(10px) scale(0.994);
  filter: blur(6px);
}

@media (prefers-reduced-motion: reduce) {
  .xeno-sql-result-enter-active,
  .xeno-sql-result-leave-active {
    transition-duration: 0.01ms !important;
  }

  .xeno-sql-result-enter-from,
  .xeno-sql-result-leave-to {
    opacity: 1;
    transform: none;
    filter: none;
  }
}
</style>
