<script setup lang="ts">
/**
 * English note.
 */
import { computed } from "vue";
import type { EChartsOption } from "echarts";
import EChart from "./EChart.vue";

export interface EChartBarData {
  labels: string[];
  values: number[];
}

interface Props {
  data: EChartBarData;
  height?: number;
  // English engineering note.
  horizontal?: boolean;
  // English engineering note.
  gradient?: boolean;
  // English engineering note.
  borderRadius?: number;
}

const props = withDefaults(defineProps<Props>(), {
  height: 200,
  horizontal: false,
  gradient: true,
  borderRadius: 4,
});

// English engineering note.
const gradientColor = {
  type: "linear" as const,
  x: 0,
  y: 0,
  x2: 0,
  y2: 1,
  colorStops: [
    { offset: 0, color: "#0ea5c9" }, // English engineering note.
    { offset: 1, color: "#44b9d4" }, // English engineering note.
  ],
};

const option = computed<EChartsOption>(() => {
  const isHorizontal = props.horizontal;

  return {
    tooltip: {
      trigger: "axis",
      axisPointer: {
        type: "shadow",
      },
      backgroundColor: "rgba(0, 0, 0, 0.8)",
      borderColor: "transparent",
      textStyle: {
        color: "#fff",
      },
    },
    grid: {
      left: isHorizontal ? 60 : 40,
      right: 20,
      top: 20,
      bottom: isHorizontal ? 20 : 30,
      containLabel: false,
    },
    xAxis: isHorizontal
      ? {
          type: "value",
          axisLine: { show: false },
          axisTick: { show: false },
          splitLine: {
            lineStyle: {
              type: "dashed",
              color: "#e5e7eb",
            },
          },
        }
      : {
          type: "category",
          data: props.data.labels,
          axisLine: { show: false },
          axisTick: { show: false },
          axisLabel: {
            fontSize: 11,
            color: "#6b7280",
          },
        },
    yAxis: isHorizontal
      ? {
          type: "category",
          data: props.data.labels,
          axisLine: { show: false },
          axisTick: { show: false },
          axisLabel: {
            fontSize: 11,
            color: "#6b7280",
          },
        }
      : {
          type: "value",
          axisLine: { show: false },
          axisTick: { show: false },
          splitLine: {
            lineStyle: {
              type: "dashed",
              color: "#e5e7eb",
            },
          },
        },
    series: [
      {
        type: "bar",
        data: props.data.values,
        itemStyle: {
          color: props.gradient ? gradientColor : "#0ea5c9",
          borderRadius: props.borderRadius,
        },
        barMaxWidth: 40,
        emphasis: {
          itemStyle: {
            color: props.gradient
              ? {
                  ...gradientColor,
                  colorStops: [
                    { offset: 0, color: "#0a88ac" }, // English engineering note.
                    { offset: 1, color: "#0ea5c9" }, // English engineering note.
                  ],
                }
              : "#0a88ac",
          },
        },
      },
    ],
  };
});
</script>

<template>
  <EChart :option="option" :height="height" />
</template>
