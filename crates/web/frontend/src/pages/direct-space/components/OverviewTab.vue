<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AnalysisSession, MessageType } from '@/types/base'
import { getMessageTypeName } from '@/types/base'
import type { MemberActivity, HourlyActivity, DailyActivity, WeekdayActivity } from '@/types/analysis'
import { EChartPie } from '@/components/charts'
import type { EChartPieData } from '@/components/charts'
import { SectionCard } from '@/components/UI'
import { useOverviewStatistics } from '@/composables/analysis/useOverviewStatistics'
import { useDailyTrend } from '@/composables/analysis/useDailyTrend'
import OverviewStatCards from '@/components/analysis/Overview/OverviewStatCards.vue'
import OverviewIdentityCard from '@/components/analysis/Overview/OverviewIdentityCard.vue'
import DailyTrendCard from '@/components/analysis/Overview/DailyTrendCard.vue'

const { t } = useI18n()

const props = defineProps<{
  session: AnalysisSession
  memberActivity: MemberActivity[]
  messageTypes: Array<{ type: MessageType; count: number }>
  hourlyActivity: HourlyActivity[]
  dailyActivity: DailyActivity[]
  timeRange: { start: number; end: number } | null
  selectedYear: number | null
  filteredMessageCount: number
  filteredMemberCount: number
  timeFilter?: { startTs?: number; endTs?: number }
}>()

// English engineering note.
const weekdayActivity = ref<WeekdayActivity[]>([])

// English engineering note.
const {
  durationDays,
  dailyAvgMessages,
  totalDurationDays,
  totalDailyAvgMessages,
  imageCount,
  peakHour,
  peakWeekday,
  weekdayNames,
  weekdayVsWeekend,
  peakDay,
  activeDays,
  totalDays,
  activeRate,
  maxConsecutiveDays,
} = useOverviewStatistics(props, weekdayActivity)

const { dailyChartData } = useDailyTrend(props.dailyActivity)

// English engineering note.
const typeChartData = computed<EChartPieData>(() => {
  return {
    labels: props.messageTypes.map((item) => getMessageTypeName(item.type, t)),
    values: props.messageTypes.map((item) => item.count),
  }
})

// English engineering note.
const memberComparisonData = computed(() => {
  // English engineering note.
  if (props.memberActivity.length < 2) return null

  // English engineering note.
  const sorted = [...props.memberActivity].sort((a, b) => b.messageCount - a.messageCount)
  const top2 = sorted.slice(0, 2)
  const total = top2[0].messageCount + top2[1].messageCount

  return {
    member1: {
      name: top2[0].name,
      avatar: top2[0].avatar,
      count: top2[0].messageCount,
      percentage: total > 0 ? Math.round((top2[0].messageCount / total) * 100) : 0,
    },
    member2: {
      name: top2[1].name,
      avatar: top2[1].avatar,
      count: top2[1].messageCount,
      percentage: total > 0 ? Math.round((top2[1].messageCount / total) * 100) : 0,
    },
    total,
  }
})

// English engineering note.
const comparisonChartData = computed<EChartPieData>(() => {
  if (!memberComparisonData.value) {
    return { labels: [], values: [] }
  }
  return {
    labels: [memberComparisonData.value.member1.name, memberComparisonData.value.member2.name],
    values: [memberComparisonData.value.member1.count, memberComparisonData.value.member2.count],
  }
})

// English engineering note.
async function loadWeekdayActivity() {
  if (!props.session.id) return
  try {
    weekdayActivity.value = await window.chatApi.getWeekdayActivity(props.session.id, props.timeFilter)
  } catch (error) {
    console.error('加载星期活跃度失败:', error)
  }
}

// English engineering note.
watch(
  () => [props.session.id, props.timeFilter],
  () => {
    loadWeekdayActivity()
  },
  { immediate: true, deep: true }
)
</script>

<template>
  <div class="main-content space-y-6 p-6">
    <!-- English UI note -->
    <OverviewIdentityCard
      :session="session"
      :total-duration-days="totalDurationDays"
      :total-daily-avg-messages="totalDailyAvgMessages"
      :time-range="timeRange"
    />

    <!-- English UI note -->
    <SectionCard v-if="memberComparisonData" :title="t('analysis.overview.messageRatio')" :show-divider="false">
      <div class="p-5">
        <div class="flex items-center gap-8">
          <!-- English UI note -->
          <div class="flex-1 text-center">
            <!-- English UI note -->
            <img
              v-if="memberComparisonData.member1.avatar"
              :src="memberComparisonData.member1.avatar"
              :alt="memberComparisonData.member1.name"
              class="mx-auto h-16 w-16 rounded-full object-cover"
            />
            <div
              v-else
              class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-pink-100 dark:bg-pink-900/30"
            >
              <span class="text-2xl font-bold text-pink-600 dark:text-pink-400">
                {{ memberComparisonData.member1.name.charAt(0) }}
              </span>
            </div>
            <p class="mt-2 text-sm font-medium text-gray-900 dark:text-white">
              {{ memberComparisonData.member1.name }}
            </p>
            <p class="text-2xl font-black text-pink-600 dark:text-pink-400">
              {{ memberComparisonData.member1.percentage }}%
            </p>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              {{ memberComparisonData.member1.count.toLocaleString() }} {{ t('analysis.overview.messageUnit') }}
            </p>
          </div>

          <!-- English UI note -->
          <div class="flex-1">
            <div class="relative h-8 overflow-hidden rounded-full bg-gray-100 dark:bg-gray-800">
              <div
                class="absolute left-0 top-0 h-full rounded-l-full bg-pink-500 transition-all"
                :style="{ width: `${memberComparisonData.member1.percentage}%` }"
              />
              <div
                class="absolute right-0 top-0 h-full rounded-r-full bg-blue-500 transition-all"
                :style="{ width: `${memberComparisonData.member2.percentage}%` }"
              />
            </div>
            <div class="mt-2 flex justify-between text-xs text-gray-500">
              <span>{{ memberComparisonData.member1.percentage }}%</span>
              <span>{{ memberComparisonData.member2.percentage }}%</span>
            </div>
          </div>

          <!-- English UI note -->
          <div class="flex-1 text-center">
            <!-- English UI note -->
            <img
              v-if="memberComparisonData.member2.avatar"
              :src="memberComparisonData.member2.avatar"
              :alt="memberComparisonData.member2.name"
              class="mx-auto h-16 w-16 rounded-full object-cover"
            />
            <div
              v-else
              class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-blue-100 dark:bg-blue-900/30"
            >
              <span class="text-2xl font-bold text-blue-600 dark:text-blue-400">
                {{ memberComparisonData.member2.name.charAt(0) }}
              </span>
            </div>
            <p class="mt-2 text-sm font-medium text-gray-900 dark:text-white">
              {{ memberComparisonData.member2.name }}
            </p>
            <p class="text-2xl font-black text-blue-600 dark:text-blue-400">
              {{ memberComparisonData.member2.percentage }}%
            </p>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              {{ memberComparisonData.member2.count.toLocaleString() }} {{ t('analysis.overview.messageUnit') }}
            </p>
          </div>
        </div>
      </div>
    </SectionCard>

    <!-- English UI note -->
    <OverviewStatCards
      :daily-avg-messages="dailyAvgMessages"
      :duration-days="durationDays"
      :image-count="imageCount"
      :peak-hour="peakHour"
      :peak-weekday="peakWeekday"
      :weekday-names="weekdayNames"
      :weekday-vs-weekend="weekdayVsWeekend"
      :peak-day="peakDay"
      :active-days="activeDays"
      :total-days="totalDays"
      :active-rate="activeRate"
      :max-consecutive-days="maxConsecutiveDays"
    />

    <!-- English UI note -->
    <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
      <!-- English UI note -->
      <SectionCard :title="t('analysis.overview.messageTypeDistribution')" :show-divider="false">
        <div class="p-5">
          <EChartPie :data="typeChartData" :height="256" />
        </div>
      </SectionCard>

      <!-- English UI note -->
      <SectionCard v-if="memberComparisonData" :title="t('analysis.overview.memberComparison')" :show-divider="false">
        <div class="p-5">
          <EChartPie :data="comparisonChartData" :height="256" />
        </div>
      </SectionCard>
    </div>

    <!-- English UI note -->
    <DailyTrendCard :daily-activity="dailyActivity" :daily-chart-data="dailyChartData" />
  </div>
</template>
