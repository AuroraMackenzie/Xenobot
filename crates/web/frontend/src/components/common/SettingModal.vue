<script setup lang="ts">
import { ref, watch, computed, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLayoutStore } from '@/stores/layout'
import AISettingsTab from './settings/AISettingsTab.vue'
import BasicSettingsTab from './settings/BasicSettingsTab.vue'
import StorageTab from './settings/StorageTab.vue'
import AboutTab from './settings/AboutTab.vue'
import SubTabs from '@/components/UI/SubTabs.vue'

const { t } = useI18n()
const layoutStore = useLayoutStore()

// English engineering note.
interface ScrollableTab {
  scrollToSection?: (sectionId: string) => void
  refresh?: () => void
}

// Props
const props = defineProps<{
  open: boolean
}>()

// Emits
const emit = defineEmits<{
  'update:open': [value: boolean]
  'ai-config-saved': []
}>()

// English engineering note.
const tabs = computed(() => [
  { id: 'settings', label: t('settings.tabs.basic'), icon: 'i-heroicons-cog-6-tooth' },
  { id: 'ai', label: t('settings.tabs.ai'), icon: 'i-heroicons-sparkles' },
  { id: 'storage', label: t('settings.tabs.storage'), icon: 'i-heroicons-folder-open' },
  { id: 'about', label: t('settings.tabs.about'), icon: 'i-heroicons-information-circle' },
])

const activeTab = ref('settings')

// English engineering note.
const tabRefs = ref<Record<string, ScrollableTab | null>>({})

/**
 * English note.
 */
function setTabRef(tabId: string, el: unknown) {
  tabRefs.value[tabId] = el as ScrollableTab | null
}

// English engineering note.
function handleAIConfigChanged() {
  emit('ai-config-saved')
}

// English engineering note.
function closeModal() {
  emit('update:open', false)
  layoutStore.clearSettingTarget()
}

// English engineering note.
watch(
  () => props.open,
  async (newVal) => {
    if (newVal) {
      // English engineering note.
      const target = layoutStore.settingTarget
      if (target) {
        activeTab.value = target.tab
        // English engineering note.
        if (target.section) {
          await nextTick()
          // English engineering note.
          setTimeout(() => {
            const tabRef = tabRefs.value[target.tab]
            tabRef?.scrollToSection?.(target.section!)
          }, 100)
        }
      } else {
        activeTab.value = 'settings' // English engineering note.
      }
      // English engineering note.
      tabRefs.value['storage']?.refresh?.()
    } else {
      // English engineering note.
      layoutStore.clearSettingTarget()
    }
  }
)

// English engineering note.
watch(
  () => activeTab.value,
  (newTab) => {
    // English engineering note.
    tabRefs.value[newTab]?.refresh?.()
  }
)
</script>

<template>
  <UModal
    :open="open"
    :ui="{ overlay: 'app-region-no-drag', content: 'md:w-full max-w-5xl app-region-no-drag' }"
    @update:open="emit('update:open', $event)"
  >
    <template #content>
      <div class="xeno-setting-shell p-6">
        <!-- Header -->
        <div class="xeno-setting-header mb-4 flex items-center justify-between gap-4">
          <div class="min-w-0">
            <h2 class="break-words text-lg font-semibold text-gray-900 dark:text-white">{{ t('settings.title') }}</h2>
            <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
              {{ t('settings.description') }}
            </p>
          </div>
          <UButton icon="i-heroicons-x-mark" variant="ghost" size="sm" @click="closeModal" />
        </div>

        <div class="xeno-setting-tabs mb-6 -mx-6">
          <SubTabs v-model="activeTab" :items="tabs" />
        </div>

        <div class="xeno-setting-content h-[560px] overflow-y-auto">
          <!-- English UI note -->
          <div v-show="activeTab === 'settings'">
            <BasicSettingsTab />
          </div>

          <!-- English UI note -->
          <div v-show="activeTab === 'ai'" class="h-full">
            <AISettingsTab :ref="(el) => setTabRef('ai', el)" @config-changed="handleAIConfigChanged" />
          </div>

          <!-- English UI note -->
          <div v-show="activeTab === 'storage'" class="h-full">
            <StorageTab :ref="(el) => setTabRef('storage', el)" />
          </div>

          <!-- English UI note -->
          <div v-show="activeTab === 'about'">
            <AboutTab />
          </div>
        </div>
      </div>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-setting-shell {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.8rem;
  background:
    radial-gradient(circle at top left, rgba(84, 214, 255, 0.12), transparent 24%),
    radial-gradient(circle at top right, rgba(255, 122, 172, 0.1), transparent 20%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 24%),
    rgba(7, 18, 29, 0.95);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 34px 80px rgba(2, 8, 16, 0.38);
  backdrop-filter: blur(22px) saturate(136%);
}

.xeno-setting-header {
  padding-bottom: 1rem;
  border-bottom: 1px solid rgba(139, 166, 189, 0.16);
}

.xeno-setting-tabs {
  border-bottom: 1px solid rgba(139, 166, 189, 0.12);
}

.xeno-setting-content {
  border: 1px solid rgba(139, 166, 189, 0.14);
  border-radius: 1.25rem;
  padding: 1rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.58);
}
</style>
