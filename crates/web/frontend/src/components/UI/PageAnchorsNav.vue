<script setup lang="ts">
import type { AnchorItem } from "@/composables";

withDefaults(
  defineProps<{
    // English engineering note.
    anchors: AnchorItem[];
    // English engineering note.
    activeAnchor: string;
    // English engineering note.
    width?: string;
    // English engineering note.
    hideOnMobile?: boolean;
  }>(),
  {
    width: "w-44",
    hideOnMobile: true,
  },
);

const emit = defineEmits<{
  // English engineering note.
  click: [id: string];
}>();

function handleClick(id: string) {
  emit("click", id);
}
</script>

<template>
  <div :class="[width, 'shrink-0', hideOnMobile ? 'hidden lg:block' : '']">
    <div class="sticky top-24 space-y-6">
      <nav>
        <div class="border-l border-gray-200 dark:border-gray-800">
          <button
            v-for="anchor in anchors"
            :key="anchor.id"
            class="-ml-px block border-l-2 py-1.5 pl-4 text-left text-sm transition-colors"
            :class="[
              activeAnchor === anchor.id
                ? 'border-pink-500 font-medium text-pink-600 dark:text-pink-400'
                : 'border-transparent text-gray-500 hover:border-gray-300 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300',
            ]"
            @click="handleClick(anchor.id)"
          >
            {{ anchor.label }}
          </button>
        </div>
      </nav>
      <!-- English UI note -->
      <slot />
    </div>
  </div>
</template>
