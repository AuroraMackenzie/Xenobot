<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import dayjs from 'dayjs'
import { CalendarDate } from '@internationalized/date'

const { t, locale } = useI18n()

const props = withDefaults(
  defineProps<{
    /** English note.
    modelValue: string
    /** English note.
    placeholder?: string
    /** English note.
    clearable?: boolean
    /** English note.
    widthClass?: string
  }>(),
  {
    placeholder: '',
    clearable: true,
    widthClass: 'w-32',
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

// English engineering note.
const popoverOpen = ref(false)

// English engineering note.
const calendarLocale = computed(() => (locale.value === 'zh-CN' ? 'zh-CN' : 'en-US'))

// English engineering note.
function stringToCalendarDate(dateStr: string): CalendarDate | undefined {
  if (!dateStr) return undefined
  const d = dayjs(dateStr)
  return new CalendarDate(d.year(), d.month() + 1, d.date())
}

// English engineering note.
function calendarDateToString(date: CalendarDate | undefined): string {
  if (!date) return ''
  return `${date.year}-${String(date.month).padStart(2, '0')}-${String(date.day).padStart(2, '0')}`
}

// English engineering note.
const dateObj = computed<CalendarDate | undefined>({
  get: () => stringToCalendarDate(props.modelValue),
  set: (val) => {
    emit('update:modelValue', calendarDateToString(val))
    if (val) popoverOpen.value = false
  },
})

// English engineering note.
const dateDisplay = computed(() => (props.modelValue ? dayjs(props.modelValue).format('YYYY/MM/DD') : ''))

// English engineering note.
function clearDate() {
  emit('update:modelValue', '')
  popoverOpen.value = false
}
</script>

<template>
  <UPopover v-model:open="popoverOpen" :ui="{ content: 'z-[100]' }">
    <UButton
      :label="dateDisplay || placeholder || t('common.datePicker.selectDate')"
      icon="i-heroicons-calendar-days"
      variant="outline"
      color="neutral"
      size="sm"
      :class="[widthClass, 'justify-start', { 'text-gray-400': !dateDisplay }]"
    />
    <template #content>
      <UCalendar v-model="dateObj" :number-of-months="1" :fixed-weeks="false" :locale="calendarLocale" />
      <div v-if="clearable" class="px-2 pb-2">
        <UButton variant="ghost" size="xs" color="neutral" class="w-full" @click="clearDate">
          {{ t('common.datePicker.clearDate') }}
        </UButton>
      </div>
    </template>
  </UPopover>
</template>
