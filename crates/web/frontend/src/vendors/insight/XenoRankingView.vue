<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { EChartBar } from '@/components/charts'
import { EmptyState, LoadingState, SectionCard } from '@/components/UI'

interface TimeFilter {
  startTs?: number
  endTs?: number
  memberId?: number | null
}

interface MemberActivity {
  memberId: number
  name: string
  messageCount: number
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

const { t } = useI18n()
const isLoading = ref(false)
const members = ref<MemberActivity[]>([])

async function loadData() {
  if (!props.sessionId) return
  isLoading.value = true
  try {
    const result = await window.chatApi.getMemberActivity(props.sessionId, props.timeFilter)
    members.value = Array.isArray(result) ? result : []
  } catch (error) {
    console.error('XenoRankingView load failed:', error)
    members.value = []
  } finally {
    isLoading.value = false
  }
}

watch(
  () => [props.sessionId, props.timeFilter],
  () => loadData(),
  { immediate: true, deep: true }
)

const rankedMembers = computed(() => [...members.value].sort((a, b) => b.messageCount - a.messageCount).slice(0, 20))
const maxCount = computed(() => rankedMembers.value[0]?.messageCount || 1)
const totalCount = computed(() => rankedMembers.value.reduce((sum, item) => sum + item.messageCount, 0))

const chartData = computed(() => {
  const top10 = rankedMembers.value.slice(0, 10)
  return {
    labels: top10.map((item) => item.name),
    values: top10.map((item) => item.messageCount),
  }
})
</script>

<template>
  <div class="main-content space-y-5 p-6">
    <LoadingState v-if="isLoading" :text="t('common.loading')" />

    <template v-else-if="rankedMembers.length > 0">
      <SectionCard title="Member Leaderboard">
        <div class="p-4">
          <EChartBar :data="chartData" :height="320" :horizontal="true" :gradient="true" />
        </div>
      </SectionCard>

      <SectionCard title="Top 20 Rankings">
        <div class="space-y-2 p-4">
          <div
            v-for="(item, idx) in rankedMembers"
            :key="item.memberId"
            class="rounded-xl border border-slate-200/70 bg-white/80 px-4 py-3 dark:border-slate-700/60 dark:bg-slate-900/60"
          >
            <div class="mb-2 flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="inline-flex h-6 w-6 items-center justify-center rounded-full bg-cyan-500 text-xs font-bold text-white">
                  {{ idx + 1 }}
                </span>
                <span class="text-sm font-semibold text-slate-800 dark:text-slate-200">{{ item.name }}</span>
              </div>
              <span class="text-sm font-bold text-cyan-600 dark:text-cyan-300">{{ item.messageCount.toLocaleString() }}</span>
            </div>
            <div class="h-2 rounded-full bg-slate-200 dark:bg-slate-700">
              <div
                class="h-2 rounded-full bg-linear-to-r from-cyan-500 to-teal-500 transition-all"
                :style="{ width: `${Math.max(6, (item.messageCount / maxCount) * 100)}%` }"
              />
            </div>
          </div>
        </div>
      </SectionCard>

      <div class="rounded-xl border border-amber-100 bg-amber-50/70 p-4 dark:border-amber-500/20 dark:bg-amber-500/8">
        <p class="text-xs font-semibold tracking-wide text-amber-700 dark:text-amber-300">TOP 20 MESSAGE TOTAL</p>
        <p class="mt-2 text-2xl font-black text-amber-600 dark:text-amber-300">{{ totalCount.toLocaleString() }}</p>
      </div>
    </template>

    <SectionCard v-else title="Ranking View">
      <EmptyState :text="t('common.noData')" />
    </SectionCard>
  </div>
</template>
