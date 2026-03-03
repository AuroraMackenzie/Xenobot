<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { computed } from 'vue'

interface Props {
  /** English note.
  contentHeight: number
  /** English note.
  maxHeightVh?: number
  /** English note.
  paddingClass?: string
}

const props = withDefaults(defineProps<Props>(), {
  maxHeightVh: 60,
  paddingClass: 'px-3 py-2',
})

// English engineering note.
const maxHeightPx = computed(() => {
  if (typeof window === 'undefined') return 500
  return Math.round((window.innerHeight * props.maxHeightVh) / 100)
})

// English engineering note.
const needScroll = computed(() => props.contentHeight > maxHeightPx.value)

// English engineering note.
const containerStyle = computed(() => ({
  maxHeight: `${props.maxHeightVh}vh`,
  overflowY: (needScroll.value ? 'auto' : 'hidden') as 'auto' | 'hidden',
}))
</script>

<template>
  <div :class="paddingClass" :style="containerStyle">
    <slot />
  </div>
</template>
