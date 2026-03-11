import { computed } from "vue";
import type { DailyActivity } from "@/types/analysis";
import type { EChartLineData } from "@/components/charts";
import dayjs from "dayjs";

export function useDailyTrend(dailyActivity: DailyActivity[]) {
  // English engineering note.
  const isMultiYear = computed(() => {
    if (dailyActivity.length < 2) return false;
    const years = new Set(dailyActivity.map((d) => dayjs(d.date).year()));
    return years.size > 1;
  });

  // English engineering note.
  const dailyChartData = computed<EChartLineData>(() => {
    const rawData = dailyActivity;
    const maxPoints = 50; // English engineering note.

    if (rawData.length <= maxPoints) {
      const dateFormat = isMultiYear.value ? "YYYY/MM/DD" : "MM/DD";
      return {
        labels: rawData.map((d) => dayjs(d.date).format(dateFormat)),
        values: rawData.map((d) => d.messageCount),
      };
    }

    // English engineering note.
    const groupSize = Math.ceil(rawData.length / maxPoints);
    const aggregatedLabels: string[] = [];
    const aggregatedValues: number[] = [];

    for (let i = 0; i < rawData.length; i += groupSize) {
      const chunk = rawData.slice(i, i + groupSize);
      if (chunk.length === 0) continue;

      const midIndex = Math.floor(chunk.length / 2);
      const midDate = chunk[midIndex].date;
      const dateFormat = isMultiYear.value ? "YYYY/MM/DD" : "MM/DD";
      aggregatedLabels.push(dayjs(midDate).format(dateFormat));

      const totalMessages = chunk.reduce((sum, d) => sum + d.messageCount, 0);
      const avgMessages = Math.round(totalMessages / chunk.length);
      aggregatedValues.push(avgMessages);
    }

    return {
      labels: aggregatedLabels,
      values: aggregatedValues,
    };
  });

  return {
    isMultiYear,
    dailyChartData,
  };
}
