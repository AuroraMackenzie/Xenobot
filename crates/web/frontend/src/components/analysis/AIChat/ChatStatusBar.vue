<script setup lang="ts">
import { ref, computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { useToast } from '@nuxt/ui/runtime/composables/useToast.js'
import { usePromptStore } from '@/stores/prompt'
import { useLayoutStore } from '@/stores/layout'
import { useLLMStore } from '@/stores/llm'

const { t } = useI18n()
const toast = useToast()

// Props
const props = defineProps<{
  chatType: 'group' | 'private'
  sessionTokenUsage: { totalTokens: number }
  hasLLMConfig: boolean
  isCheckingConfig: boolean
}>()

// Store
const promptStore = usePromptStore()
const layoutStore = useLayoutStore()
const llmStore = useLLMStore()
const { aiPromptSettings, activePreset, aiGlobalSettings } = storeToRefs(promptStore)
const { configs, activeConfig, isLoading: isLoadingLLM } = storeToRefs(llmStore)

// English engineering note.
const currentPresets = computed(() => promptStore.getPresetsForChatType(props.chatType))

// English engineering note.
const currentActivePresetId = computed(() => aiPromptSettings.value.activePresetId)

// English engineering note.
const currentActivePreset = computed(() => {
  const activeInList = currentPresets.value.find((p) => p.id === currentActivePresetId.value)
  return activeInList || activePreset.value
})

// English engineering note.
const isPresetPopoverOpen = ref(false)
const isModelPopoverOpen = ref(false)
const isOpeningLog = ref(false)

// English engineering note.
function setActivePreset(presetId: string) {
  promptStore.setActivePreset(presetId)
  isPresetPopoverOpen.value = false
}

// English engineering note.
function openPresetSettings() {
  isPresetPopoverOpen.value = false
  layoutStore.openSettingAt('ai', 'preset')
}

// English engineering note.
function openChatSettings() {
  layoutStore.openSettingAt('ai', 'chat')
}

// English engineering note.
async function switchModelConfig(configId: string) {
  const success = await llmStore.setActiveConfig(configId)
  if (success) {
    isModelPopoverOpen.value = false
  } else {
    toast.add({
      title: t('ai.chat.statusBar.model.switchFailed'),
      icon: 'i-heroicons-x-circle',
      color: 'error',
      duration: 2000,
    })
  }
}

// English engineering note.
function openModelSettings() {
  isModelPopoverOpen.value = false
  layoutStore.openSettingAt('ai', 'model')
}

// English engineering note.
async function openAiLogFile() {
  if (isOpeningLog.value) return
  isOpeningLog.value = true
  try {
    const result = await window.aiApi.showAiLogFile()
    if (!result?.success) {
      toast.add({
        title: t('ai.chat.statusBar.log.openFailed'),
        description: result?.error || t('ai.chat.statusBar.log.openFailedDesc'),
        icon: 'i-heroicons-x-circle',
        color: 'error',
        duration: 2000,
      })
    }
  } catch (error) {
    console.error('打开 AI 日志失败：', error)
    toast.add({
      title: t('ai.chat.statusBar.log.openFailed'),
      description: String(error),
      icon: 'i-heroicons-x-circle',
      color: 'error',
      duration: 2000,
    })
  } finally {
    isOpeningLog.value = false
  }
}
</script>

<template>
  <div class="flex items-center justify-between px-1">
    <!-- English UI note -->
    <div class="flex items-center gap-1">
      <UPopover v-model:open="isPresetPopoverOpen" :ui="{ content: 'p-0' }">
        <button
          class="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-gray-500 transition-colors hover:bg-gray-100 hover:text-gray-700 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-gray-300"
        >
          <UIcon name="i-heroicons-chat-bubble-bottom-center-text" class="h-3.5 w-3.5" />
          <span class="max-w-[120px] truncate">
            {{ currentActivePreset?.name || t('ai.chat.statusBar.preset.default') }}
          </span>
          <UIcon name="i-heroicons-chevron-down" class="h-3 w-3" />
        </button>
        <template #content>
          <div class="w-48 py-1">
            <div class="px-3 py-1.5 text-xs font-medium text-gray-400 dark:text-gray-500">
              {{
                chatType === 'group'
                  ? t('ai.chat.statusBar.preset.groupTitle')
                  : t('ai.chat.statusBar.preset.privateTitle')
              }}
            </div>
            <button
              v-for="preset in currentPresets"
              :key="preset.id"
              class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
              :class="[
                preset.id === currentActivePresetId
                  ? 'text-pink-600 dark:text-pink-400'
                  : 'text-gray-700 dark:text-gray-300',
              ]"
              @click="setActivePreset(preset.id)"
            >
              <UIcon
                :name="
                  preset.id === currentActivePresetId ? 'i-heroicons-check-circle-solid' : 'i-heroicons-document-text'
                "
                class="h-4 w-4 shrink-0"
                :class="[preset.id === currentActivePresetId ? 'text-pink-500' : 'text-gray-400']"
              />
              <span class="truncate">{{ preset.name }}</span>
            </button>

            <!-- English UI note -->
            <div class="my-1 border-t border-gray-200 dark:border-gray-700" />

            <!-- English UI note -->
            <button
              class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-gray-500 transition-colors hover:bg-gray-100 hover:text-gray-700 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-gray-300"
              @click="openPresetSettings"
            >
              <UIcon name="i-heroicons-cog-6-tooth" class="h-4 w-4 shrink-0" />
              <span>{{ t('ai.chat.statusBar.preset.manage') }}</span>
            </button>
          </div>
        </template>
      </UPopover>

      <!-- English UI note -->
      <UPopover v-model:open="isModelPopoverOpen" :ui="{ content: 'p-0' }">
        <button
          class="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-gray-500 transition-colors hover:bg-gray-100 hover:text-gray-700 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-gray-300"
          :disabled="isLoadingLLM"
        >
          <UIcon name="i-heroicons-cpu-chip" class="h-3.5 w-3.5" />
          <span class="max-w-[120px] truncate">
            {{ activeConfig?.name || t('ai.chat.statusBar.model.notConfigured') }}
          </span>
          <UIcon name="i-heroicons-chevron-down" class="h-3 w-3" />
        </button>
        <template #content>
          <div class="w-48 py-1">
            <div class="px-3 py-1.5 text-xs font-medium text-gray-400 dark:text-gray-500">
              {{ t('ai.chat.statusBar.model.title') }}
            </div>

            <!-- English UI note -->
            <template v-if="configs.length > 0">
              <button
                v-for="config in configs"
                :key="config.id"
                class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
                :class="[
                  config.id === activeConfig?.id
                    ? 'text-pink-600 dark:text-pink-400'
                    : 'text-gray-700 dark:text-gray-300',
                ]"
                @click="switchModelConfig(config.id)"
              >
                <UIcon
                  :name="config.id === activeConfig?.id ? 'i-heroicons-check-circle-solid' : 'i-heroicons-cpu-chip'"
                  class="h-4 w-4 shrink-0"
                  :class="[config.id === activeConfig?.id ? 'text-pink-500' : 'text-gray-400']"
                />
                <span class="truncate">{{ config.name }}</span>
              </button>
            </template>

            <!-- English UI note -->
            <div v-else class="px-3 py-2 text-sm text-gray-400 dark:text-gray-500">
              {{ t('ai.chat.statusBar.model.empty') }}
            </div>

            <!-- English UI note -->
            <div class="my-1 border-t border-gray-200 dark:border-gray-700" />

            <!-- English UI note -->
            <button
              class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-gray-500 transition-colors hover:bg-gray-100 hover:text-gray-700 dark:text-gray-400 dark:hover:bg-gray-800 dark:hover:text-gray-300"
              @click="openModelSettings"
            >
              <UIcon name="i-heroicons-cog-6-tooth" class="h-4 w-4 shrink-0" />
              <span>{{ t('ai.chat.statusBar.model.manage') }}</span>
            </button>
          </div>
        </template>
      </UPopover>
    </div>

    <!-- English UI note -->
    <div class="flex items-center gap-1">
      <!-- English UI note -->
      <button
        class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-gray-400 transition-colors hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800 dark:hover:text-gray-300"
        :title="t('ai.chat.statusBar.messageLimit.title')"
        @click="openChatSettings"
      >
        <span>{{ t('ai.chat.statusBar.messageLimit.label') }}{{ aiGlobalSettings.maxMessagesPerRequest }}</span>
      </button>
      <!-- English UI note -->
      <button
        class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-gray-400 transition-colors hover:bg-gray-100 hover:text-gray-600 disabled:cursor-not-allowed disabled:opacity-60 dark:hover:bg-gray-800 dark:hover:text-gray-300"
        :title="t('ai.chat.statusBar.log.title')"
        :disabled="isOpeningLog"
        @click="openAiLogFile"
      >
        <UIcon name="i-heroicons-folder-open" class="h-3.5 w-3.5" />
        <span>{{ t('ai.chat.statusBar.log.label') }}</span>
      </button>
    </div>
  </div>
</template>
