<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { computed } from 'vue'
import type { EChartsOption, BarSeriesOption } from 'echarts'
import EChart from './EChart.vue'
import type { RankItem } from './RankList.vue'
import { SectionCard, ScrollableChart } from '@/components/UI'

interface Props {
  /** English note.
  members: RankItem[]
  /** English note.
  title: string
  /** English note.
  description?: string
  /** English note.
  topN?: number
  /** English note.
  unit?: string
  /** English note.
  height?: 'auto' | number
  /** English note.
  maxHeightVh?: number
  /** English note.
  bare?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  topN: 10,
  unit: '条',
  height: 'auto',
  maxHeightVh: 60,
  bare: false,
})

// English engineering note.
const displayData = computed(() => {
  return props.members.slice(0, props.topN)
})

// English engineering note.
const chartHeight = computed(() => {
  if (props.height !== 'auto') {
    return props.height
  }
  // English engineering note.
  const dataHeight = displayData.value.length * 36
  // English engineering note.
  return Math.max(dataHeight + 30, 180)
})

// English engineering note.
const barColor = {
  type: 'linear' as const,
  x: 0,
  y: 0,
  x2: 1,
  y2: 0,
  colorStops: [
    { offset: 0, color: '#0ea5c9' }, // English engineering note.
    { offset: 1, color: '#44b9d4' }, // English engineering note.
  ],
}

// English engineering note.
function truncateName(name: string, maxLength = 8): string {
  if (name.length <= maxLength) return name
  return name.slice(0, maxLength) + '…'
}

// English engineering note.
const option = computed<EChartsOption>(() => {
  // English engineering note.
  const reversedData = [...displayData.value].reverse()
  const names = reversedData.map((item) => truncateName(item.name))
  const values = reversedData.map((item) => item.value)
  const maxValue = Math.max(...values, 1)

  // English engineering note.
  const dataWithStyle = reversedData.map((item) => ({
    value: item.value,
    itemStyle: {
      color: barColor,
      borderRadius: [0, 4, 4, 0],
    },
  }))

  return {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'shadow',
      },
      backgroundColor: 'rgba(0, 0, 0, 0.8)',
      borderColor: 'transparent',
      textStyle: {
        color: '#fff',
      },
      formatter: (params: any) => {
        const data = params[0]
        if (!data) return ''
        const originalIndex = displayData.value.length - 1 - data.dataIndex
        const member = displayData.value[originalIndex]
        return `
          <div style="padding: 4px 8px;">
            <div style="font-weight: bold; margin-bottom: 4px;">${member.name}</div>
            <div>${member.value} ${props.unit} (${member.percentage}%)</div>
          </div>
        `
      },
    },
    grid: {
      left: 110,
      right: 70,
      top: 15,
      bottom: 15,
      containLabel: false,
    },
    xAxis: {
      type: 'value',
      max: maxValue * 1.1, // English engineering note.
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { show: false },
      splitLine: { show: false },
    },
    yAxis: {
      type: 'category',
      data: names,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        fontSize: 12,
        color: '#4b5563',
        margin: 12,
        formatter: (value: string, index: number) => {
          const originalIndex = displayData.value.length - 1 - index
          const rank = originalIndex + 1
          // English engineering note.
          const prefix = rank === 1 ? '🥇' : rank === 2 ? '🥈' : rank === 3 ? '🥉' : `${rank}.`
          return `${prefix} ${value}`
        },
      },
    },
    series: [
      {
        type: 'bar',
        data: dataWithStyle,
        barWidth: 18,
        barCategoryGap: '30%',
        label: {
          show: true,
          position: 'right',
          distance: 8,
          formatter: (params: any) => {
            const originalIndex = displayData.value.length - 1 - params.dataIndex
            const member = displayData.value[originalIndex]
            return `${member.value} ${props.unit}`
          },
          fontSize: 11,
          fontWeight: 500,
          color: '#6b7280',
        },
        emphasis: {
          itemStyle: {
            shadowBlur: 6,
            shadowColor: 'rgba(14, 165, 201, 0.32)',
          },
        },
      } as BarSeriesOption,
    ],
  }
})
</script>

<template>
  <!-- English UI note -->
  <ScrollableChart v-if="bare" :content-height="chartHeight" :max-height-vh="maxHeightVh">
    <EChart :option="option" :height="chartHeight" />
  </ScrollableChart>
  <!-- English UI note -->
  <SectionCard v-else :title="title" :description="description" scrollable :max-height-vh="maxHeightVh">
    <div class="px-3 py-2">
      <EChart :option="option" :height="chartHeight" />
    </div>
  </SectionCard>
</template>
