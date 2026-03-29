<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();

interface TimeFilter {
  startTs?: number;
  endTs?: number;
}

// Props
defineProps<{
  sessionId: string;
  timeFilter?: TimeFilter;
}>();
</script>

<template>
  <div class="main-content xeno-analysis-empty-state">
    <div class="xeno-analysis-empty-panel">
      <div class="text-center">
        <UIcon
          name="i-heroicons-chart-bar"
          class="xeno-analysis-empty-icon mx-auto h-12 w-12"
        />
        <p class="xeno-analysis-empty-title mt-3 text-sm font-medium">
          {{ t("views.timeline.title") }}
        </p>
        <p class="xeno-analysis-empty-copy mt-1 max-w-md px-4 text-sm">
          {{ t("views.timeline.description") }}
        </p>
      </div>
    </div>
  </div>
</template>
