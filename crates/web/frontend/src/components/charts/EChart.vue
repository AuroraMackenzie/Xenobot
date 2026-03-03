<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import * as echarts from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { PieChart, BarChart, LineChart, HeatmapChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
} from 'echarts/components'
import type { EChartsOption } from 'echarts'

// English engineering note.
echarts.use([
  CanvasRenderer,
  PieChart,
  BarChart,
  LineChart,
  HeatmapChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
])

interface Props {
  option: EChartsOption
  height?: number | string
  loading?: boolean
  theme?: 'light' | 'dark' | 'auto'
}

const props = withDefaults(defineProps<Props>(), {
  height: 300,
  loading: false,
  theme: 'auto',
})

const chartRef = ref<HTMLDivElement>()
let chartInstance: echarts.ECharts | null = null

// English engineering note.
const heightStyle = computed(() => {
  if (typeof props.height === 'number') {
    return `${props.height}px`
  }
  return props.height
})

// English engineering note.
const isDark = computed(() => {
  if (props.theme === 'auto') {
    return document.documentElement.classList.contains('dark')
  }
  return props.theme === 'dark'
})

// English engineering note.
function initChart() {
  if (!chartRef.value) return

  // English engineering note.
  if (chartInstance) {
    chartInstance.dispose()
  }

  // English engineering note.
  chartInstance = echarts.init(chartRef.value, isDark.value ? 'dark' : undefined)
  chartInstance.setOption(props.option)
}

// English engineering note.
function updateChart() {
  if (!chartInstance) {
    initChart()
    return
  }
  chartInstance.setOption(props.option, { notMerge: true })
}

// English engineering note.
function handleResize() {
  chartInstance?.resize()
}

// English engineering note.
watch(() => props.option, updateChart, { deep: true })

// English engineering note.
watch(
  () => props.height,
  () => {
    // English engineering note.
    setTimeout(() => {
      chartInstance?.resize()
    }, 0)
  }
)

// English engineering note.
watch(isDark, () => {
  initChart()
})

// English engineering note.
watch(
  () => props.loading,
  (loading) => {
    if (loading) {
      chartInstance?.showLoading('default', {
        text: '',
        spinnerRadius: 12,
        lineWidth: 2,
      })
    } else {
      chartInstance?.hideLoading()
    }
  }
)

// English engineering note.
let observer: MutationObserver | null = null

onMounted(() => {
  initChart()
  window.addEventListener('resize', handleResize)

  // English engineering note.
  observer = new MutationObserver(() => {
    if (props.theme === 'auto') {
      initChart()
    }
  })
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class'],
  })
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  observer?.disconnect()
  chartInstance?.dispose()
  chartInstance = null
})

// English engineering note.
defineExpose({
  getInstance: () => chartInstance,
  resize: handleResize,
})
</script>

<template>
  <div ref="chartRef" :style="{ height: heightStyle, width: '100%' }" />
</template>
