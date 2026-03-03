<script setup lang="ts">
/**
 * English note.
 * English note.
 */
defineProps<{
  /** English note.
  icon?: string
  /** English note.
  content: string
  /** English note.
  color?: 'error' | 'warning' | 'info' | 'success'
}>()
</script>

<template>
  <UAlert :color="color || 'error'" variant="outline" :icon="icon || 'i-lucide-terminal'" class="p-2">
    <template #title>
      <p v-html="content" />
    </template>
  </UAlert>
</template>
