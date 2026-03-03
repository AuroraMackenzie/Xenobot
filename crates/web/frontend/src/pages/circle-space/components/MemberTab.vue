<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { SubTabs } from '@/components/UI'
import MemberList from './member/MemberList.vue'
import NicknameHistory from './member/NicknameHistory.vue'
import Relationships from './member/Relationships.vue'
import { XenoClusterView } from '@/vendors/insight'

const { t } = useI18n()

interface TimeFilter {
  startTs?: number
  endTs?: number
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

const emit = defineEmits<{
  'data-changed': []
}>()

// English engineering note.
const subTabs = computed(() => [
  { id: 'list', label: t('analysis.subTabs.member.memberList'), icon: 'i-heroicons-users' },
  { id: 'relationships', label: t('analysis.subTabs.member.relationships'), icon: 'i-heroicons-heart' },
  { id: 'cluster', label: t('analysis.subTabs.member.cluster'), icon: 'i-heroicons-user-group' },
  { id: 'history', label: t('analysis.subTabs.member.nicknameHistory'), icon: 'i-heroicons-clock' },
])

const activeSubTab = ref('list')

function handleDataChanged() {
  emit('data-changed')
}
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- English UI note -->
    <SubTabs v-model="activeSubTab" :items="subTabs" persist-key="memberTab" />

    <!-- English UI note -->
    <div class="flex-1 min-h-0 overflow-auto">
      <Transition name="fade" mode="out-in">
        <!-- English UI note -->
        <MemberList
          v-if="activeSubTab === 'list'"
          :session-id="props.sessionId"
          @data-changed="handleDataChanged"
        />

        <!-- English UI note -->
        <Relationships
          v-else-if="activeSubTab === 'relationships'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <!-- English UI note -->
        <XenoClusterView
          v-else-if="activeSubTab === 'cluster'"
          :session-id="props.sessionId"
          :time-filter="props.timeFilter"
        />

        <!-- English UI note -->
        <NicknameHistory v-else-if="activeSubTab === 'history'" :session-id="props.sessionId" />
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
