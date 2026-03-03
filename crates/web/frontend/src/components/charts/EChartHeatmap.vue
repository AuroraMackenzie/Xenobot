<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { computed } from 'vue'
import type { EChartsOption } from 'echarts'
import EChart from './EChart.vue'

export interface EChartHeatmapData {
  /** English note.
  xLabels: string[]
  /** English note.
  yLabels: string[]
  /** English note.
  data: Array<[number, number, number]>
}

interface Props {
  data: EChartHeatmapData
  height?: number
  /** English note.
  minColor?: string
  /** English note.
  maxColor?: string
}

const props = withDefaults(defineProps<Props>(), {
  height: 280,
  minColor: '#d6f1f7', // English engineering note.
  maxColor: '#0ea5c9', // English engineering note.
})

// English engineering note.
const maxValue = computed(() => {
  let max = 0
  for (const [, , value] of props.data.data) {
    if (value > max) max = value
  }
  return max || 1
})

const option = computed<EChartsOption>(() => {
  return {
    tooltip: {
      position: 'top',
      formatter: (params: any) => {
        const xLabel = props.data.xLabels[params.data[0]]
        const yLabel = props.data.yLabels[params.data[1]]
        const value = params.data[2]
        return `${yLabel} ${xLabel}<br/>消息数: <strong>${value}</strong>`
      },
      backgroundColor: 'rgba(0, 0, 0, 0.8)',
      borderColor: 'transparent',
      textStyle: {
        color: '#fff',
      },
    },
    grid: {
      left: 60,
      right: 20,
      top: 20,
      bottom: 60,
    },
    xAxis: {
      type: 'category',
      data: props.data.xLabels,
      splitArea: {
        show: true,
      },
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        fontSize: 11,
        color: '#6b7280',
        interval: 0,
      },
    },
    yAxis: {
      type: 'category',
      data: props.data.yLabels,
      splitArea: {
        show: true,
      },
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        fontSize: 11,
        color: '#6b7280',
      },
    },
    visualMap: {
      min: 0,
      max: maxValue.value,
      calculable: true,
      orient: 'horizontal',
      left: 'center',
      bottom: 0,
      itemWidth: 10,
      itemHeight: 120,
      inRange: {
        color: [props.minColor, props.maxColor],
      },
      textStyle: {
        color: '#6b7280',
        fontSize: 11,
      },
    },
    series: [
      {
        type: 'heatmap',
        data: props.data.data,
        label: {
          show: false,
        },
        emphasis: {
          itemStyle: {
            shadowBlur: 10,
            shadowColor: 'rgba(0, 0, 0, 0.5)',
          },
        },
      },
    ],
  }
})
</script>

<template>
  <EChart :option="option" :height="height" />
</template>
