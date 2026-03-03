<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    /** English note.
    title: string
    /** English note.
    description?: string
    /** English note.
    showDivider?: boolean
    /** English note.
    scrollable?: boolean
    /** English note.
    maxHeightVh?: number
  }>(),
  {
    showDivider: true,
    scrollable: false,
    maxHeightVh: 60,
  }
)

// English engineering note.
const contentStyle = computed(() => {
  if (!props.scrollable) return undefined
  return {
    maxHeight: `${props.maxHeightVh}vh`,
    overflowY: 'auto' as const,
  }
})
</script>

<template>
  <div class="rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-800 dark:bg-gray-900">
    <!-- English UI note -->
    <div class="px-5 py-3" :class="{ 'border-b border-gray-200 dark:border-gray-800': showDivider && $slots.default }">
      <div class="flex items-center justify-between">
        <div>
          <p class="font-semibold text-gray-900 dark:text-white whitespace-nowrap">{{ title }}</p>
          <p v-if="description" class="mt-1 text-sm text-gray-500 dark:text-gray-400">
            {{ description }}
          </p>
        </div>
        <!-- English UI note -->
        <slot name="headerRight" />
      </div>
    </div>

    <!-- English UI note -->
    <div v-if="scrollable" :style="contentStyle">
      <slot />
    </div>
    <slot v-else />

    <!-- English UI note -->
    <slot name="footer" />
  </div>
</template>
