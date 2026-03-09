<script setup lang="ts">
/**
 * Chat record filter panel.
 * Supports message id, time range, and keyword/semantic query filters.
 */
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import dayjs from 'dayjs'
import { DatePicker } from '@/components/UI'
import type { ChatRecordQuery, FilterFormData } from './types'

const { t } = useI18n()

const props = defineProps<{
  /** Active filter query mirrored from the drawer state. */
  query: ChatRecordQuery
}>()

const emit = defineEmits<{
  /** Emit active filter query. */
  (e: 'apply', query: ChatRecordQuery): void
  /** Reset filter fields. */
  (e: 'reset'): void
}>()

// Local editable form state.
const formData = ref<FilterFormData>({
  messageId: '',
  memberName: '',
  keywords: '',
  startDate: '',
  endDate: '',
})

// Keep form state aligned with external query updates.
watch(
  () => props.query,
  (query) => {
    if (query) {
      formData.value = {
        messageId: query.scrollToMessageId?.toString() || '',
        memberName: query.memberName || '',
        keywords: query.keywords?.join(', ') || '',
        startDate: query.startTs ? dayjs.unix(query.startTs).format('YYYY-MM-DD') : '',
        endDate: query.endTs ? dayjs.unix(query.endTs).format('YYYY-MM-DD') : '',
      }
    }
  },
  { immediate: true }
)

const SEMANTIC_PREFIX = /^(sem|semantic|vector|meaning|语义)\s*:/i

// Build query object from current form values and emit it to parent.
function applyFilter() {
  const f = formData.value
  const query: ChatRecordQuery = {}

  // Message-id jump and text search are mutually exclusive.
  const hasKeywords = f.keywords && f.keywords.trim()

  // Message id is only active when no text query is provided.
  if (f.messageId && !hasKeywords) {
    const id = parseInt(f.messageId, 10)
    if (!isNaN(id)) {
      query.scrollToMessageId = id
    }
  }

  // Member-name filter is display-only until member-id mapping is wired.
  if (f.memberName) {
    query.memberName = f.memberName
  }

  // Parse keyword query.
  if (hasKeywords) {
    let rawKeywords = f.keywords.trim()
    let searchMode: NonNullable<ChatRecordQuery['searchMode']> = 'keyword'
    if (SEMANTIC_PREFIX.test(rawKeywords)) {
      rawKeywords = rawKeywords.replace(SEMANTIC_PREFIX, '').trim()
      searchMode = 'semantic'
    }

    const keywords =
      searchMode === 'semantic'
        ? (rawKeywords ? [rawKeywords] : [])
        : rawKeywords
            .split(/[,，]/)
            .map((k) => k.trim())
            .filter((k) => k)

    if (keywords.length > 0) {
      query.keywords = keywords
      query.searchMode = searchMode
      query.highlightKeywords =
        searchMode === 'semantic'
          ? keywords[0]
              .split(/\s+/)
              .map((k) => k.trim())
              .filter((k) => k)
              .slice(0, 12)
          : keywords
      if (searchMode === 'semantic') {
        query.semanticThreshold = 0.45
      }

      // Clear message id because mixed mode is not supported.
      formData.value.messageId = ''
    }
  }

  // Time-window filter.
  if (f.startDate) {
    query.startTs = dayjs(f.startDate).startOf('day').unix()
  }
  if (f.endDate) {
    query.endTs = dayjs(f.endDate).endOf('day').unix()
  }

  emit('apply', query)
}

// Submit with Enter in keyword input.
function handleKeywordsKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    event.preventDefault()
    applyFilter()
  }
}

// Reset all form controls.
function resetFilter() {
  formData.value = {
    messageId: '',
    memberName: '',
    keywords: '',
    startDate: '',
    endDate: '',
  }
  emit('reset')
}
</script>

<template>
  <div class="xeno-record-filter border-b px-4 py-3">
    <div class="flex flex-wrap items-center gap-3">
      <UInput
        v-model="formData.messageId"
        type="number"
        :placeholder="t('records.filter.messageId')"
        size="sm"
        class="w-24 shrink-0"
      />
      <UInput
        v-model="formData.memberName"
        :placeholder="t('records.filter.memberNotSupported')"
        size="sm"
        class="w-32 shrink-0"
        disabled
      />
      <div class="min-w-0 flex flex-1 flex-wrap items-center gap-2">
        <DatePicker v-model="formData.startDate" :placeholder="t('records.filter.startDate')" class="min-w-[10rem]" />
        <span class="text-xs text-gray-400">~</span>
        <DatePicker v-model="formData.endDate" :placeholder="t('records.filter.endDate')" class="min-w-[10rem]" />
      </div>
    </div>

    <div class="mt-3 flex flex-wrap items-center gap-3">
      <UInput
        v-model="formData.keywords"
        :placeholder="`${t('records.filter.keywordsPlaceholder')} · ${t('records.filter.keywordsSemanticHint')}`"
        size="sm"
        class="min-w-[15rem] flex-1"
        @keydown="handleKeywordsKeydown"
      />
      <div class="flex shrink-0 gap-2">
        <UButton color="neutral" variant="ghost" size="sm" @click="resetFilter">
          {{ t('records.filter.reset') }}
        </UButton>
        <UButton color="primary" size="sm" @click="applyFilter">
          {{ t('records.filter.filter') }}
        </UButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-record-filter {
  border-bottom-color: rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 110%),
    rgba(6, 15, 24, 0.42);
}
</style>
