import { computed, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AnalysisSession, MessageType } from '@/types/base'
import type { HourlyActivity, DailyActivity, WeekdayActivity } from '@/types/analysis'
import dayjs from 'dayjs'

interface UseOverviewStatisticsProps {
  session: AnalysisSession
  messageTypes: Array<{ type: MessageType; count: number }>
  hourlyActivity: HourlyActivity[]
  dailyActivity: DailyActivity[]
  timeRange: { start: number; end: number } | null
  selectedYear: number | null
  filteredMessageCount: number
  filteredMemberCount: number
}

export function useOverviewStatistics(props: UseOverviewStatisticsProps, weekdayActivity: Ref<WeekdayActivity[]>) {
  const { t } = useI18n()

  // English engineering note.
  const durationDays = computed(() => {
    if (props.selectedYear) {
      const isLeapYear =
        (props.selectedYear % 4 === 0 && props.selectedYear % 100 !== 0) || props.selectedYear % 400 === 0
      return isLeapYear ? 366 : 365
    }
    if (!props.timeRange) return 0
    return Math.ceil((props.timeRange.end - props.timeRange.start) / 86400)
  })

  // English engineering note.
  const displayMessageCount = computed(() => {
    return props.selectedYear ? props.filteredMessageCount : props.session.messageCount
  })

  // English engineering note.
  const displayMemberCount = computed(() => {
    return props.selectedYear ? props.filteredMemberCount : props.session.memberCount
  })

  // English engineering note.
  const totalDurationDays = computed(() => {
    if (!props.timeRange) return 0
    return Math.ceil((props.timeRange.end - props.timeRange.start) / 86400)
  })

  // English engineering note.
  const totalDailyAvgMessages = computed(() => {
    if (totalDurationDays.value === 0) return 0
    return Math.round(props.session.messageCount / totalDurationDays.value)
  })

  // English engineering note.
  const dailyAvgMessages = computed(() => {
    if (durationDays.value === 0) return 0
    return Math.round(displayMessageCount.value / durationDays.value)
  })

  // English engineering note.
  const imageCount = computed(() => {
    const imageType = props.messageTypes.find((t) => t.type === 1)
    return imageType?.count || 0
  })

  // English engineering note.
  const peakHour = computed(() => {
    if (!props.hourlyActivity.length) return null
    return props.hourlyActivity.reduce(
      (max, h) => (h.messageCount > max.messageCount ? h : max),
      props.hourlyActivity[0]
    )
  })

  // English engineering note.
  const peakWeekday = computed(() => {
    if (!weekdayActivity.value.length) return null
    return weekdayActivity.value.reduce(
      (max, w) => (w.messageCount > max.messageCount ? w : max),
      weekdayActivity.value[0]
    )
  })

  // English engineering note.
  const weekdayNames = computed(() => [
    t('common.weekday.mon'),
    t('common.weekday.tue'),
    t('common.weekday.wed'),
    t('common.weekday.thu'),
    t('common.weekday.fri'),
    t('common.weekday.sat'),
    t('common.weekday.sun'),
  ])

  // English engineering note.
  const weekdayVsWeekend = computed(() => {
    if (!weekdayActivity.value.length) return { weekday: 0, weekend: 0 }
    const weekdaySum = weekdayActivity.value
      .filter((w) => w.weekday >= 1 && w.weekday <= 5)
      .reduce((sum, w) => sum + w.messageCount, 0)
    const weekendSum = weekdayActivity.value
      .filter((w) => w.weekday >= 6 && w.weekday <= 7)
      .reduce((sum, w) => sum + w.messageCount, 0)
    const total = weekdaySum + weekendSum
    return {
      weekday: total > 0 ? Math.round((weekdaySum / total) * 100) : 0,
      weekend: total > 0 ? Math.round((weekendSum / total) * 100) : 0,
    }
  })

  // English engineering note.
  const peakDay = computed(() => {
    if (!props.dailyActivity.length) return null
    return props.dailyActivity.reduce((max, d) => (d.messageCount > max.messageCount ? d : max), props.dailyActivity[0])
  })

  // English engineering note.
  const activeDays = computed(() => {
    return props.dailyActivity.filter((d) => d.messageCount > 0).length
  })

  // English engineering note.
  const totalDays = computed(() => {
    if (!props.timeRange) return 0
    const start = dayjs.unix(props.timeRange.start)
    const end = dayjs.unix(props.timeRange.end)
    return end.diff(start, 'day') + 1
  })

  // English engineering note.
  const activeRate = computed(() => {
    return totalDays.value > 0 ? Math.round((activeDays.value / totalDays.value) * 100) : 0
  })

  // English engineering note.
  const maxConsecutiveDays = computed(() => {
    if (!props.dailyActivity.length) return 0

    // English engineering note.
    const sortedDates = [...props.dailyActivity]
      .filter((d) => d.messageCount > 0)
      .sort((a, b) => dayjs(a.date).valueOf() - dayjs(b.date).valueOf())

    if (sortedDates.length === 0) return 0

    let maxStreak = 1
    let currentStreak = 1

    for (let i = 1; i < sortedDates.length; i++) {
      const prevDate = dayjs(sortedDates[i - 1].date)
      const currDate = dayjs(sortedDates[i].date)

      // English engineering note.
      if (currDate.diff(prevDate, 'day') === 1) {
        currentStreak++
      } else {
        if (currentStreak > maxStreak) {
          maxStreak = currentStreak
        }
        currentStreak = 1
      }
    }

    // English engineering note.
    if (currentStreak > maxStreak) {
      maxStreak = currentStreak
    }

    return maxStreak
  })

  return {
    durationDays,
    displayMessageCount,
    displayMemberCount,
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
  }
}
