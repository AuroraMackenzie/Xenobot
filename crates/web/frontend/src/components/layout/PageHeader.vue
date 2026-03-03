<script setup lang="ts">
/**
 * Shared page header component.
 * Includes title, description, optional avatar/icon, and slots for extra actions.
 */

defineProps<{
  title: string
  description?: string
  icon?: string // Fallback icon
  iconClass?: string // Icon background style class
  avatar?: string | null // Avatar image (base64 Data URL), preferred over icon
}>()
</script>

<template>
  <div class="relative border-b border-gray-200/50 px-6 pb-2 dark:border-gray-800/50">
    <!-- Drag zone covering top safe-area with platform-aware height.
         macOS: 16px padding + 16px = 32px; Windows/Linux: 32px padding + 16px = 48px -->
    <div class="titlebar-drag-cover" />

    <!-- English UI note -->
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        <!-- English UI note -->
        <img v-if="avatar" :src="avatar" :alt="title" class="h-10 w-10 rounded-xl object-cover" />
        <!-- English UI note -->
        <div v-else-if="icon" class="flex h-10 w-10 items-center justify-center rounded-xl" :class="iconClass">
          <UIcon :name="icon" class="h-5 w-5 text-white" />
        </div>
        <div>
          <h1 class="text-lg font-semibold text-gray-900 dark:text-white">
            {{ title }}
          </h1>
          <p v-if="description" class="text-xs text-gray-500 dark:text-gray-400">
            {{ description }}
          </p>
        </div>
      </div>

      <!-- English UI note -->
      <div class="flex-1 self-stretch mx-4" style="-webkit-app-region: drag" />

      <!-- English UI note -->
      <div class="flex items-center gap-2">
        <slot name="actions" />
      </div>
    </div>

    <!-- English UI note -->
    <slot />
  </div>
</template>

<style scoped>
/* Drag cover for the title bar using CSS variables for platform-adaptive height. */
.titlebar-drag-cover {
  position: absolute;
  left: 0;
  right: 0;
  z-index: 50;
  top: calc(-1 * var(--titlebar-area-height));
  height: calc(var(--titlebar-area-height) + 1rem);
  -webkit-app-region: drag;
}
</style>
