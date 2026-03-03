<script setup lang="ts">
/**
 * English note.
 * English note.
 */
defineProps<{
  /** English note.
  label: string
  /** English note.
  value: string | number
  /** English note.
  valueColor?: 'pink' | 'amber' | 'blue' | 'green' | 'red' | 'gray'
  /** English note.
  subtext?: string
  /** English note.
  icon?: string
  /** English note.
  iconBg?: 'pink' | 'amber' | 'blue' | 'green' | 'red' | 'gray'
}>()

// English engineering note.
const valueColorMap: Record<string, string> = {
  pink: 'text-pink-600 dark:text-pink-400',
  amber: 'text-amber-600 dark:text-amber-400',
  blue: 'text-blue-600 dark:text-blue-400',
  green: 'text-green-600 dark:text-green-400',
  red: 'text-red-600 dark:text-red-400',
  gray: 'text-gray-900 dark:text-white',
}

const iconBgMap: Record<string, string> = {
  pink: 'bg-pink-100 dark:bg-pink-900/30',
  amber: 'bg-amber-100 dark:bg-amber-900/30',
  blue: 'bg-blue-100 dark:bg-blue-900/30',
  green: 'bg-green-100 dark:bg-green-900/30',
  red: 'bg-red-100 dark:bg-red-900/30',
  gray: 'bg-gray-100 dark:bg-gray-800',
}
</script>

<template>
  <div class="rounded-xl border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-800 dark:bg-gray-900">
    <!-- English UI note -->
    <template v-if="icon">
      <div class="flex items-center gap-3">
        <div
          class="flex h-12 w-12 items-center justify-center rounded-xl text-2xl"
          :class="iconBgMap[iconBg || 'gray']"
        >
          {{ icon }}
        </div>
        <div>
          <p class="text-xs font-medium text-gray-500 dark:text-gray-400">{{ label }}</p>
          <p class="text-lg font-bold text-gray-900 dark:text-white">{{ value }}</p>
        </div>
      </div>
      <div v-if="subtext || $slots.subtext" class="mt-3 flex items-baseline gap-1">
        <slot name="subtext">
          <span class="text-sm text-gray-500">{{ subtext }}</span>
        </slot>
      </div>
    </template>

    <!-- English UI note -->
    <template v-else>
      <p class="text-xs font-medium text-gray-500 dark:text-gray-400">{{ label }}</p>
      <p class="mt-1 text-2xl font-bold" :class="valueColorMap[valueColor || 'pink']">{{ value }}</p>
      <p v-if="subtext || $slots.subtext" class="mt-1 text-xs text-gray-400">
        <slot name="subtext">{{ subtext }}</slot>
      </p>
    </template>
  </div>
</template>
