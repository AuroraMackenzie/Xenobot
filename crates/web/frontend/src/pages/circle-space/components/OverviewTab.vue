<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { AnalysisSession, MessageType } from "@/types/base";
import { getMessageTypeName } from "@/types/base";
import type {
  MemberActivity,
  HourlyActivity,
  DailyActivity,
  WeekdayActivity,
} from "@/types/analysis";
import { EChartPie } from "@/components/charts";
import type { EChartPieData } from "@/components/charts";
import { SectionCard } from "@/components/UI";
import { useOverviewStatistics } from "@/composables/analysis/useOverviewStatistics";
import { useDailyTrend } from "@/composables/analysis/useDailyTrend";
import OverviewStatCards from "@/components/analysis/Overview/OverviewStatCards.vue";
import OverviewIdentityCard from "@/components/analysis/Overview/OverviewIdentityCard.vue";
import DailyTrendCard from "@/components/analysis/Overview/DailyTrendCard.vue";

const { t } = useI18n();

const props = defineProps<{
  session: AnalysisSession;
  memberActivity: MemberActivity[];
  topMembers: MemberActivity[];
  bottomMembers: MemberActivity[];
  messageTypes: Array<{ type: MessageType; count: number }>;
  hourlyActivity: HourlyActivity[];
  dailyActivity: DailyActivity[];
  timeRange: { start: number; end: number } | null;
  selectedYear: number | null;
  filteredMessageCount: number;
  filteredMemberCount: number;
  timeFilter?: { startTs?: number; endTs?: number };
}>();

// English engineering note.
const weekdayActivity = ref<WeekdayActivity[]>([]);

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
} = useOverviewStatistics(props, weekdayActivity);

const { dailyChartData } = useDailyTrend(props.dailyActivity);

// English engineering note.
const typeChartData = computed<EChartPieData>(() => {
  return {
    labels: props.messageTypes.map((item) => getMessageTypeName(item.type, t)),
    values: props.messageTypes.map((item) => item.count),
  };
});

// English engineering note.
const memberChartData = computed<EChartPieData>(() => {
  const sortedMembers = [...props.memberActivity].sort(
    (a, b) => b.messageCount - a.messageCount,
  );
  const top10 = sortedMembers.slice(0, 10);
  const othersCount = sortedMembers
    .slice(10)
    .reduce((sum, m) => sum + m.messageCount, 0);

  const labels = top10.map((m) => m.name);
  const values = top10.map((m) => m.messageCount);

  if (othersCount > 0) {
    labels.push(t("analysis.overview.others"));
    values.push(othersCount);
  }

  return {
    labels,
    values,
  };
});

// English engineering note.
async function loadWeekdayActivity() {
  if (!props.session.id) return;
  try {
    weekdayActivity.value = await window.chatApi.getWeekdayActivity(
      props.session.id,
      props.timeFilter,
    );
  } catch (error) {
    console.error(
      "[CircleSpaceOverview] Failed to load weekday activity:",
      error,
    );
  }
}

// English engineering note.
watch(
  () => [props.session.id, props.timeFilter],
  () => {
    loadWeekdayActivity();
  },
  { immediate: true, deep: true },
);
</script>

<template>
  <div class="xeno-overview-shell--group main-content space-y-6 p-6">
    <!-- English UI note -->
    <OverviewIdentityCard
      :session="session"
      :total-duration-days="totalDurationDays"
      :total-daily-avg-messages="totalDailyAvgMessages"
      :time-range="timeRange"
    />

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
      <SectionCard
        :title="t('analysis.overview.messageTypeDistribution')"
        :show-divider="false"
      >
        <div class="p-5">
          <EChartPie :data="typeChartData" :height="256" />
        </div>
      </SectionCard>

      <!-- English UI note -->
      <SectionCard
        :title="t('analysis.overview.memberDistribution')"
        :show-divider="false"
      >
        <div class="p-5">
          <EChartPie :data="memberChartData" :height="256" />
        </div>
      </SectionCard>
    </div>

    <!-- English UI note -->
    <DailyTrendCard
      :daily-activity="dailyActivity"
      :daily-chart-data="dailyChartData"
    />
  </div>
</template>
