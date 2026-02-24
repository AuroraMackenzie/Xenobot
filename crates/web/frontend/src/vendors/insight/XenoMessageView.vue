<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { EChartLine, EChartPie } from '@/components/charts'
import { EmptyState, LoadingState, SectionCard } from '@/components/UI'

interface TimeFilter {
  startTs?: number
  endTs?: number
  memberId?: number | null
}

interface HourlyActivity {
  hour: number
  messageCount: number
}

interface DailyActivity {
  date: string
  messageCount: number
}

interface MessageTypeItem {
  type: number
  count: number
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

const { t } = useI18n()

const isLoading = ref(false)
const hourly = ref<HourlyActivity[]>([])
const daily = ref<DailyActivity[]>([])
const messageTypes = ref<MessageTypeItem[]>([])

function messageTypeLabel(type: number) {
  if (type === 1) return 'Image'
  if (type === 2) return 'Audio'
  if (type === 3) return 'Video'
  if (type === 4) return 'File'
  if (type === 5) return 'Link'
  return 'Text'
}

async function loadData() {
  if (!props.sessionId) return
  isLoading.value = true
  try {
    const [hourlyData, dailyData, typeData] = await Promise.all([
      window.chatApi.getHourlyActivity(props.sessionId, props.timeFilter),
      window.chatApi.getDailyActivity(props.sessionId, props.timeFilter),
      window.chatApi.getMessageTypeDistribution(props.sessionId, props.timeFilter),
    ])
    hourly.value = Array.isArray(hourlyData) ? hourlyData : []
    daily.value = Array.isArray(dailyData) ? dailyData : []
    messageTypes.value = Array.isArray(typeData) ? typeData : []
  } catch (error) {
    console.error('XenoMessageView load failed:', error)
    hourly.value = []
    daily.value = []
    messageTypes.value = []
  } finally {
    isLoading.value = false
  }
}

watch(
  () => [props.sessionId, props.timeFilter],
  () => {
    loadData()
  },
  { immediate: true, deep: true }
)

const hasData = computed(() => daily.value.length > 0 || hourly.value.length > 0 || messageTypes.value.length > 0)

const totalMessages = computed(() => {
  const byTypes = messageTypes.value.reduce((sum, item) => sum + (item.count || 0), 0)
  if (byTypes > 0) return byTypes
  return daily.value.reduce((sum, item) => sum + (item.messageCount || 0), 0)
})

const peakHour = computed(() => {
  if (hourly.value.length === 0) return '--'
  const top = [...hourly.value].sort((a, b) => b.messageCount - a.messageCount)[0]
  return `${String(top.hour).padStart(2, '0')}:00`
})

const activeDays = computed(() => daily.value.filter((d) => d.messageCount > 0).length)

const lineData = computed(() => {
  const recent = daily.value.slice(-45)
  return {
    labels: recent.map((item) => item.date),
    values: recent.map((item) => item.messageCount),
  }
})

const pieData = computed(() => ({
  labels: messageTypes.value.map((item) => messageTypeLabel(item.type)),
  values: messageTypes.value.map((item) => item.count),
}))
</script>

<template>
  <div class="main-content space-y-5 p-6">
    <LoadingState v-if="isLoading" :text="t('common.loading')" />

    <template v-else-if="hasData">
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <div class="rounded-xl border border-cyan-100 bg-white/80 p-4 dark:border-cyan-500/20 dark:bg-slate-900/60">
          <p class="text-xs font-semibold tracking-wide text-slate-500 dark:text-slate-400">TOTAL MESSAGES</p>
          <p class="mt-2 text-2xl font-black text-cyan-600 dark:text-cyan-300">
            {{ totalMessages.toLocaleString() }}
          </p>
        </div>
        <div class="rounded-xl border border-amber-100 bg-white/80 p-4 dark:border-amber-500/20 dark:bg-slate-900/60">
          <p class="text-xs font-semibold tracking-wide text-slate-500 dark:text-slate-400">PEAK HOUR</p>
          <p class="mt-2 text-2xl font-black text-amber-600 dark:text-amber-300">{{ peakHour }}</p>
        </div>
        <div class="rounded-xl border border-teal-100 bg-white/80 p-4 dark:border-teal-500/20 dark:bg-slate-900/60">
          <p class="text-xs font-semibold tracking-wide text-slate-500 dark:text-slate-400">ACTIVE DAYS</p>
          <p class="mt-2 text-2xl font-black text-teal-600 dark:text-teal-300">{{ activeDays }}</p>
        </div>
      </div>

      <div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <SectionCard title="Message Timeline">
          <div class="p-4">
            <EChartLine :data="lineData" :height="280" />
          </div>
        </SectionCard>
        <SectionCard title="Content Mix">
          <div class="p-4">
            <EChartPie :data="pieData" :height="280" />
          </div>
        </SectionCard>
      </div>
    </template>

    <SectionCard v-else title="Message View">
      <EmptyState :text="t('common.noData')" />
    </SectionCard>
  </div>
</template>
