<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { StatCard } from "@/components/UI";
import type {
  WeekdayActivity,
  DailyActivity,
  HourlyActivity,
} from "@/types/analysis";
import dayjs from "dayjs";

const { t } = useI18n();

defineProps<{
  dailyAvgMessages: number;
  durationDays: number;
  imageCount: number;
  peakHour: HourlyActivity | null;
  peakWeekday: WeekdayActivity | null;
  weekdayNames: string[];
  weekdayVsWeekend: { weekday: number; weekend: number };
  peakDay: DailyActivity | null;
  activeDays: number;
  totalDays: number;
  activeRate: number;
  maxConsecutiveDays: number;
}>();
</script>

<template>
  <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.dailyAvgMessages')"
      :value="
        t('analysis.overview.statCards.messagesCount', {
          count: dailyAvgMessages,
        })
      "
      icon="📊"
      icon-bg="blue"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">
          {{
            t("analysis.overview.statCards.daysCount", { count: durationDays })
          }}
        </span>
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.imageMessages')"
      :value="
        t('analysis.overview.statCards.imagesCount', { count: imageCount })
      "
      icon="📸"
      icon-bg="pink"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">{{
          t("analysis.overview.statCards.peakHour")
        }}</span>
        <span class="font-semibold text-pink-500"
          >{{ peakHour?.hour || 0 }}:00</span
        >
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.mostActiveWeekday')"
      :value="peakWeekday ? weekdayNames[peakWeekday.weekday - 1] : '-'"
      icon="📅"
      icon-bg="amber"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">
          {{
            t("analysis.overview.statCards.messagesOnDay", {
              count: peakWeekday?.messageCount ?? 0,
            })
          }}
        </span>
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.weekendActivity')"
      :value="`${weekdayVsWeekend.weekend}%`"
      icon="🏖️"
      icon-bg="green"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">{{
          t("analysis.overview.statCards.weekendRatio")
        }}</span>
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.mostActiveDate')"
      :value="peakDay ? dayjs(peakDay.date).format('MM/DD') : '-'"
      icon="🔥"
      icon-bg="red"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">
          {{
            t("analysis.overview.statCards.messagesOnDay", {
              count: peakDay?.messageCount ?? 0,
            })
          }}
        </span>
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.activeDays')"
      :value="`${activeDays}`"
      icon="📆"
      icon-bg="blue"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">
          {{ t("analysis.overview.statCards.slashDays", { count: totalDays }) }}
        </span>
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.consecutiveStreak')"
      :value="
        t('analysis.overview.statCards.daysStreak', {
          count: maxConsecutiveDays,
        })
      "
      icon="⚡"
      icon-bg="amber"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">{{
          t("analysis.overview.statCards.longestStreak")
        }}</span>
      </template>
    </StatCard>

    <!-- English UI note -->
    <StatCard
      :label="t('analysis.overview.statCards.activityRate')"
      :value="`${activeRate}%`"
      icon="📈"
      icon-bg="gray"
    >
      <template #subtext>
        <span class="text-sm text-gray-500">{{
          t("analysis.overview.statCards.activeDaysRatio")
        }}</span>
      </template>
    </StatCard>
  </div>
</template>
