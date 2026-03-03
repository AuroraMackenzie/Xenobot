<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import StorageManageSection from './DataStorage/StorageManageSection.vue'
import SessionIndexSection from './DataStorage/SessionIndexSection.vue'
import SubTabs from '@/components/UI/SubTabs.vue'
import { useSubTabsScroll } from '@/composables/useSubTabsScroll'

const { t } = useI18n()

// English engineering note.
const navItems = computed(() => [
  { id: 'storage', label: t('settings.tabs.storageManage') },
  { id: 'session', label: t('settings.tabs.sessionManage') },
])

// English engineering note.
const { activeNav, scrollContainerRef, setSectionRef, handleNavChange } = useSubTabsScroll(navItems)
void scrollContainerRef // English engineering note.

// Template refs
const storageManageRef = ref<InstanceType<typeof StorageManageSection> | null>(null)

// English engineering note.
defineExpose({
  refresh: () => storageManageRef.value?.refresh(),
})
</script>

<template>
  <div class="flex h-full gap-6">
    <!-- English UI note -->
    <div class="w-28 shrink-0">
      <SubTabs v-model="activeNav" :items="navItems" orientation="vertical" @change="handleNavChange" />
    </div>

    <!-- English UI note -->
    <div ref="scrollContainerRef" class="min-w-0 flex-1 overflow-y-auto">
      <div class="space-y-8">
        <!-- English UI note -->
        <div :ref="(el) => setSectionRef('storage', el as HTMLElement)">
          <StorageManageSection ref="storageManageRef" />
        </div>

        <!-- English UI note -->
        <div class="border-t border-gray-200 dark:border-gray-700" />

        <!-- English UI note -->
        <div :ref="(el) => setSectionRef('session', el as HTMLElement)">
          <SessionIndexSection />
        </div>
      </div>
    </div>
  </div>
</template>
