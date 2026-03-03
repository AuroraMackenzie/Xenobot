<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { SubTabs } from '@/components/UI'
import ChatExplorer from './AIChat/ChatExplorer.vue'
import SQLLabTab from './SQLLabTab.vue'
import FilterTab from './Filter/FilterTab.vue'

const { t, locale } = useI18n()

// English engineering note.
const followLink = computed(() => {
  if (locale.value === 'zh-CN') {
    return {
      url: 'https://xenobot.app/cn/',
      name: 'Xenobot CN',
    }
  }
  return {
    url: 'https://xenobot.app/en/',
    name: 'Xenobot EN',
  }
})

// Props
const props = defineProps<{
  sessionId: string
  sessionName: string
  timeFilter?: { startTs: number; endTs: number }
  chatType?: 'group' | 'private'
}>()

const route = useRoute()

// English engineering note.
const isGroupChat = computed(() => route.name === 'circle-room')

// English engineering note.
const groupOnlyTabs = ['mbti', 'cyber-friend', 'campus']

// English engineering note.
const allSubTabs = computed(() => [
  { id: 'chat-explorer', label: t('ai.tab.chatExplorer'), icon: 'i-heroicons-chat-bubble-left-ellipsis' },
  {
    id: 'manual',
    label: t('ai.tab.filterAnalysis'),
    desc: t('ai.tab.filterAnalysisDesc'),
    icon: 'i-heroicons-adjustments-horizontal',
  },
  { id: 'sql-lab', label: t('ai.tab.sqlLab'), icon: 'i-heroicons-command-line' },
])

// English engineering note.
const subTabs = computed(() => {
  if (isGroupChat.value) {
    // English engineering note.
    return allSubTabs.value
  }
  // English engineering note.
  return allSubTabs.value.filter((tab) => !groupOnlyTabs.includes(tab.id))
})

const activeSubTab = ref('chat-explorer')

// English engineering note.
const chatExplorerRef = ref<InstanceType<typeof ChatExplorer> | null>(null)

// English engineering note.
function refreshAIConfig() {
  chatExplorerRef.value?.refreshConfig()
}

// English engineering note.
defineExpose({
  refreshAIConfig,
})
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- English UI note -->
    <SubTabs v-model="activeSubTab" :items="subTabs" persist-key="aiTab" />

    <!-- English UI note -->
    <div class="flex-1 min-h-0 overflow-hidden">
      <Transition name="fade" mode="out-in">
        <!-- English UI note -->
        <ChatExplorer
          v-if="activeSubTab === 'chat-explorer'"
          ref="chatExplorerRef"
          class="h-full"
          :session-id="sessionId"
          :session-name="sessionName"
          :time-filter="timeFilter"
          :chat-type="chatType"
        />
        <!-- English UI note -->
        <FilterTab v-else-if="activeSubTab === 'manual'" class="h-full" />
        <!-- English UI note -->
        <SQLLabTab v-else-if="activeSubTab === 'sql-lab'" class="h-full" :session-id="props.sessionId" />

        <!-- English UI note -->
        <div
          v-else-if="['mbti', 'cyber-friend', 'campus'].includes(activeSubTab)"
          class="main-content flex h-full items-center justify-center p-6"
        >
          <div
            class="flex h-full w-full items-center justify-center rounded-xl border-2 border-dashed border-gray-300 bg-gray-50 dark:border-gray-700 dark:bg-gray-900/50"
          >
            <div class="text-center">
              <UIcon :name="subTabs.find((t) => t.id === activeSubTab)?.icon" class="mx-auto h-12 w-12 text-gray-400" />
              <p class="mt-3 text-sm font-medium text-gray-600 dark:text-gray-400">
                {{ t('ai.tab.featureInDev', { name: subTabs.find((tab) => tab.id === activeSubTab)?.label || '' }) }}
              </p>
              <p class="mt-1 max-w-md px-4 text-sm text-gray-500">
                {{ subTabs.find((tab) => tab.id === activeSubTab)?.desc || t('ai.tab.comingSoon') }}
              </p>

              <div class="mt-8 flex items-center justify-center gap-1 text-xs text-gray-400">
                <span>{{ t('ai.tab.followNotice') }}</span>
                <UButton
                  :to="followLink.url"
                  target="_blank"
                  variant="link"
                  :padded="false"
                  class="text-xs font-medium"
                >
                  {{ followLink.name }}
                </UButton>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
