<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { useDebounceFn } from "@vueuse/core";

const props = defineProps<{
  open: boolean;
  sessionId: string;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  completed: [];
}>();

const { t, locale } = useI18n();

// English engineering note.
const isOpen = computed({
  get: () => props.open,
  set: (val) => emit("update:open", val),
});

// English engineering note.
type QueryMode = "time" | "range";
const queryMode = ref<QueryMode>("range");

// English engineering note.
const rangePercent = ref(50);
const totalSessionCount = ref(0); // English engineering note.

// English engineering note.
type TimeRangePreset = "today" | "yesterday" | "week" | "month" | "custom";
const selectedPreset = ref<TimeRangePreset>("today");

// English engineering note.
const customStartDate = ref<string>("");
const customEndDate = ref<string>("");

// English engineering note.
interface SessionItem {
  id: number;
  startTs: number;
  endTs: number;
  messageCount: number;
  summary: string | null;
}
const sessions = ref<SessionItem[]>([]);
const isLoading = ref(false);

// English engineering note.
const isGenerating = ref(false);
const currentIndex = ref(0);
const totalToGenerate = ref(0); // English engineering note.
const results = ref<
  Array<{
    id: number;
    status: "success" | "failed" | "skipped";
    message?: string;
    summary?: string;
  }>
>([]);
const shouldStop = ref(false);

// English engineering note.
function isTooFewMessagesError(error: string): boolean {
  return (
    error.includes("少于3条") ||
    error.includes("less than 3") ||
    error.includes("无需生成摘要")
  );
}

// English engineering note.
const resultsContainer = ref<HTMLElement | null>(null);

// English engineering note.
const timeRange = computed(() => {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  switch (selectedPreset.value) {
    case "today":
      return {
        start: today.getTime(),
        end: now.getTime(),
      };
    case "yesterday": {
      const yesterday = new Date(today);
      yesterday.setDate(yesterday.getDate() - 1);
      return {
        start: yesterday.getTime(),
        end: today.getTime() - 1,
      };
    }
    case "week": {
      const weekAgo = new Date(today);
      weekAgo.setDate(weekAgo.getDate() - 7);
      return {
        start: weekAgo.getTime(),
        end: now.getTime(),
      };
    }
    case "month": {
      const monthAgo = new Date(today);
      monthAgo.setMonth(monthAgo.getMonth() - 1);
      return {
        start: monthAgo.getTime(),
        end: now.getTime(),
      };
    }
    case "custom":
      if (customStartDate.value && customEndDate.value) {
        const startDate = new Date(customStartDate.value);
        const endDate = new Date(customEndDate.value);
        return {
          start: startDate.getTime(),
          end: new Date(endDate.getTime() + 24 * 60 * 60 * 1000 - 1).getTime(), // English engineering note.
        };
      }
      return null;
    default:
      return null;
  }
});

// English engineering note.
const canGenerateMap = ref<
  Record<number, { canGenerate: boolean; reason?: string }>
>({});
const isChecking = ref(false);

// English engineering note.
const pendingSessions = computed(() => {
  return sessions.value.filter((s) => {
    if (s.summary) return false;
    const checkResult = canGenerateMap.value[s.id];
    return checkResult?.canGenerate !== false;
  });
});

// English engineering note.
const existingSummaryCount = computed(() => {
  return sessions.value.filter((s) => s.summary).length;
});

// English engineering note.
const tooFewMessagesCount = computed(() => {
  return sessions.value.filter((s) => {
    if (s.summary) return false;
    const checkResult = canGenerateMap.value[s.id];
    return checkResult?.canGenerate === false;
  }).length;
});

// English engineering note.
const progressPercent = computed(() => {
  if (totalToGenerate.value === 0) return 100;
  return Math.round((currentIndex.value / totalToGenerate.value) * 100);
});

// English engineering note.
const stats = computed(() => {
  const success = results.value.filter((r) => r.status === "success").length;
  const failed = results.value.filter((r) => r.status === "failed").length;
  const skipped = results.value.filter((r) => r.status === "skipped").length;
  return { success, failed, skipped };
});

// English engineering note.
async function fetchSessions() {
  isLoading.value = true;
  canGenerateMap.value = {};

  try {
    if (queryMode.value === "range") {
      // English engineering note.
      const allSessions = await window.sessionApi.getSessions(props.sessionId);
      totalSessionCount.value = allSessions.length;
      const count = Math.ceil(allSessions.length * (rangePercent.value / 100));
      // English engineering note.
      sessions.value = allSessions.slice(-count);
    } else {
      // English engineering note.
      if (!timeRange.value) {
        sessions.value = [];
        return;
      }
      // English engineering note.
      const startTs = Math.floor(timeRange.value.start / 1000);
      const endTs = Math.floor(timeRange.value.end / 1000);

      sessions.value = await window.sessionApi.getByTimeRange(
        props.sessionId,
        startTs,
        endTs,
      );
    }

    // English engineering note.
    if (sessions.value.length > 0) {
      await checkCanGenerate();
    }
  } catch (error) {
    console.error("[BatchSummaryModal] Failed to query sessions:", error);
    sessions.value = [];
  } finally {
    isLoading.value = false;
  }
}

// English engineering note.
async function checkCanGenerate() {
  const noSummaryIds = sessions.value
    .filter((s) => !s.summary)
    .map((s) => s.id);
  if (noSummaryIds.length === 0) return;

  isChecking.value = true;
  try {
    canGenerateMap.value = await window.sessionApi.checkCanGenerateSummary(
      props.sessionId,
      noSummaryIds,
    );
  } catch (error) {
    console.error(
      "[BatchSummaryModal] Failed to check summary availability:",
      error,
    );
  } finally {
    isChecking.value = false;
  }
}

// English engineering note.
const debouncedFetchSessions = useDebounceFn(() => {
  fetchSessions();
}, 300);

// English engineering note.
watch(
  () => [
    queryMode.value,
    selectedPreset.value,
    customStartDate.value,
    customEndDate.value,
  ],
  () => {
    if (queryMode.value === "range") {
      fetchSessions();
    } else if (
      selectedPreset.value !== "custom" ||
      (customStartDate.value && customEndDate.value)
    ) {
      fetchSessions();
    }
  },
  { immediate: true },
);

// English engineering note.
watch(
  () => rangePercent.value,
  () => {
    if (queryMode.value === "range") {
      debouncedFetchSessions();
    }
  },
);

// English engineering note.
watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      // English engineering note.
      isGenerating.value = false;
      currentIndex.value = 0;
      results.value = [];
      shouldStop.value = false;
      fetchSessions();
    }
  },
);

// English engineering note.
async function startGenerate() {
  // English engineering note.
  const sessionsToProcess = [...pendingSessions.value];
  if (sessionsToProcess.length === 0) return;

  isGenerating.value = true;
  shouldStop.value = false;
  currentIndex.value = 0;
  totalToGenerate.value = sessionsToProcess.length;
  results.value = [];

  try {
    for (const session of sessionsToProcess) {
      if (shouldStop.value) break;

      try {
        const result = await window.sessionApi.generateSummary(
          props.sessionId,
          session.id,
          locale.value,
          false,
        );

        if (result.success) {
          // English engineering note.
          results.value.push({
            id: session.id,
            status: "success",
            summary: result.summary || "",
          });
          // English engineering note.
          const idx = sessions.value.findIndex((s) => s.id === session.id);
          if (idx !== -1) {
            sessions.value[idx].summary = result.summary || "";
          }
        } else if (result.error && isTooFewMessagesError(result.error)) {
          // English engineering note.
          results.value.push({
            id: session.id,
            status: "skipped",
            message: result.error,
          });
        } else {
          // English engineering note.
          results.value.push({
            id: session.id,
            status: "failed",
            message: result.error,
          });
        }
      } catch (error) {
        results.value.push({
          id: session.id,
          status: "failed",
          message: String(error),
        });
      }

      currentIndex.value++;

      // English engineering note.
      await nextTick();
      if (resultsContainer.value) {
        resultsContainer.value.scrollTop = resultsContainer.value.scrollHeight;
      }
    }
  } finally {
    // English engineering note.
    isGenerating.value = false;
  }

  // English engineering note.
  if (stats.value.success > 0) {
    emit("completed");
  }
}

// English engineering note.
function stopGenerate() {
  shouldStop.value = true;
}

// English engineering note.
function close() {
  if (isGenerating.value) {
    shouldStop.value = true;
  }
  emit("update:open", false);
}

// English engineering note.
</script>

<template>
  <UModal
    v-model:open="isOpen"
    :ui="{ overlay: 'z-[10001]', content: 'z-[10001] max-w-4xl' }"
  >
    <template #content>
      <UCard class="xeno-batch-summary-card">
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div class="min-w-0">
              <h3 class="break-words text-lg font-semibold">
                {{ t("records.batchSummary.title") }}
              </h3>
              <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                {{ t("records.batchSummary.description") }}
              </p>
            </div>
            <UButton
              color="neutral"
              variant="ghost"
              icon="i-heroicons-x-mark"
              size="sm"
              @click="close"
            />
          </div>
        </template>

        <div class="space-y-4">
          <!-- English UI note -->
          <div
            class="flex flex-wrap gap-2 border-b border-gray-200 pb-3 dark:border-gray-700"
          >
            <UButton
              :color="queryMode === 'range' ? 'primary' : 'neutral'"
              :variant="queryMode === 'range' ? 'solid' : 'ghost'"
              size="sm"
              :disabled="isGenerating"
              @click="queryMode = 'range'"
            >
              {{ t("records.batchSummary.byRange") }}
            </UButton>
            <UButton
              :color="queryMode === 'time' ? 'primary' : 'neutral'"
              :variant="queryMode === 'time' ? 'solid' : 'ghost'"
              size="sm"
              :disabled="isGenerating"
              @click="queryMode = 'time'"
            >
              {{ t("records.batchSummary.byTime") }}
            </UButton>
          </div>

          <!-- English UI note -->
          <div
            v-if="queryMode === 'range'"
            class="xeno-batch-summary-panel rounded-xl p-4"
          >
            <label
              class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
            >
              {{ t("records.batchSummary.selectRange") }}
            </label>
            <div class="space-y-3">
              <div class="flex items-center gap-4">
                <USlider
                  v-model="rangePercent"
                  :min="1"
                  :max="100"
                  :step="1"
                  :disabled="isGenerating"
                  class="flex-1"
                />
                <span
                  class="text-lg font-semibold text-primary-600 dark:text-primary-400 min-w-[4rem] text-right"
                >
                  {{ rangePercent }}%
                </span>
              </div>
              <div class="text-xs text-gray-500 flex justify-between">
                <span>{{ t("records.batchSummary.rangeStart", "最早") }}</span>
                <span v-if="totalSessionCount > 0">
                  {{ t("records.batchSummary.rangeInfo") }}
                  {{ Math.ceil((totalSessionCount * rangePercent) / 100) }} /
                  {{ totalSessionCount }}
                  {{ t("records.batchSummary.sessionsUnit") }}
                </span>
                <span>{{ t("records.batchSummary.rangeEnd") }}</span>
              </div>
            </div>
          </div>

          <!-- English UI note -->
          <div
            v-else-if="queryMode === 'time'"
            class="xeno-batch-summary-panel rounded-xl p-4"
          >
            <label
              class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
            >
              {{ t("records.batchSummary.timeRange") }}
            </label>
            <div class="flex flex-wrap gap-2">
              <UButton
                v-for="preset in [
                  { key: 'today', label: t('records.batchSummary.today') },
                  {
                    key: 'yesterday',
                    label: t('records.batchSummary.yesterday'),
                  },
                  { key: 'week', label: t('records.batchSummary.week') },
                  { key: 'month', label: t('records.batchSummary.month') },
                  { key: 'custom', label: t('records.batchSummary.custom') },
                ]"
                :key="preset.key"
                :color="selectedPreset === preset.key ? 'primary' : 'neutral'"
                :variant="selectedPreset === preset.key ? 'solid' : 'outline'"
                size="sm"
                :disabled="isGenerating"
                @click="selectedPreset = preset.key as TimeRangePreset"
              >
                {{ preset.label }}
              </UButton>
            </div>

            <!-- English UI note -->
            <div
              v-if="selectedPreset === 'custom'"
              class="mt-3 flex flex-wrap items-center gap-2"
            >
              <UInput
                v-model="customStartDate"
                type="date"
                :disabled="isGenerating"
                size="sm"
                class="min-w-[12rem]"
              />
              <span class="text-gray-500">—</span>
              <UInput
                v-model="customEndDate"
                type="date"
                :disabled="isGenerating"
                size="sm"
                class="min-w-[12rem]"
              />
            </div>
          </div>

          <!-- English UI note -->
          <div
            v-if="!isLoading && !isChecking"
            class="xeno-batch-summary-panel rounded-xl p-4 text-sm text-gray-600 dark:text-gray-400"
          >
            <template v-if="sessions.length > 0">
              <p>
                {{ t("records.batchSummary.found") }} {{ sessions.length }}
                {{ t("records.batchSummary.sessionsUnit") }}
                <template
                  v-if="existingSummaryCount > 0 || tooFewMessagesCount > 0"
                >
                  <span class="text-gray-500">
                    （
                    <template v-if="existingSummaryCount > 0">
                      <span class="text-green-600 dark:text-green-400">
                        {{ existingSummaryCount }}
                        {{ t("records.batchSummary.hasSummary") }}
                      </span>
                    </template>
                    <template
                      v-if="existingSummaryCount > 0 && tooFewMessagesCount > 0"
                      >，</template
                    >
                    <template v-if="tooFewMessagesCount > 0">
                      <span class="text-gray-400">
                        {{ tooFewMessagesCount }}
                        {{ t("records.batchSummary.tooFewMessages") }}
                      </span>
                    </template>
                    ）
                  </span>
                </template>
              </p>
              <p v-if="pendingSessions.length > 0" class="mt-1 font-medium">
                {{ t("records.batchSummary.pending") }}
                {{ pendingSessions.length }}
                {{ t("records.batchSummary.unit") }}
              </p>
              <p v-else class="mt-1 text-gray-400">
                {{ t("records.batchSummary.noPending") }}
              </p>
            </template>
            <p v-else class="text-gray-400">
              {{ t("records.batchSummary.noSessions") }}
            </p>
          </div>
          <div
            v-else-if="isChecking"
            class="xeno-batch-summary-panel flex items-center gap-2 rounded-xl p-4 text-sm text-gray-500"
          >
            <UIcon name="i-heroicons-arrow-path" class="animate-spin" />
            {{ t("records.batchSummary.checking") }}
          </div>
          <div
            v-else
            class="xeno-batch-summary-panel flex items-center gap-2 rounded-xl p-4 text-sm text-gray-500"
          >
            <UIcon name="i-heroicons-arrow-path" class="animate-spin" />
            {{ t("records.batchSummary.loading") }}
          </div>

          <!-- English UI note -->
          <div v-if="isGenerating || results.length > 0" class="space-y-2">
            <div class="flex items-center justify-between text-sm">
              <span>{{ t("records.batchSummary.progress") }}</span>
              <span
                >{{ currentIndex }} /
                {{ totalToGenerate || pendingSessions.length }}</span
              >
            </div>
            <!-- English UI note -->
            <UProgress v-if="isGenerating" :value="progressPercent" />
            <!-- English UI note -->
            <div v-else class="h-2 w-full rounded-full bg-green-500" />
          </div>

          <!-- English UI note -->
          <div
            v-if="results.length > 0"
            ref="resultsContainer"
            class="xeno-batch-summary-results max-h-64 overflow-y-auto rounded"
          >
            <div
              v-for="result in results"
              :key="result.id"
              class="flex flex-col gap-1 border-b border-gray-200 px-3 py-2 text-sm last:border-b-0 dark:border-gray-700"
            >
              <!-- English UI note -->
              <div class="flex items-center gap-2">
                <UIcon
                  :name="
                    result.status === 'success'
                      ? 'i-heroicons-check-circle'
                      : result.status === 'skipped'
                        ? 'i-heroicons-minus-circle'
                        : 'i-heroicons-x-circle'
                  "
                  class="flex-shrink-0"
                  :class="{
                    'text-green-500': result.status === 'success',
                    'text-gray-400': result.status === 'skipped',
                    'text-red-500': result.status === 'failed',
                  }"
                />
                <span class="flex-1 font-medium"
                  >{{ t("records.batchSummary.session") }} #{{
                    result.id
                  }}</span
                >
                <span
                  class="flex-shrink-0 text-xs"
                  :class="{
                    'text-green-600 dark:text-green-400':
                      result.status === 'success',
                    'text-gray-500': result.status === 'skipped',
                    'text-red-600 dark:text-red-400':
                      result.status === 'failed',
                  }"
                >
                  {{
                    result.status === "success"
                      ? t("records.batchSummary.statusSuccess")
                      : result.status === "skipped"
                        ? t("records.batchSummary.statusSkipped")
                        : t("records.batchSummary.statusFailed")
                  }}
                </span>
              </div>
              <div
                v-if="result.summary"
                class="break-words pl-6 text-xs text-gray-600 dark:text-gray-400 line-clamp-2"
              >
                {{ result.summary }}
              </div>
              <div
                v-else-if="result.status === 'failed' && result.message"
                class="break-words pl-6 text-xs text-red-500"
              >
                {{ result.message }}
              </div>
              <div
                v-else-if="result.status === 'skipped'"
                class="pl-6 text-xs text-gray-400 italic"
              >
                {{ t("records.batchSummary.tooFewMessages") }}
              </div>
            </div>
          </div>

          <!-- English UI note -->
          <div
            v-if="!isGenerating && results.length > 0"
            class="flex items-center gap-4 text-sm"
          >
            <span class="text-green-600 dark:text-green-400">
              <UIcon name="i-heroicons-check-circle" class="mr-1" />
              {{ t("records.batchSummary.success") }} {{ stats.success }}
            </span>
            <span
              v-if="stats.failed > 0"
              class="text-red-600 dark:text-red-400"
            >
              <UIcon name="i-heroicons-x-circle" class="mr-1" />
              {{ t("records.batchSummary.failed") }} {{ stats.failed }}
            </span>
            <span v-if="stats.skipped > 0" class="text-gray-500">
              <UIcon name="i-heroicons-minus-circle" class="mr-1" />
              {{ t("records.batchSummary.skipped") }} {{ stats.skipped }}
            </span>
          </div>
        </div>

        <template #footer>
          <div class="flex justify-end gap-2">
            <UButton
              color="neutral"
              variant="outline"
              :disabled="isGenerating"
              @click="close"
            >
              {{ t("common.close", "关闭") }}
            </UButton>
            <UButton
              v-if="!isGenerating"
              color="primary"
              :disabled="pendingSessions.length === 0 || isLoading"
              @click="startGenerate"
            >
              {{ t("records.batchSummary.start") }}
            </UButton>
            <UButton v-else color="error" @click="stopGenerate">
              {{ t("records.batchSummary.stop") }}
            </UButton>
          </div>
        </template>
      </UCard>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-batch-summary-card {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.6rem;
  background:
    radial-gradient(
      circle at top left,
      rgba(84, 214, 255, 0.12),
      transparent 24%
    ),
    radial-gradient(
      circle at top right,
      rgba(255, 122, 172, 0.08),
      transparent 18%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 22%),
    rgba(7, 18, 29, 0.95);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 30px 72px rgba(2, 8, 16, 0.36);
  backdrop-filter: blur(22px) saturate(134%);
}

.xeno-batch-summary-panel {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.54);
}

.xeno-batch-summary-results {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.48);
}
</style>
