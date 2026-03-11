<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { MemberNameHistory } from "@/types/analysis";

const { t } = useI18n();

const props = defineProps<{
  history: MemberNameHistory[];
  // English engineering note.
  compact?: boolean;
}>();

/**
 * English note.
 * English note.
 */
function formatDate(ts: number): string {
  const date = new Date(ts * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * English note.
 */
function formatPeriod(startTs: number, endTs: number | null): string {
  const start = formatDate(startTs);
  if (endTs === null) {
    return t("views.charts.nicknameHistory.periodToNow", { start });
  }
  const end = formatDate(endTs);
  if (start === end) {
    return start;
  }
  return t("views.charts.nicknameHistory.periodRange", { start, end });
}

/**
 * English note.
 */
function isCurrent(item: MemberNameHistory): boolean {
  return item.endTs === null;
}

/**
 * English note.
 */
const hasHistory = computed(() => props.history.length > 0);

/**
 * English note.
 */
const singleNickname = computed(() => props.history.length === 1);
</script>

<template>
  <div v-if="hasHistory" :class="compact ? 'py-1' : 'py-2'">
    <!-- English UI note -->
    <div v-if="singleNickname" class="flex items-center gap-1 text-sm">
      <span class="text-gray-900 dark:text-white">{{ history[0].name }}</span>
      <span class="text-xs text-gray-500 dark:text-gray-400">
        （{{ formatPeriod(history[0].startTs, history[0].endTs) }}）
      </span>
    </div>

    <!-- English UI note -->
    <div v-else class="space-y-0">
      <div v-for="(item, index) in history" :key="index" class="flex gap-3">
        <!-- English UI note -->
        <div class="flex flex-col items-center">
          <div
            class="mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full"
            :class="
              isCurrent(item) ? 'bg-[#0a88ac]' : 'bg-gray-300 dark:bg-gray-600'
            "
          />
          <div
            v-if="index < history.length - 1"
            class="h-full min-h-[24px] w-px grow bg-gray-200 dark:bg-gray-700"
          />
        </div>

        <!-- English UI note -->
        <div :class="compact ? 'pb-2' : 'pb-4'" class="flex-1">
          <div class="flex items-center gap-2">
            <span
              class="text-gray-900 dark:text-white"
              :class="{ 'font-semibold text-[#0a88ac]': isCurrent(item) }"
            >
              {{ item.name }}
            </span>
            <UBadge
              v-if="isCurrent(item)"
              color="primary"
              variant="soft"
              size="xs"
            >
              {{ t("views.charts.nicknameHistory.current") }}
            </UBadge>
          </div>
          <div class="mt-0.5">
            <span class="text-xs text-gray-500 dark:text-gray-400">{{
              formatPeriod(item.startTs, item.endTs)
            }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- English UI note -->
  <div v-else class="py-4 text-center">
    <span class="text-sm text-gray-400">{{
      t("views.charts.nicknameHistory.empty")
    }}</span>
  </div>
</template>
