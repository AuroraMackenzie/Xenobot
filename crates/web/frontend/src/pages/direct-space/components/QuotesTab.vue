<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { SubTabs } from "@/components/UI";
import {
  CatchphraseTab,
  KeywordAnalysis,
  WordcloudTab,
} from "@/components/analysis/quotes";

const { t } = useI18n();

interface TimeFilter {
  startTs?: number;
  endTs?: number;
}

const props = defineProps<{
  sessionId: string;
  timeFilter?: TimeFilter;
}>();

// English engineering note.
const subTabs = computed(() => [
  {
    id: "wordcloud",
    label: t("analysis.subTabs.quotes.wordcloud"),
    icon: "i-heroicons-cloud",
  },
  {
    id: "catchphrase",
    label: t("analysis.subTabs.quotes.catchphrase"),
    icon: "i-heroicons-chat-bubble-bottom-center-text",
  },
  {
    id: "keyword",
    label: t("analysis.subTabs.quotes.keywordAnalysis"),
    icon: "i-heroicons-magnifying-glass",
  },
]);

const activeSubTab = ref("wordcloud");
</script>

<template>
  <div class="xeno-quotes-shell flex h-full flex-col gap-4">
    <SubTabs v-model="activeSubTab" :items="subTabs" persist-key="quotesTab" />

    <div class="xeno-quotes-stage flex-1 min-h-0 overflow-auto rounded-2xl">
      <Transition name="fade" mode="out-in">
        <CatchphraseTab
          v-if="activeSubTab === 'catchphrase'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <WordcloudTab
          v-else-if="activeSubTab === 'wordcloud'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <div
          v-else-if="activeSubTab === 'keyword'"
          class="main-content mx-auto max-w-3xl p-6"
        >
          <KeywordAnalysis
            :session-id="props.sessionId"
            :time-filter="props.timeFilter"
          />
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.xeno-quotes-shell {
  background:
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 24%
    ),
    radial-gradient(
      circle at left center,
      rgba(236, 72, 153, 0.06),
      transparent 20%
    );
}

.xeno-quotes-stage {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: linear-gradient(
    180deg,
    rgba(15, 23, 42, 0.76),
    rgba(15, 23, 42, 0.62)
  );
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 38px rgba(2, 6, 23, 0.18);
  backdrop-filter: blur(18px);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
