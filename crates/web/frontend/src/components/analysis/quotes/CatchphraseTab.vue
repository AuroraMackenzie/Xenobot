<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { CatchphraseAnalysis } from '@/types/analysis'
import { ListPro } from '@/components/charts'
import { SectionCard, EmptyState, LoadingState } from '@/components/UI'

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
const catchphraseAnalysis = ref<CatchphraseAnalysis | null>(null)
const isLoading = ref(false)

async function loadCatchphraseAnalysis() {
  if (!props.sessionId) return
  isLoading.value = true
  try {
    catchphraseAnalysis.value = await window.chatApi.getCatchphraseAnalysis(props.sessionId, props.timeFilter)
  } catch (error) {
    console.error('Failed to load catchphrase analysis:', error)
  } finally {
    isLoading.value = false
  }
}

function truncateContent(content: string, maxLength = 20): string {
  if (content.length <= maxLength) return content
  return content.slice(0, maxLength) + '...'
}

// English engineering note.
watch(
  () => [props.sessionId, props.timeFilter],
  () => {
    loadCatchphraseAnalysis()
  },
  { immediate: true, deep: true }
)
</script>

<template>
  <div class="xeno-quotes-panel main-content mx-auto max-w-3xl p-6">
    <!-- English UI note -->
    <LoadingState v-if="isLoading" :text="t('quotes.catchphrase.loading')" />

    <!-- English UI note -->
    <ListPro
      v-else-if="catchphraseAnalysis && catchphraseAnalysis.members.length > 0"
      :items="catchphraseAnalysis.members"
      :title="t('quotes.catchphrase.title')"
      :description="t('quotes.catchphrase.description', { count: catchphraseAnalysis.members.length })"
      :count-template="t('quotes.catchphrase.countTemplate')"
    >
      <template #item="{ item: member }">
        <div class="flex items-start gap-4">
          <div class="w-28 shrink-0 pt-1 font-medium text-gray-900 dark:text-white">
            {{ member.name }}
          </div>

          <div class="flex flex-1 flex-wrap items-center gap-2">
            <div
              v-for="(phrase, index) in member.catchphrases"
              :key="index"
              class="flex items-center gap-1.5 rounded-lg px-3 py-1.5"
              :class="
                index === 0
                  ? 'bg-amber-50 dark:bg-amber-900/20'
                  : index === 1
                    ? 'bg-gray-100 dark:bg-gray-800'
                    : 'bg-gray-50 dark:bg-gray-800/50'
              "
            >
              <span
                class="text-sm"
                :class="
                  index === 0 ? 'font-medium text-amber-700 dark:text-amber-400' : 'text-gray-700 dark:text-gray-300'
                "
                :title="phrase.content"
              >
                {{ truncateContent(phrase.content) }}
              </span>
              <span class="text-xs text-gray-400">{{ t('quotes.catchphrase.times', { count: phrase.count }) }}</span>
            </div>
          </div>
        </div>
      </template>
    </ListPro>

    <!-- English UI note -->
    <SectionCard v-else :title="t('quotes.catchphrase.title')">
      <EmptyState :text="t('quotes.catchphrase.empty')" />
    </SectionCard>
  </div>
</template>

<style scoped>
.xeno-quotes-panel {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 1.5rem;
  background:
    radial-gradient(circle at top right, rgba(236, 72, 153, 0.08), transparent 24%),
    linear-gradient(180deg, rgba(15, 23, 42, 0.74), rgba(15, 23, 42, 0.62));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 38px rgba(2, 6, 23, 0.18);
  backdrop-filter: blur(18px);
}
</style>
