<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import TitleBar from '@/components/common/TitleBar.vue'
import Sidebar from '@/components/common/Sidebar.vue'
import XenoSingularityBackdrop from '@/components/common/XenoSingularityBackdrop.vue'
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
const routeTransitionName = ref<'xeno-route-forward' | 'xeno-route-back'>('xeno-route-forward')
const previousRouteRank = ref(0)

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

function resolveRouteRank(name: string | symbol | null | undefined): number {
  if (typeof name !== 'string') {
    return 0
  }

  switch (name) {
    case 'launchpad':
      return 0
    case 'workbench':
      return 1
    case 'circle-room':
    case 'direct-room':
      return 2
    default:
      return 1
  }
}

watch(
  () => route.name,
  (nextName) => {
    const nextRank = resolveRouteRank(nextName)
    routeTransitionName.value = nextRank >= previousRouteRank.value ? 'xeno-route-forward' : 'xeno-route-back'
    previousRouteRank.value = nextRank
  },
  { immediate: true },
)

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
      <div class="xeno-shell-atmosphere pointer-events-none absolute inset-0 z-0" aria-hidden="true">
        <XenoSingularityBackdrop />
        <div class="xeno-shell-orbit xeno-shell-orbit-a" />
        <div class="xeno-shell-orbit xeno-shell-orbit-b" />
        <div class="xeno-shell-beam" />
        <div class="xeno-shell-horizon" />
      </div>
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
          <div class="xeno-page-atmosphere pointer-events-none absolute inset-0 z-0" aria-hidden="true">
            <div class="xeno-page-scanline" />
            <div class="xeno-page-pulse" />
          </div>
          <div
            class="xeno-route-progress pointer-events-none absolute left-0 right-0 top-0 z-30"
            :class="{ 'xeno-route-progress-active': isRouteTransitioning }"
            aria-hidden="true"
          />
          <div
            class="xeno-route-curtain pointer-events-none absolute inset-0 z-20"
            :class="{ 'xeno-route-curtain-active': isRouteTransitioning }"
            aria-hidden="true"
          />
          <router-view v-slot="{ Component }">
            <Transition
              :name="routeTransitionName"
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

.xeno-app-shell > :not(.xeno-shell-atmosphere) {
  position: relative;
  z-index: 1;
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

.xeno-app-shell::after {
  content: '';
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 20%),
    radial-gradient(circle at 76% 18%, rgba(34, 211, 238, 0.1), transparent 30%);
  mix-blend-mode: screen;
  opacity: 0.72;
}

.xeno-app-booting::before {
  animation: xeno-boot-reveal 760ms cubic-bezier(0.22, 0.92, 0.3, 1) forwards;
}

.xeno-shell-atmosphere {
  overflow: hidden;
}

.xeno-shell-orbit {
  position: absolute;
  border-radius: 9999px;
  filter: blur(14px);
  opacity: 0.7;
}

.xeno-shell-orbit-a {
  top: -18%;
  right: 8%;
  width: 28rem;
  height: 28rem;
  border: 1px solid rgba(56, 189, 248, 0.14);
  background: radial-gradient(circle, rgba(56, 189, 248, 0.14), transparent 68%);
}

.xeno-shell-orbit-b {
  bottom: -22%;
  left: 20%;
  width: 20rem;
  height: 20rem;
  border: 1px solid rgba(45, 212, 191, 0.12);
  background: radial-gradient(circle, rgba(45, 212, 191, 0.12), transparent 72%);
}

.xeno-shell-beam {
  position: absolute;
  inset: 0 auto 0 22%;
  width: 36rem;
  transform: skewX(-14deg);
  background: linear-gradient(180deg, rgba(56, 189, 248, 0.08), transparent 42%, rgba(14, 165, 233, 0.05) 100%);
  filter: blur(18px);
  opacity: 0.55;
}

.xeno-shell-horizon {
  position: absolute;
  left: 0;
  right: 0;
  top: 4.2rem;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(56, 189, 248, 0.18), rgba(255, 255, 255, 0.12), transparent);
  opacity: 0.7;
}

.xeno-page-content {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 10%),
    radial-gradient(circle at 100% 0%, rgba(56, 189, 248, 0.08), transparent 26%);
}

.xeno-page-atmosphere {
  overflow: hidden;
}

.xeno-page-scanline {
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(56, 189, 248, 0.4), transparent);
  box-shadow: 0 0 22px rgba(14, 165, 233, 0.3);
  opacity: 0.55;
}

.xeno-page-pulse {
  position: absolute;
  top: 3rem;
  right: -6rem;
  width: 18rem;
  height: 18rem;
  border-radius: 9999px;
  background: radial-gradient(circle, rgba(14, 165, 233, 0.12), transparent 70%);
  filter: blur(22px);
  opacity: 0.55;
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

.xeno-route-progress {
  height: 2px;
  opacity: 0;
  transform: scaleX(0.1);
  transform-origin: left center;
  background:
    linear-gradient(90deg, rgba(45, 212, 191, 0.2) 0%, rgba(14, 165, 233, 0.7) 45%, rgba(148, 163, 184, 0.2) 100%);
  box-shadow: 0 0 18px rgba(14, 165, 233, 0.38);
}

.xeno-route-progress-active {
  animation: xeno-route-progress-run 420ms cubic-bezier(0.22, 0.92, 0.3, 1) both;
}

.xeno-route-forward-enter-active,
.xeno-route-forward-leave-active,
.xeno-route-back-enter-active,
.xeno-route-back-leave-active {
  transition:
    opacity 0.34s cubic-bezier(0.22, 0.92, 0.3, 1),
    transform 0.34s cubic-bezier(0.22, 0.92, 0.3, 1),
    filter 0.34s cubic-bezier(0.22, 0.92, 0.3, 1);
}

.xeno-route-forward-enter-from {
  opacity: 0;
  transform: translate3d(18px, 12px, 0) scale(0.992);
  filter: blur(8px) saturate(108%);
}

.xeno-route-forward-leave-to {
  opacity: 0;
  transform: translate3d(-14px, -10px, 0) scale(0.996);
  filter: blur(7px) saturate(106%);
}

.xeno-route-back-enter-from {
  opacity: 0;
  transform: translate3d(-18px, 12px, 0) scale(0.992);
  filter: blur(8px) saturate(108%);
}

.xeno-route-back-leave-to {
  opacity: 0;
  transform: translate3d(14px, -10px, 0) scale(0.996);
  filter: blur(7px) saturate(106%);
}

.xeno-route-forward-enter-to,
.xeno-route-forward-leave-from,
.xeno-route-back-enter-to,
.xeno-route-back-leave-from {
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

@keyframes xeno-route-progress-run {
  0% {
    opacity: 0;
    transform: scaleX(0.08);
  }
  25% {
    opacity: 1;
    transform: scaleX(0.42);
  }
  100% {
    opacity: 0;
    transform: scaleX(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .xeno-shell-orbit,
  .xeno-shell-beam,
  .xeno-page-pulse,
  .xeno-app-booting::before,
  .xeno-route-curtain-active,
  .xeno-route-progress-active {
    animation: none !important;
  }

  .xeno-route-forward-enter-active,
  .xeno-route-forward-leave-active,
  .xeno-route-back-enter-active,
  .xeno-route-back-leave-active {
    transition-duration: 0.01ms !important;
  }

  .xeno-route-forward-enter-from,
  .xeno-route-forward-leave-to,
  .xeno-route-back-enter-from,
  .xeno-route-back-leave-to {
    opacity: 1;
    transform: none;
    filter: none;
  }
}
</style>
