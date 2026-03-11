<script setup lang="ts">
/**
 * English note.
 * English note.
 */

import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    // English engineering note.
    text?: string;
    // English engineering note.
    height?: string;
    // English engineering note.
    variant?: "inline" | "page" | "overlay";
  }>(),
  {
    variant: "inline",
  },
);

// English engineering note.
const containerClass = computed(() => {
  const base = "flex items-center justify-center";

  switch (props.variant) {
    case "page":
      return `${base} h-full w-full`;
    case "overlay":
      return `${base} absolute inset-0 z-10 bg-white/50 backdrop-blur-sm dark:bg-gray-950/50`;
    default:
      return `${base} ${props.height || "py-8"}`;
  }
});
</script>

<template>
  <div :class="containerClass">
    <div class="flex flex-col items-center justify-center text-center">
      <UIcon
        name="i-heroicons-arrow-path"
        class="h-6 w-6 animate-spin text-pink-500"
      />
      <p v-if="text" class="mt-2 text-sm text-gray-500">{{ text }}</p>
    </div>
  </div>
</template>
