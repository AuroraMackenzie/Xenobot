<script setup lang="ts">
/**
 * English note.
 * English note.
 */
import { computed } from 'vue'

const props = defineProps<{
  open: boolean
  imageData: string | null
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const isOpen = computed({
  get: () => props.open,
  set: (value) => emit('update:open', value),
})

function closeModal() {
  isOpen.value = false
}
</script>

<template>
  <UModal v-model:open="isOpen" :ui="{ content: 'max-w-5xl z-100' }">
    <template #content>
      <div class="xeno-capture-shell flex flex-col">
        <!-- Header -->
        <div class="xeno-capture-header flex items-center justify-between px-6 py-4">
          <div class="flex items-center gap-3">
            <div
              class="xeno-capture-icon flex h-10 w-10 items-center justify-center rounded-xl"
            >
              <UIcon name="i-heroicons-camera" class="h-5 w-5 text-white" />
            </div>
            <div class="min-w-0">
              <h2 class="break-words text-lg font-semibold text-gray-900 dark:text-white">截图预览</h2>
            </div>
          </div>
          <UButton icon="i-heroicons-x-mark" variant="ghost" color="neutral" size="sm" @click="closeModal" />
        </div>

        <!-- Image Preview -->
        <div class="xeno-capture-body p-4">
          <div class="xeno-capture-frame mx-auto max-h-[70vh] overflow-auto rounded-lg">
            <img v-if="imageData" :src="imageData" alt="截图预览" class="block w-full" />
          </div>
        </div>
      </div>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-capture-shell {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.6rem;
  background:
    radial-gradient(circle at top left, rgba(255, 122, 172, 0.12), transparent 24%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 24%),
    rgba(7, 18, 29, 0.95);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 30px 74px rgba(2, 8, 16, 0.36);
  backdrop-filter: blur(22px) saturate(132%);
}

.xeno-capture-header {
  border-bottom: 1px solid rgba(139, 166, 189, 0.16);
}

.xeno-capture-icon {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    linear-gradient(135deg, rgba(255, 122, 172, 0.92), rgba(219, 39, 119, 0.9));
  border: 1px solid rgba(255, 163, 201, 0.22);
  box-shadow: 0 14px 32px rgba(21, 7, 15, 0.24);
}

.xeno-capture-body {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent 120%),
    rgba(6, 16, 24, 0.48);
}

.xeno-capture-frame {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.68);
  box-shadow: 0 18px 44px rgba(2, 8, 16, 0.28);
}
</style>
