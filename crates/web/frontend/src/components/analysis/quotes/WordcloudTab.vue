<script setup lang="ts">
import { ref, watch, computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { EChartWordcloud } from "@/components/charts";
import type { EChartWordcloudData } from "@/components/charts";
import { LoadingState, EmptyState, UITabs } from "@/components/UI";
import UserSelect from "@/components/common/UserSelect.vue";
import { useSettingsStore } from "@/stores/settings";
import { useLayoutStore } from "@/stores/layout";

const { t } = useI18n();
const settingsStore = useSettingsStore();
const layoutStore = useLayoutStore();

interface TimeFilter {
  startTs?: number;
  endTs?: number;
}

interface PosTagInfo {
  tag: string;
  name: string;
  description: string;
  meaningful: boolean;
}

type PosFilterMode = "all" | "meaningful" | "custom";

const props = defineProps<{
  sessionId: string;
  timeFilter?: TimeFilter;
  memberId?: number | null;
}>();

// English engineering note.
const isLoading = ref(false);
const wordcloudData = ref<EChartWordcloudData>({ words: [] });
const stats = ref({
  totalMessages: 0,
  totalWords: 0,
  uniqueWords: 0,
});

// English engineering note.
const colorScheme = "default" as const;

// English engineering note.
const sizeScale = ref(1.25);

// English engineering note.
const maxWords = ref(150);

// English engineering note.
const posFilterMode = ref<PosFilterMode>("meaningful");

// English engineering note.
const enableStopwords = ref(true);

// English engineering note.
const customPosTags = ref<string[]>([]);

// English engineering note.
const posTagDefinitions = ref<PosTagInfo[]>([]);

// English engineering note.
const posTagStats = ref<Map<string, number>>(new Map());

// English engineering note.
const selectedMemberId = ref<number | null>(null);

// English engineering note.
const locale = computed(() => settingsStore.locale as "zh-CN" | "en-US");

// English engineering note.
const posFilterModeOptions = computed(() => [
  { label: t("quotes.wordcloud.posFilter.all"), value: "all" },
  { label: t("quotes.wordcloud.posFilter.meaningful"), value: "meaningful" },
  { label: t("quotes.wordcloud.posFilter.custom"), value: "custom" },
]);

// English engineering note.
const maxWordsOptions = [
  { label: "80", value: 80 },
  { label: "100", value: 100 },
  { label: "150", value: 150 },
  { label: "200", value: 200 },
  { label: "300", value: 300 },
];

// English engineering note.
const sizeScaleOptions = computed(() => [
  { label: t("quotes.wordcloud.size.small"), value: 0.75 },
  { label: t("quotes.wordcloud.size.medium"), value: 1 },
  { label: t("quotes.wordcloud.size.large"), value: 1.25 },
  { label: t("quotes.wordcloud.size.xlarge"), value: 1.5 },
]);

// English engineering note.
const posTagOptions = computed(() =>
  posTagDefinitions.value.map((p) => ({
    label: p.name,
    tag: p.tag,
    value: p.tag,
    count: posTagStats.value.get(p.tag) || 0,
    meaningful: p.meaningful,
  })),
);

// English engineering note.
async function loadPosTagDefinitions() {
  try {
    const tags = await window.nlpApi.getPosTags();
    posTagDefinitions.value = tags;
    // English engineering note.
    customPosTags.value = tags.filter((t) => t.meaningful).map((t) => t.tag);
  } catch (error) {
    console.error("[WordcloudTab] Failed to load POS tag definitions:", error);
  }
}

// English engineering note.
async function loadWordFrequency() {
  if (!props.sessionId) return;

  isLoading.value = true;
  try {
    const result = await window.nlpApi.getWordFrequency({
      sessionId: props.sessionId,
      locale: locale.value,
      timeFilter: props.timeFilter
        ? { startTs: props.timeFilter.startTs, endTs: props.timeFilter.endTs }
        : undefined,
      memberId: selectedMemberId.value ?? undefined,
      topN: maxWords.value,
      minCount: 2,
      posFilterMode: posFilterMode.value,
      customPosTags:
        posFilterMode.value === "custom" ? [...customPosTags.value] : undefined,
      enableStopwords: enableStopwords.value,
    });

    wordcloudData.value = {
      words: result.words.map((w) => ({
        word: w.word,
        count: w.count,
        percentage: w.percentage,
      })),
    };

    stats.value = {
      totalMessages: result.totalMessages,
      totalWords: result.totalWords,
      uniqueWords: result.uniqueWords,
    };

    // English engineering note.
    if (result.posTagStats) {
      const statsMap = new Map<string, number>();
      for (const stat of result.posTagStats) {
        statsMap.set(stat.tag, stat.count);
      }
      posTagStats.value = statsMap;
    }
  } catch (error) {
    console.error("[WordcloudTab] Failed to load word frequency data:", error);
    wordcloudData.value = { words: [] };
  } finally {
    isLoading.value = false;
  }
}

// English engineering note.
watch(
  () => [
    props.sessionId,
    props.timeFilter,
    selectedMemberId.value,
    maxWords.value,
    posFilterMode.value,
    enableStopwords.value,
  ],
  () => {
    loadWordFrequency();
  },
  { immediate: true, deep: true },
);

// English engineering note.
watch(
  customPosTags,
  () => {
    if (posFilterMode.value === "custom") {
      loadWordFrequency();
    }
  },
  { deep: true },
);

// English engineering note.
watch(locale, () => {
  loadWordFrequency();
});

// English engineering note.
function handleWordClick(word: string) {
  layoutStore.openChatRecordDrawer({
    keywords: [word],
  });
}

// English engineering note.
onMounted(() => {
  loadPosTagDefinitions();
});
</script>

<template>
  <div class="xeno-wordcloud-shell main-content mx-auto max-w-6xl py-6">
    <div class="flex gap-6">
      <!-- English UI note -->
      <div
        class="xeno-wordcloud-stage flex-1 min-w-0 space-y-4 rounded-2xl p-5"
      >
        <!-- English UI note -->
        <div class="relative w-full" style="aspect-ratio: 16 / 9">
          <!-- English UI note -->
          <LoadingState
            v-if="isLoading"
            :text="t('quotes.wordcloud.loading')"
            class="absolute inset-0 z-10 rounded-lg bg-white/80 dark:bg-gray-900/80"
          />

          <!-- English UI note -->
          <EmptyState
            v-else-if="wordcloudData.words.length === 0"
            icon="i-heroicons-cloud"
            :title="t('quotes.wordcloud.empty.title')"
            :description="t('quotes.wordcloud.empty.description')"
            class="h-full"
          />

          <!-- English UI note -->
          <EChartWordcloud
            v-else
            :data="wordcloudData"
            height="100%"
            :max-words="maxWords"
            :color-scheme="colorScheme"
            :size-scale="sizeScale"
            :loading="isLoading"
            @word-click="handleWordClick"
          />
        </div>

        <!-- English UI note -->
        <div class="flex items-center justify-center gap-8 py-3">
          <div class="flex items-center gap-2">
            <UIcon
              name="i-heroicons-chat-bubble-left-right"
              class="text-lg text-primary-500"
            />
            <div class="text-center">
              <div class="text-2xl font-bold text-gray-900 dark:text-white">
                {{ stats.totalMessages.toLocaleString() }}
              </div>
              <div class="text-xs text-gray-500 dark:text-gray-400">
                {{ t("quotes.wordcloud.stats.messagesLabel") }}
              </div>
            </div>
          </div>
          <div class="h-8 w-px bg-gray-200 dark:bg-gray-700" />
          <div class="flex items-center gap-2">
            <UIcon
              name="i-heroicons-document-text"
              class="text-lg text-emerald-500"
            />
            <div class="text-center">
              <div class="text-2xl font-bold text-gray-900 dark:text-white">
                {{ stats.totalWords.toLocaleString() }}
              </div>
              <div class="text-xs text-gray-500 dark:text-gray-400">
                {{ t("quotes.wordcloud.stats.wordsLabel") }}
              </div>
            </div>
          </div>
          <div class="h-8 w-px bg-gray-200 dark:bg-gray-700" />
          <div class="flex items-center gap-2">
            <UIcon name="i-heroicons-sparkles" class="text-lg text-amber-500" />
            <div class="text-center">
              <div class="text-2xl font-bold text-gray-900 dark:text-white">
                {{ stats.uniqueWords.toLocaleString() }}
              </div>
              <div class="text-xs text-gray-500 dark:text-gray-400">
                {{ t("quotes.wordcloud.stats.uniqueLabel") }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- English UI note -->
      <div
        class="xeno-wordcloud-sidebar w-[300px] shrink-0 space-y-4 rounded-2xl p-4"
      >
        <!-- English UI note -->
        <div>
          <h4 class="mb-2 text-xs font-medium text-gray-600 dark:text-gray-400">
            {{ t("quotes.wordcloud.config.maxWords") }}
          </h4>
          <UITabs v-model="maxWords" size="xs" :items="maxWordsOptions" />
        </div>

        <!-- English UI note -->
        <div>
          <h4 class="mb-2 text-xs font-medium text-gray-600 dark:text-gray-400">
            {{ t("quotes.wordcloud.config.sizeScale") }}
          </h4>
          <UITabs v-model="sizeScale" size="xs" :items="sizeScaleOptions" />
        </div>

        <!-- English UI note -->
        <div>
          <h4 class="mb-2 text-xs font-medium text-gray-600 dark:text-gray-400">
            {{ t("quotes.wordcloud.config.userFilter") }}
          </h4>
          <UserSelect
            v-model="selectedMemberId"
            :session-id="props.sessionId"
            class="w-full"
          />
        </div>

        <!-- English UI note -->
        <div>
          <h4 class="mb-2 text-xs font-medium text-gray-600 dark:text-gray-400">
            {{ t("quotes.wordcloud.config.posFilter") }}
          </h4>
          <UITabs
            v-model="posFilterMode"
            size="xs"
            :items="posFilterModeOptions"
          />
        </div>

        <!-- English UI note -->
        <div class="flex items-center">
          <UCheckbox
            v-model="enableStopwords"
            :label="t('quotes.wordcloud.config.enableStopwords')"
          />
        </div>

        <!-- English UI note -->
        <div v-if="posFilterMode === 'custom'" class="space-y-2">
          <div class="flex items-center justify-between">
            <h4 class="text-xs font-medium text-gray-600 dark:text-gray-400">
              {{ t("quotes.wordcloud.posFilter.customHint") }}
            </h4>
            <!-- English UI note -->
            <div class="flex gap-1">
              <UButton
                size="xs"
                variant="ghost"
                color="neutral"
                @click="
                  customPosTags = posTagDefinitions
                    .filter((t) => t.meaningful)
                    .map((t) => t.tag)
                "
              >
                {{ t("quotes.wordcloud.posFilter.selectMeaningful") }}
              </UButton>
              <UButton
                size="xs"
                variant="ghost"
                color="neutral"
                @click="customPosTags = posTagDefinitions.map((t) => t.tag)"
              >
                {{ t("quotes.wordcloud.posFilter.selectAll") }}
              </UButton>
              <UButton
                size="xs"
                variant="ghost"
                color="neutral"
                @click="customPosTags = []"
              >
                {{ t("quotes.wordcloud.posFilter.clearAll") }}
              </UButton>
            </div>
          </div>
          <!-- English UI note -->
          <div class="flex flex-wrap gap-1.5 max-h-[360px] overflow-y-auto">
            <UBadge
              v-for="tag in posTagOptions"
              :key="tag.value"
              :color="customPosTags.includes(tag.value) ? 'primary' : 'neutral'"
              :variant="customPosTags.includes(tag.value) ? 'solid' : 'outline'"
              class="cursor-pointer select-none transition-colors"
              @click="
                () => {
                  if (customPosTags.includes(tag.value)) {
                    customPosTags = customPosTags.filter(
                      (t) => t !== tag.value,
                    );
                  } else {
                    customPosTags = [...customPosTags, tag.value];
                  }
                }
              "
            >
              {{ tag.label }}
              <span v-if="tag.count > 0" class="ml-1 opacity-60"
                >({{ tag.count }})</span
              >
            </UBadge>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-wordcloud-shell {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 1.75rem;
  background:
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 24%
    ),
    radial-gradient(
      circle at left center,
      rgba(250, 204, 21, 0.06),
      transparent 20%
    ),
    linear-gradient(180deg, rgba(15, 23, 42, 0.74), rgba(15, 23, 42, 0.62));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 38px rgba(2, 6, 23, 0.18);
  backdrop-filter: blur(18px);
}

.xeno-wordcloud-stage,
.xeno-wordcloud-sidebar {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: linear-gradient(
    180deg,
    rgba(15, 23, 42, 0.6),
    rgba(15, 23, 42, 0.48)
  );
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
}
</style>
