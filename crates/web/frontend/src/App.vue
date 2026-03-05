<script setup lang="ts">
import { onMounted, ref } from 'vue'
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
const isBooting = ref(true)
const isRouteTransitioning = ref(false)

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

  // English engineering note.
  window.setTimeout(() => {
    isBooting.value = false
  }, 680)
})

// English engineering note.
function onRouteBeforeEnter() {
  isRouteTransitioning.value = true
}

// English engineering note.
function onRouteBeforeLeave() {
  isRouteTransitioning.value = true
}

// English engineering note.
function onRouteAfterEnter() {
  window.setTimeout(() => {
    isRouteTransitioning.value = false
  }, 80)
}

// English engineering note.
function onRouteTransitionCancelled() {
  isRouteTransitioning.value = false
}
</script>

<template>
  <UApp :tooltip="tooltip">
    <!-- English UI note -->
    <TitleBar />
    <div class="xeno-app-shell relative flex h-screen w-full overflow-hidden" :class="{ 'xeno-app-booting': isBooting }">
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
        <main class="xeno-page-content relative flex-1 overflow-hidden">
          <div
            class="xeno-route-curtain pointer-events-none absolute inset-0 z-20"
            :class="{ 'xeno-route-curtain-active': isRouteTransitioning }"
            aria-hidden="true"
          />
          <router-view v-slot="{ Component }">
            <Transition
              name="xeno-route"
              mode="out-in"
              @before-enter="onRouteBeforeEnter"
              @before-leave="onRouteBeforeLeave"
              @after-enter="onRouteAfterEnter"
              @enter-cancelled="onRouteTransitionCancelled"
              @leave-cancelled="onRouteTransitionCancelled"
            >
              <component :is="Component" :key="route.fullPath" />
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
.xeno-app-shell {
  isolation: isolate;
}

.xeno-app-shell::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  opacity: 0;
  background: radial-gradient(circle at 18% 8%, rgba(14, 165, 233, 0.24), transparent 42%);
}

.xeno-app-booting::before {
  animation: xeno-boot-reveal 760ms cubic-bezier(0.22, 0.92, 0.3, 1) forwards;
}

.xeno-route-curtain {
  opacity: 0;
  transform: scale(1.02);
  background:
    radial-gradient(circle at 70% 22%, rgba(14, 165, 233, 0.14), transparent 40%),
    radial-gradient(circle at 16% 84%, rgba(45, 212, 191, 0.12), transparent 36%),
    linear-gradient(120deg, transparent 32%, rgba(255, 255, 255, 0.09) 48%, transparent 62%);
}

.xeno-route-curtain-active {
  animation: xeno-route-curtain-pulse 420ms cubic-bezier(0.22, 0.92, 0.3, 1) both;
}

.xeno-route-enter-active,
.xeno-route-leave-active {
  transition:
    opacity 0.34s cubic-bezier(0.22, 0.92, 0.3, 1),
    transform 0.34s cubic-bezier(0.22, 0.92, 0.3, 1),
    filter 0.34s cubic-bezier(0.22, 0.92, 0.3, 1);
}

.xeno-route-enter-from {
  opacity: 0;
  transform: translateY(16px) scale(0.992);
  filter: blur(8px) saturate(108%);
}

.xeno-route-leave-to {
  opacity: 0;
  transform: translateY(-12px) scale(0.996);
  filter: blur(7px) saturate(106%);
}

.xeno-route-enter-to,
.xeno-route-leave-from {
  opacity: 1;
  transform: translateY(0) scale(1);
  filter: blur(0) saturate(100%);
}

@keyframes xeno-boot-reveal {
  0% {
    opacity: 0.66;
    transform: scale(1.02);
  }
  100% {
    opacity: 0;
    transform: scale(1);
  }
}

@keyframes xeno-route-curtain-pulse {
  0% {
    opacity: 0;
    transform: scale(1.03);
  }
  20% {
    opacity: 0.8;
  }
  100% {
    opacity: 0;
    transform: scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .xeno-app-booting::before,
  .xeno-route-curtain-active {
    animation: none !important;
  }

  .xeno-route-enter-active,
  .xeno-route-leave-active {
    transition-duration: 0.01ms !important;
  }

  .xeno-route-enter-from,
  .xeno-route-leave-to {
    opacity: 1;
    transform: none;
    filter: none;
  }
}
</style>
