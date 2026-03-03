<script setup lang="ts">
/**
 * English note.
 * English note.
 */
defineProps<{
  /** English note.
  text?: string
  /** English note.
  icon?: string
  /** English note.
  padding?: 'sm' | 'md' | 'lg'
}>()

const paddingMap = {
  sm: 'px-4 py-4',
  md: 'px-5 py-8',
  lg: 'px-6 py-12',
}
</script>

<template>
  <div class="text-center text-sm text-gray-400" :class="paddingMap[padding || 'md']">
    <span v-if="icon" class="mb-2 block text-2xl">{{ icon }}</span>
    {{ text || '暂无数据' }}
  </div>
</template>
