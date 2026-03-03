<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import * as echarts from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { TooltipComponent } from 'echarts/components'
import 'echarts-wordcloud'

// English engineering note.
echarts.use([CanvasRenderer, TooltipComponent])

export interface WordcloudData {
  words: Array<{
    word: string
    count: number
    percentage?: number
  }>
}

interface Props {
  data: WordcloudData
  height?: number | string
  loading?: boolean
  /** English note.
  maxWords?: number
  /** English note.
  colorScheme?: 'default' | 'warm' | 'cool' | 'rainbow'
  /** English note.
  sizeScale?: number
}

const props = withDefaults(defineProps<Props>(), {
  height: 400,
  loading: false,
  maxWords: 100,
  colorScheme: 'default',
  sizeScale: 1,
})

const emit = defineEmits<{
  /** English note.
  wordClick: [word: string, count: number]
}>()

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
  return document.documentElement.classList.contains('dark')
})

// English engineering note.
const colorSchemes = {
  default: ['#6366f1', '#8b5cf6', '#ec4899', '#f43f5e', '#f97316', '#22c55e', '#14b8a6', '#3b82f6'],
  warm: ['#f97316', '#fb923c', '#fbbf24', '#facc15', '#f59e0b', '#ea580c', '#dc2626', '#ef4444'],
  cool: ['#3b82f6', '#6366f1', '#8b5cf6', '#a855f7', '#14b8a6', '#06b6d4', '#0ea5e9', '#0284c7'],
  rainbow: ['#ef4444', '#f97316', '#eab308', '#22c55e', '#14b8a6', '#3b82f6', '#8b5cf6', '#ec4899'],
}

// English engineering note.
const getOption = () => {
  const words = props.data.words.slice(0, props.maxWords)
  if (words.length === 0) return null

  // English engineering note.
  const maxCount = Math.max(...words.map((w) => w.count))
  const minCount = Math.min(...words.map((w) => w.count))
  const range = maxCount - minCount || 1

  const colors = colorSchemes[props.colorScheme]

  // English engineering note.
  const baseSizeMin = 14
  const baseSizeMax = 56
  // English engineering note.
  const sizeMin = Math.round(baseSizeMin * props.sizeScale)
  const sizeMax = Math.round(baseSizeMax * props.sizeScale)
  const sizeRange = sizeMax - sizeMin

  // English engineering note.
  const seriesData = words.map((item, index) => {
    // English engineering note.
    const normalized = (item.count - minCount) / range
    const fontSize = Math.round(sizeMin + normalized * sizeRange)
    // English engineering note.
    const color = colors[index % colors.length]

    return {
      name: item.word,
      value: item.count,
      textStyle: {
        fontSize,
        color,
      },
    }
  })

  return {
    backgroundColor: 'transparent',
    tooltip: {
      show: true,
      formatter: (params: { name: string; value: number }) => {
        const word = words.find((w) => w.word === params.name)
        const percentage = word?.percentage ? ` (${word.percentage}%)` : ''
        return `${params.name}: ${params.value}次${percentage}`
      },
      backgroundColor: 'rgba(0, 0, 0, 0.8)',
      borderColor: 'transparent',
      textStyle: {
        color: '#fff',
      },
    },
    series: [
      {
        type: 'wordCloud',
        // English engineering note.
        shape: 'circle',
        // English engineering note.
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
        width: '100%',
        height: '100%',
        // English engineering note.
        gridSize: Math.max(2, Math.round(4 * props.sizeScale)),
        // English engineering note.
        sizeRange: [sizeMin, sizeMax],
        // English engineering note.
        rotationRange: [-45, 45],
        rotationStep: 15,
        // English engineering note.
        drawOutOfBound: false,
        // English engineering note.
        layoutAnimation: true,
        // English engineering note.
        textStyle: {
          fontFamily: 'sans-serif',
          fontWeight: 'bold',
        },
        // English engineering note.
        emphasis: {
          focus: 'self',
          textStyle: {
            shadowBlur: 10,
            shadowColor: isDark.value ? 'rgba(255,255,255,0.5)' : 'rgba(0,0,0,0.3)',
          },
        },
        data: seriesData,
      },
    ],
  }
}

// English engineering note.
function initChart() {
  if (!chartRef.value) return

  // English engineering note.
  if (chartInstance) {
    chartInstance.dispose()
  }

  // English engineering note.
  chartInstance = echarts.init(chartRef.value)

  const option = getOption()
  if (option) {
    chartInstance.setOption(option)
  }

  // English engineering note.
  chartInstance.on('click', (params) => {
    if (params.componentType === 'series' && params.seriesType === 'wordCloud') {
      emit('wordClick', params.name, params.value as number)
    }
  })
}

// English engineering note.
function updateChart() {
  if (!chartInstance) {
    initChart()
    return
  }

  const option = getOption()
  if (option) {
    chartInstance.setOption(option, { notMerge: true })
  }
}

// English engineering note.
function handleResize() {
  chartInstance?.resize()
}

// English engineering note.
watch(() => props.data, updateChart, { deep: true })

// English engineering note.
watch(() => props.colorScheme, updateChart)

// English engineering note.
watch(() => props.sizeScale, updateChart)

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
    initChart()
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
