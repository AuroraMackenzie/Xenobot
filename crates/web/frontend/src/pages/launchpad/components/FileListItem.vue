<script setup lang="ts">
/**
 * English note.
 * English note.
 */
defineProps<{
  /** English note.
  name: string
  /** English note.
  statusIcon: string
  /** English note.
  statusClass: string
  /** English note.
  progressText?: string
  /** English note.
  index: number
  /** English note.
  total: number
  /** English note.
  highlight?: boolean
}>()
</script>

<template>
  <div
    class="flex items-center gap-3 rounded-lg px-3 py-2 transition-colors"
    :class="{ 'bg-pink-50/50 dark:bg-pink-500/5': highlight }"
  >
    <UIcon :name="statusIcon" class="h-5 w-5 shrink-0" :class="statusClass" />
    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium text-gray-900 dark:text-white">
        {{ name }}
      </p>
      <p v-if="progressText" class="text-xs text-gray-500 dark:text-gray-400">
        {{ progressText }}
      </p>
      <!-- English UI note -->
      <slot name="extra" />
    </div>
    <span class="text-xs text-gray-400">{{ index + 1 }}/{{ total }}</span>
    <!-- English UI note -->
    <slot name="action" />
  </div>
</template>
