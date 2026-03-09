<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSessionStore } from '@/stores/session'
import MarkdownIt from 'markdown-it'

const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
})

const { t } = useI18n()
const sessionStore = useSessionStore()

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

// Props
const props = defineProps<{
  filterResult: {
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
  } | null
  filterMode: 'condition' | 'session'
}>()

const open = defineModel<boolean>('open', { default: false })

const DATA_TOO_LARGE_THRESHOLD = 5000

const isDataTooLarge = computed(() => {
  if (!props.filterResult) return false
  return props.filterResult.stats.totalMessages > DATA_TOO_LARGE_THRESHOLD
})

const analysisMode = ref<'preset' | 'custom'>('preset')

const presetOptions = [
  { id: 'summary', label: '总结对话要点', prompt: '请总结这段对话的主要内容和关键要点。' },
  { id: 'sentiment', label: '情感分析', prompt: '请分析这段对话中参与者的情感变化和整体氛围。' },
  { id: 'topics', label: '话题提取', prompt: '请提取这段对话中讨论的主要话题，并简要说明每个话题的内容。' },
  { id: 'insights', label: '洞察分析', prompt: '请对这段对话进行深度分析，包括参与者的关系、互动模式、潜在问题等。' },
]

const selectedPreset = ref(presetOptions[0].id)
const customPrompt = ref('')

const editablePresetPrompt = ref(presetOptions[0].prompt)

const isAnalyzing = ref(false)
const analysisResult = ref('')
const analysisError = ref('')

let currentRequestId: string | null = null

const contextContent = computed(() => {
  if (!props.filterResult) return ''

  let content = ''
  for (const block of props.filterResult.blocks) {
    const startTime = new Date(block.startTs * 1000).toLocaleString()
    content += `\n--- ${t('analysis.filter.blockTitle', { index: startTime })} ---\n`

    for (const msg of block.messages) {
      const time = new Date(msg.timestamp * 1000).toLocaleTimeString()
      content += `[${time}] ${msg.senderName}: ${msg.content || '[非文本消息]'}\n`
    }
  }
  return content
})

const userQuestion = computed(() => {
  if (analysisMode.value === 'preset') {
    return editablePresetPrompt.value
  }
  return customPrompt.value
})

async function executeAnalysis() {
  if (!props.filterResult || !userQuestion.value) return

  isAnalyzing.value = true
  analysisResult.value = ''
  analysisError.value = ''

  try {
    const fullMessage = `${t('analysis.filter.analysisPromptPrefix')}

${contextContent.value}

---
${t('analysis.filter.analysisPromptQuestion')}${userQuestion.value}`

    const context = {
      sessionId: sessionStore.currentSessionId || '',
    }

    const { requestId, promise } = window.agentApi.runStream(
      fullMessage,
      context,
      (chunk) => {
        if (chunk.type === 'content' && chunk.content) {
          analysisResult.value += chunk.content
        } else if (chunk.type === 'error') {
          analysisError.value = chunk.error || t('analysis.filter.analysisRuntimeError')
        }
      },
      [],
      sessionStore.currentSession?.type === 'group' ? 'group' : 'private'
    )

    currentRequestId = requestId

    const result = await promise

    if (!result.success && result.error) {
      analysisError.value = result.error
    }
  } catch (error) {
    console.error('[LocalAnalysisModal] Failed to execute analysis:', error)
    analysisError.value = String(error)
  } finally {
    isAnalyzing.value = false
    currentRequestId = null
  }
}

async function abortAnalysis() {
  if (currentRequestId) {
    try {
      await window.agentApi.abort(currentRequestId)
    } catch (error) {
      console.error('[LocalAnalysisModal] Failed to abort analysis:', error)
    }
    isAnalyzing.value = false
    currentRequestId = null
  }
}

async function copyResult() {
  try {
    await navigator.clipboard.writeText(analysisResult.value)
  } catch (error) {
    console.error('[LocalAnalysisModal] Failed to copy analysis result:', error)
  }
}

watch(selectedPreset, (newPresetId) => {
  const preset = presetOptions.find((p) => p.id === newPresetId)
  if (preset) {
    editablePresetPrompt.value = preset.prompt
  }
})

watch(open, (val) => {
  if (!val) {
    analysisResult.value = ''
    analysisError.value = ''
    if (isAnalyzing.value) {
      abortAnalysis()
    }
  } else {
    editablePresetPrompt.value = presetOptions.find((p) => p.id === selectedPreset.value)?.prompt || ''
  }
})
</script>

<template>
  <UModal v-model:open="open" :ui="{ width: 'max-w-3xl' }">
    <template #content>
      <UCard class="xeno-local-analysis-card">
        <template #header>
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold">{{ t('analysis.filter.localAnalysisTitle') }}</h3>
            <UButton variant="ghost" icon="i-heroicons-x-mark" size="sm" @click="open = false" />
          </div>
        </template>

        <div class="space-y-4">
          <div class="xeno-local-analysis-panel rounded-xl p-3 text-sm">
            <div class="flex items-center gap-4 text-gray-600 dark:text-gray-400">
              <span>{{ filterResult?.blocks.length || 0 }} {{ t('analysis.filter.stats.blocks') }}</span>
              <span>{{ filterResult?.stats.totalMessages || 0 }} {{ t('analysis.filter.stats.messages') }}</span>
              <span>{{ filterResult?.stats.totalChars.toLocaleString() || 0 }} {{ t('analysis.filter.stats.chars') }}</span>
            </div>
          </div>

          <div
            v-if="isDataTooLarge"
            class="xeno-local-analysis-alert rounded-xl p-3"
          >
            <div class="flex items-start gap-2">
              <UIcon name="i-heroicons-exclamation-triangle" class="w-5 h-5 text-red-500 shrink-0 mt-0.5" />
              <div class="text-sm">
                <p class="font-medium text-red-700 dark:text-red-400">
                  {{ t('analysis.filter.dataTooLarge') }}
                </p>
                <p class="text-red-600 dark:text-red-500 mt-1">
                  {{ t('analysis.filter.dataTooLargeThreshold', { count: DATA_TOO_LARGE_THRESHOLD }) }}
                </p>
              </div>
            </div>
          </div>

          <div class="xeno-local-analysis-mode flex w-fit items-center gap-1 rounded-lg p-1">
            <button
              class="px-3 py-1.5 text-sm font-medium rounded-md transition-colors"
              :class="
                analysisMode === 'preset'
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
              "
              @click="analysisMode = 'preset'"
            >
              {{ t('analysis.filter.presetAnalysis') }}
            </button>
            <button
              class="px-3 py-1.5 text-sm font-medium rounded-md transition-colors"
              :class="
                analysisMode === 'custom'
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                  : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
              "
              @click="analysisMode = 'custom'"
            >
              {{ t('analysis.filter.customAnalysis') }}
            </button>
          </div>

          <div v-if="analysisMode === 'preset'" class="space-y-3">
            <div class="grid grid-cols-2 gap-2">
              <label
                v-for="option in presetOptions"
                :key="option.id"
                class="flex items-center gap-2 p-3 border rounded-lg cursor-pointer transition-colors"
                :class="
                  selectedPreset === option.id
                    ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                "
              >
                <input v-model="selectedPreset" type="radio" :value="option.id" class="text-primary-500" />
                <span class="text-sm text-gray-700 dark:text-gray-300">{{ option.label }}</span>
              </label>
            </div>

            <div class="xeno-local-analysis-panel space-y-1 rounded-xl p-3">
              <label class="text-xs text-gray-500 dark:text-gray-400">
                {{ t('analysis.filter.editablePromptLabel') }}
              </label>
              <UTextarea v-model="editablePresetPrompt" :rows="2" class="w-full text-sm" />
            </div>
          </div>

          <!-- English UI note -->
          <div v-else>
            <UTextarea
              v-model="customPrompt"
              :placeholder="t('analysis.filter.customPromptPlaceholder')"
              :rows="3"
              class="w-full"
            />
          </div>

          <div class="flex justify-end gap-2">
            <UButton v-if="isAnalyzing" color="red" variant="outline" @click="abortAnalysis">
              {{ t('common.cancel') }}
            </UButton>
            <UButton
              color="primary"
              :loading="isAnalyzing"
              :disabled="isAnalyzing || isDataTooLarge || (analysisMode === 'custom' && !customPrompt.trim())"
              @click="executeAnalysis"
            >
              <UIcon name="i-heroicons-sparkles" class="w-4 h-4 mr-1" />
              {{ isAnalyzing ? t('analysis.filter.analyzing') : t('analysis.filter.startAnalysis') }}
            </UButton>
          </div>

          <div v-if="analysisResult || analysisError" class="border-t border-gray-200 pt-4 dark:border-gray-700">
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">
                {{ t('analysis.filter.analysisResult') }}
              </h4>
              <UButton v-if="analysisResult" size="xs" variant="ghost" @click="copyResult">
                <UIcon name="i-heroicons-clipboard" class="w-4 h-4 mr-1" />
                {{ t('common.copy') }}
              </UButton>
            </div>

            <div
              v-if="analysisError"
              class="xeno-local-analysis-alert rounded-xl p-3 text-sm text-red-600 dark:text-red-400"
            >
              {{ analysisError }}
            </div>

            <div
              v-else-if="analysisResult"
              class="xeno-local-analysis-result prose prose-sm dark:prose-invert max-w-none max-h-80 overflow-y-auto rounded-xl p-4"
              v-html="md.render(analysisResult)"
            />
          </div>
        </div>
      </UCard>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-local-analysis-card {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.6rem;
  background:
    radial-gradient(circle at top left, rgba(84, 214, 255, 0.12), transparent 24%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 22%),
    rgba(7, 18, 29, 0.95);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 30px 72px rgba(2, 8, 16, 0.36);
  backdrop-filter: blur(22px) saturate(134%);
}

.xeno-local-analysis-panel,
.xeno-local-analysis-result,
.xeno-local-analysis-mode {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.48);
}

.xeno-local-analysis-alert {
  border: 1px solid rgba(248, 113, 113, 0.18);
  background:
    linear-gradient(180deg, rgba(248, 113, 113, 0.08), transparent 120%),
    rgba(40, 10, 16, 0.38);
}
</style>
