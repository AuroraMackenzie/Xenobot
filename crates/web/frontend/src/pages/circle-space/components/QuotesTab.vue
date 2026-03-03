<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { SubTabs } from '@/components/UI'
import { CatchphraseTab, HotRepeatTab, KeywordAnalysis, WordcloudTab } from '@/components/analysis/quotes'

const { t } = useI18n()

interface TimeFilter {
  startTs?: number
  endTs?: number
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

// English engineering note.
const subTabs = computed(() => [
  { id: 'wordcloud', label: t('analysis.subTabs.quotes.wordcloud'), icon: 'i-heroicons-cloud' },
  { id: 'hot-repeat', label: t('analysis.subTabs.quotes.hotRepeat'), icon: 'i-heroicons-fire' },
  {
    id: 'catchphrase',
    label: t('analysis.subTabs.quotes.catchphrase'),
    icon: 'i-heroicons-chat-bubble-bottom-center-text',
  },
  { id: 'keyword', label: t('analysis.subTabs.quotes.keywordAnalysis'), icon: 'i-heroicons-magnifying-glass' },
])

const activeSubTab = ref('wordcloud')
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- English UI note -->
    <SubTabs v-model="activeSubTab" :items="subTabs" persist-key="quotesTab" />

    <!-- English UI note -->
    <div class="flex-1 min-h-0 overflow-auto">
      <Transition name="fade" mode="out-in">
        <!-- English UI note -->
        <CatchphraseTab
          v-if="activeSubTab === 'catchphrase'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <!-- English UI note -->
        <HotRepeatTab
          v-else-if="activeSubTab === 'hot-repeat'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <!-- English UI note -->
        <WordcloudTab
          v-else-if="activeSubTab === 'wordcloud'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <!-- English UI note -->
        <div v-else-if="activeSubTab === 'keyword'" class="main-content mx-auto max-w-3xl p-6">
          <KeywordAnalysis :session-id="props.sessionId" :time-filter="props.timeFilter" />
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
