<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MemberWithStats, MemberNameHistory } from '@/types/analysis'
import { SectionCard, EmptyState, LoadingState } from '@/components/UI'
import { formatPeriod } from '@/utils'

const { t } = useI18n()

// Props
const props = defineProps<{
  sessionId: string
}>()

// English engineering note.
const members = ref<MemberWithStats[]>([])

// English engineering note.
interface MemberWithHistory {
  memberId: number
  name: string
  history: MemberNameHistory[]
}

const membersWithNicknameChanges = ref<MemberWithHistory[]>([])
const isLoadingHistory = ref(false)

// English engineering note.
function getDisplayName(member: MemberWithStats): string {
  return member.groupNickname || member.accountName || member.platformId
}

// English engineering note.
async function loadMembers() {
  if (!props.sessionId) return
  try {
    members.value = await window.chatApi.getMembers(props.sessionId)
  } catch (error) {
    console.error('[NicknameHistory] Failed to load members:', error)
  }
}

async function loadMembersWithNicknameChanges() {
  if (!props.sessionId || members.value.length === 0) return

  isLoadingHistory.value = true
  const membersWithChanges: MemberWithHistory[] = []

  try {
    const historyPromises = members.value.map((member) =>
      window.chatApi.getMemberNameHistory(props.sessionId, member.id)
    )

    const allHistories = await Promise.all(historyPromises)

    members.value.forEach((member, index) => {
      const history = allHistories[index]
      if (history.length > 2) {
        membersWithChanges.push({
          memberId: member.id,
          name: getDisplayName(member),
          history,
        })
      }
    })

    membersWithNicknameChanges.value = membersWithChanges
  } catch (error) {
    console.error('[NicknameHistory] Failed to load nickname change history:', error)
  } finally {
    isLoadingHistory.value = false
  }
}

// English engineering note.
watch(
  () => props.sessionId,
  async () => {
    await loadMembers()
  },
  { immediate: true }
)

// English engineering note.
watch(
  () => members.value.length,
  () => {
    if (members.value.length > 0) {
      loadMembersWithNicknameChanges()
    }
  }
)

onMounted(async () => {
  if (members.value.length === 0) {
    await loadMembers()
  }
})
</script>

<template>
  <div class="xeno-nickname-shell main-content max-w-5xl p-6">
    <p class="mb-4 text-sm text-gray-500 dark:text-gray-400 no-capture">
      {{ t('members.nicknameHistory.note') }}
    </p>
    <!-- English UI note -->
    <SectionCard
      :title="t('members.nicknameHistory.title')"
      :description="
        isLoadingHistory
          ? t('members.nicknameHistory.loading')
          : membersWithNicknameChanges.length > 0
            ? t('members.nicknameHistory.hasChanges', { count: membersWithNicknameChanges.length })
            : t('members.nicknameHistory.noChanges')
      "
    >
      <div
        v-if="!isLoadingHistory && membersWithNicknameChanges.length > 0"
        class="divide-y divide-gray-100 dark:divide-gray-800"
      >
        <div
          v-for="member in membersWithNicknameChanges"
          :key="member.memberId"
          class="flex items-start gap-3 px-5 py-3"
        >
          <div class="w-32 shrink-0 pt-0.5 font-medium text-gray-900 dark:text-white">
            {{ member.name }}
          </div>

          <div class="flex flex-1 flex-wrap items-center gap-2">
            <template v-for="(item, index) in member.history" :key="index">
              <div class="xeno-nickname-chip flex items-center gap-1.5 rounded-xl px-3 py-1.5">
                <span
                  class="text-sm"
                  :class="item.endTs === null ? 'font-semibold text-pink-600' : 'text-gray-700 dark:text-gray-300'"
                >
                  {{ item.name }}
                </span>
                <UBadge v-if="item.endTs === null" color="primary" variant="soft" size="xs">
                  {{ t('members.nicknameHistory.current') }}
                </UBadge>
                <span class="text-xs text-gray-400">({{ formatPeriod(item.startTs, item.endTs) }})</span>
              </div>

              <span v-if="index < member.history.length - 1" class="text-gray-300 dark:text-gray-600">→</span>
            </template>
          </div>
        </div>
      </div>

      <EmptyState v-else-if="!isLoadingHistory" :text="t('members.nicknameHistory.empty')" />

      <LoadingState v-else :text="t('members.nicknameHistory.loadingText')" />
    </SectionCard>
  </div>
</template>

<style scoped>
.xeno-nickname-shell {
  background:
    radial-gradient(circle at top right, rgba(59, 130, 246, 0.08), transparent 26%),
    radial-gradient(circle at left center, rgba(236, 72, 153, 0.06), transparent 22%);
}

.xeno-nickname-chip {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background:
    linear-gradient(180deg, rgba(15, 23, 42, 0.72), rgba(15, 23, 42, 0.58));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
}
</style>
