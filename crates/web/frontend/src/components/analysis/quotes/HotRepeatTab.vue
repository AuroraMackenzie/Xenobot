<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  queryXenoRepeatAnalysis,
  type XenoRepeatAnalysis,
} from "@/vendors/insight";
import { ListPro } from "@/components/charts";
import { LoadingState, EmptyState, SectionCard } from "@/components/UI";
import { formatDate, getRankBadgeClass } from "@/utils";
import { useLayoutStore } from "@/stores/layout";

const { t } = useI18n();

interface TimeFilter {
  startTs?: number;
  endTs?: number;
}

const props = defineProps<{
  sessionId: string;
  timeFilter?: TimeFilter;
}>();

const layoutStore = useLayoutStore();

// English engineering note.
const repeatAnalysis = ref<XenoRepeatAnalysis | null>(null);
const isLoading = ref(false);

async function loadRepeatAnalysis() {
  if (!props.sessionId) return;
  isLoading.value = true;
  try {
    repeatAnalysis.value = await queryXenoRepeatAnalysis(
      props.sessionId,
      props.timeFilter,
    );
  } catch (error) {
    console.error("Failed to load repeat analysis:", error);
  } finally {
    isLoading.value = false;
  }
}

function truncateContent(content: string, maxLength = 30): string {
  if (content.length <= maxLength) return content;
  return content.slice(0, maxLength) + "...";
}

// English engineering note.
function viewRepeatContext(item: { content: string; firstMessageId: number }) {
  layoutStore.openChatRecordDrawer({
    scrollToMessageId: item.firstMessageId,
    highlightKeywords: [item.content],
  });
}

// English engineering note.
watch(
  () => [props.sessionId, props.timeFilter],
  () => {
    loadRepeatAnalysis();
  },
  { immediate: true, deep: true },
);
</script>

<template>
  <div class="xeno-quotes-panel main-content mx-auto max-w-3xl p-6">
    <!-- English UI note -->
    <LoadingState v-if="isLoading" :text="t('quotes.hotRepeat.loading')" />

    <!-- English UI note -->
    <ListPro
      v-else-if="repeatAnalysis && repeatAnalysis.hotContents.length > 0"
      :items="repeatAnalysis.hotContents"
      :title="t('quotes.hotRepeat.title')"
      :description="t('quotes.hotRepeat.description')"
      :top-n="50"
      :count-template="t('quotes.hotRepeat.countTemplate')"
    >
      <template #item="{ item, index }">
        <div class="flex items-center gap-3">
          <span
            class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-bold"
            :class="getRankBadgeClass(index)"
          >
            {{ index + 1 }}
          </span>
          <span class="shrink-0 text-lg font-bold text-pink-600">
            {{ t("quotes.hotRepeat.people", { count: item.maxChainLength }) }}
          </span>
          <div class="flex flex-1 items-center gap-1 overflow-hidden text-sm">
            <span
              class="shrink-0 font-medium text-gray-900 dark:text-white whitespace-nowrap"
            >
              {{ item.originatorName }}{{ t("quotes.hotRepeat.colon") }}
            </span>
            <span
              class="truncate text-gray-600 dark:text-gray-400"
              :title="item.content"
            >
              {{ truncateContent(item.content) }}
            </span>
          </div>
          <div class="flex shrink-0 items-center gap-2 text-xs text-gray-500">
            <span>{{
              t("quotes.hotRepeat.times", { count: item.count })
            }}</span>
            <span class="text-gray-300 dark:text-gray-600">|</span>
            <span>{{ formatDate(item.lastTs) }}</span>
            <UButton
              icon="i-heroicons-chat-bubble-left-right"
              color="neutral"
              variant="ghost"
              size="xs"
              :title="t('quotes.hotRepeat.viewChat')"
              @click.stop="viewRepeatContext(item)"
            />
          </div>
        </div>
      </template>
    </ListPro>

    <!-- English UI note -->
    <SectionCard v-else :title="t('quotes.hotRepeat.title')">
      <EmptyState :text="t('quotes.hotRepeat.empty')" />
    </SectionCard>
  </div>
</template>

<style scoped>
.xeno-quotes-panel {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.5rem;
  background:
    radial-gradient(
      circle at top right,
      rgba(248, 113, 113, 0.09),
      transparent 24%
    ),
    var(--xeno-stage-shell-bg);
  box-shadow:
    inset 0 1px 0 var(--xeno-surface-hairline),
    0 18px 38px rgba(2, 6, 23, 0.14);
  backdrop-filter: none;
}
</style>
