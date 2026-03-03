<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { SubTabs } from '@/components/UI'
import UserSelect from '@/components/common/UserSelect.vue'
import { XenoMessageView, XenoInteractionView, XenoRankingView } from '@/vendors/insight'
import { isFeatureSupported, type LocaleType } from '@/i18n'

const { t, locale } = useI18n()

interface TimeFilter {
  startTs?: number
  endTs?: number
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

// English engineering note.
const subTabs = computed(() => {
  const tabs = [
    { id: 'message', label: t('analysis.subTabs.view.message'), icon: 'i-heroicons-chat-bubble-left-right' },
    { id: 'interaction', label: t('analysis.subTabs.view.interaction'), icon: 'i-heroicons-arrows-right-left' },
  ]
  // English engineering note.
  if (isFeatureSupported('groupRanking', locale.value as LocaleType)) {
    tabs.push({ id: 'ranking', label: t('analysis.subTabs.view.ranking'), icon: 'i-heroicons-trophy' })
  }
  return tabs
})

const activeSubTab = ref('message')

// English engineering note.
const selectedMemberId = ref<number | null>(null)

// English engineering note.
const viewTimeFilter = computed(() => ({
  ...props.timeFilter,
  memberId: selectedMemberId.value,
}))
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- English UI note -->
    <SubTabs v-model="activeSubTab" :items="subTabs" persist-key="groupViewTab">
      <template #right>
        <UserSelect v-model="selectedMemberId" :session-id="props.sessionId" />
      </template>
    </SubTabs>

    <!-- English UI note -->
    <div class="flex-1 min-h-0 overflow-y-auto">
      <Transition name="fade" mode="out-in">
        <XenoMessageView
          v-if="activeSubTab === 'message'"
          :session-id="props.sessionId"
          :time-filter="viewTimeFilter"
        />
        <XenoInteractionView
          v-else-if="activeSubTab === 'interaction'"
          :session-id="props.sessionId"
          :time-filter="viewTimeFilter"
        />
        <XenoRankingView
          v-else-if="activeSubTab === 'ranking'"
          :session-id="props.sessionId"
          :time-filter="viewTimeFilter"
        />
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
