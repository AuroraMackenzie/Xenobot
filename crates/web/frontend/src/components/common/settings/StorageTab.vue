<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import StorageManageSection from './DataStorage/StorageManageSection.vue'
import SessionIndexSection from './DataStorage/SessionIndexSection.vue'
import SubTabs from '@/components/UI/SubTabs.vue'
import { useSubTabsScroll } from '@/composables/useSubTabsScroll'

const { t } = useI18n()

const navItems = computed(() => [
  { id: 'storage', label: t('settings.tabs.storageManage') },
  { id: 'session', label: t('settings.tabs.sessionManage') },
])

const { activeNav, scrollContainerRef, setSectionRef, handleNavChange } = useSubTabsScroll(navItems)
void scrollContainerRef

const storageManageRef = ref<InstanceType<typeof StorageManageSection> | null>(null)

defineExpose({
  refresh: () => storageManageRef.value?.refresh(),
})
</script>

<template>
  <div class="flex h-full gap-6">
    <div class="xeno-storage-nav w-32 shrink-0 rounded-2xl p-3">
      <SubTabs v-model="activeNav" :items="navItems" orientation="vertical" @change="handleNavChange" />
    </div>

    <div ref="scrollContainerRef" class="xeno-storage-shell min-w-0 flex-1 overflow-y-auto rounded-2xl p-5">
      <div class="space-y-8">
        <div :ref="(el) => setSectionRef('storage', el as HTMLElement)">
          <StorageManageSection ref="storageManageRef" />
        </div>

        <div class="border-t border-white/10" />

        <div :ref="(el) => setSectionRef('session', el as HTMLElement)">
          <SessionIndexSection />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-storage-nav,
.xeno-storage-shell {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background:
    radial-gradient(circle at top right, rgba(59, 130, 246, 0.08), transparent 26%),
    linear-gradient(180deg, rgba(15, 23, 42, 0.72), rgba(15, 23, 42, 0.62));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 38px rgba(2, 6, 23, 0.18);
  backdrop-filter: blur(18px);
}
</style>
