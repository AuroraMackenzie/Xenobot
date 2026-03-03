<script setup lang="ts">
import { onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import TitleBar from '@/components/common/TitleBar.vue'
import Sidebar from '@/components/common/Sidebar.vue'
import SettingModal from '@/components/common/SettingModal.vue'
import ScreenCaptureModal from '@/components/common/ScreenCaptureModal.vue'
import { ChatRecordDrawer } from '@/components/common/ChatRecord'
import { useSessionStore } from '@/stores/session'
import { useLayoutStore } from '@/stores/layout'
import { usePromptStore } from '@/stores/prompt'
import { useSettingsStore } from '@/stores/settings'
import { useLLMStore } from '@/stores/llm'

const { t } = useI18n()

const sessionStore = useSessionStore()
const layoutStore = useLayoutStore()
const promptStore = usePromptStore()
const settingsStore = useSettingsStore()
const llmStore = useLLMStore()
const { isInitialized } = storeToRefs(sessionStore)
const route = useRoute()

const tooltip = {
  delayDuration: 100,
}

// English engineering note.
onMounted(async () => {
  // English engineering note.
  const platform = navigator.platform.toLowerCase()
  if (platform.includes('win')) {
    document.documentElement.classList.add('platform-windows')
  } else if (platform.includes('linux')) {
    document.documentElement.classList.add('platform-linux')
  }

  // English engineering note.
  settingsStore.initLocale()
  // English engineering note.
  llmStore.init()
  // English engineering note.
  await sessionStore.loadSessions()
})
</script>

<template>
  <UApp :tooltip="tooltip">
    <!-- English UI note -->
    <TitleBar />
    <div class="relative flex h-screen w-full overflow-hidden bg-gray-50 dark:bg-gray-900">
      <!-- English UI note -->
      <template v-if="!isInitialized">
        <div class="flex h-full w-full items-center justify-center">
          <div class="flex flex-col items-center justify-center text-center">
            <UIcon name="i-heroicons-arrow-path" class="h-8 w-8 animate-spin text-cyan-500" />
            <p class="mt-2 text-sm text-gray-500">{{ t('common.initializing') }}</p>
          </div>
        </div>
      </template>
      <template v-else>
        <Sidebar />
        <main class="relative flex-1 overflow-hidden">
          <router-view v-slot="{ Component }">
            <Transition name="page-fade" mode="out-in">
              <component :is="Component" :key="route.path" />
            </Transition>
          </router-view>
        </main>
      </template>
    </div>
    <SettingModal v-model:open="layoutStore.showSettingModal" @ai-config-saved="promptStore.notifyAIConfigChanged" />
    <ScreenCaptureModal
      :open="layoutStore.showScreenCaptureModal"
      :image-data="layoutStore.screenCaptureImage"
      @update:open="(v) => (v ? null : layoutStore.closeScreenCaptureModal())"
    />
    <!-- English UI note -->
    <ChatRecordDrawer />
  </UApp>
</template>

<style scoped>
.page-fade-enter-active,
.page-fade-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.page-fade-enter-from {
  opacity: 0;
  transform: translateY(10px);
}

.page-fade-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
