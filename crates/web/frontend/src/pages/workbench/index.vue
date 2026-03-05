<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import BatchManageTab from './components/BatchManageTab.vue'
import PageHeader from '@/components/layout/PageHeader.vue'

const { t } = useI18n()
</script>

<template>
  <div class="xeno-analysis-shell relative z-0 flex h-full flex-col pt-4">
    <!-- Header -->
    <PageHeader
      :title="t('tools.title')"
      :description="t('tools.description')"
      icon="i-heroicons-rectangle-stack"
      icon-class="bg-primary-600 dark:bg-primary-500"
    />

    <!-- Content -->
    <div class="flex-1 overflow-auto p-6">
      <div class="xeno-panel rounded-2xl p-4 sm:p-5">
        <BatchManageTab />
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-analysis-shell {
  background: transparent;
  color: var(--xeno-text-main);
}
</style>
