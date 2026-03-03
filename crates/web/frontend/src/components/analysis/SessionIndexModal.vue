<script setup lang="ts">
/**
 * English note.
 * English note.
 * English note.
 */
import { ref, watch, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  sessionId: string
  /** English note.
  modelValue?: boolean
}>()

const emit = defineEmits<{
  /** English note.
  (e: 'update:modelValue', value: boolean): void
  /** English note.
  (e: 'generated', sessionCount: number): void
}>()

const { t } = useI18n()

// English engineering note.
const hasIndex = ref(false)
const sessionCount = ref(0)
const isGenerating = ref(false)
const isLoading = ref(true)
// English engineering note.
const forceMode = ref(false)

// English engineering note.
const isOpen = computed({
  get: () => props.modelValue ?? false,
  set: (value) => emit('update:modelValue', value),
})

// English engineering note.
const canClose = computed(() => {
  // English engineering note.
  return !forceMode.value
})

// English engineering note.
async function checkAndAutoOpen() {
  if (!props.sessionId) return

  isLoading.value = true
  try {
    const stats = await window.sessionApi.getStats(props.sessionId)
    hasIndex.value = stats.hasIndex
    sessionCount.value = stats.sessionCount

    // English engineering note.
    if (!hasIndex.value) {
      forceMode.value = true
      isOpen.value = true
    }
  } catch (error) {
    console.error('检查会话索引失败:', error)
  } finally {
    isLoading.value = false
  }
}

// English engineering note.
async function refreshStatus() {
  if (!props.sessionId) return

  isLoading.value = true
  try {
    const stats = await window.sessionApi.getStats(props.sessionId)
    hasIndex.value = stats.hasIndex
    sessionCount.value = stats.sessionCount
  } catch (error) {
    console.error('检查会话索引失败:', error)
  } finally {
    isLoading.value = false
  }
}

// English engineering note.
async function generateSessionIndex() {
  if (!props.sessionId) return

  isGenerating.value = true
  try {
    // English engineering note.
    const savedThreshold = localStorage.getItem('sessionGapThreshold')
    const gapThreshold = savedThreshold ? parseInt(savedThreshold, 10) : 1800 // English engineering note.

    const count = await window.sessionApi.generate(props.sessionId, gapThreshold)
    hasIndex.value = true
    sessionCount.value = count
    emit('generated', count)

    // English engineering note.
    forceMode.value = false
    isOpen.value = false
  } catch (error) {
    console.error('生成会话索引失败:', error)
  } finally {
    isGenerating.value = false
  }
}

// English engineering note.
function close() {
  if (!canClose.value) return
  isOpen.value = false
}

// English engineering note.
function handleOpenChange(value: boolean) {
  if (!value && !canClose.value) {
    // English engineering note.
    return
  }

  isOpen.value = value

  // English engineering note.
  if (value && !forceMode.value) {
    refreshStatus()
  }
}

// English engineering note.
watch(
  () => props.sessionId,
  () => {
    checkAndAutoOpen()
  }
)

// English engineering note.
onMounted(() => {
  checkAndAutoOpen()
})
</script>

<template>
  <UModal :open="isOpen" :dismissible="canClose" @update:open="handleOpenChange">
    <template #content>
      <div class="p-6">
        <!-- English UI note -->
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <div class="flex h-10 w-10 items-center justify-center rounded-full bg-blue-100 dark:bg-blue-900/30">
              <UIcon name="i-heroicons-clock" class="h-5 w-5 text-blue-600 dark:text-blue-400" />
            </div>
            <div>
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
                {{ t('records.sessionIndex.title') }}
              </h3>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                {{ t('records.sessionIndex.subtitle') }}
              </p>
            </div>
          </div>
          <UButton v-if="canClose" icon="i-heroicons-x-mark" color="neutral" variant="ghost" size="sm" @click="close" />
        </div>

        <!-- English UI note -->
        <div v-if="isLoading" class="flex items-center justify-center py-8">
          <UIcon name="i-heroicons-arrow-path" class="h-6 w-6 animate-spin text-gray-400" />
        </div>

        <!-- English UI note -->
        <template v-else>
          <!-- English UI note -->
          <div v-if="!hasIndex" class="space-y-4">
            <div
              class="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-800/50 dark:bg-amber-900/20"
            >
              <div class="flex gap-3">
                <UIcon name="i-heroicons-exclamation-triangle" class="h-5 w-5 shrink-0 text-amber-500" />
                <div>
                  <p class="text-sm font-medium text-amber-800 dark:text-amber-200">
                    {{ t('records.sessionIndex.notGenerated') }}
                  </p>
                  <p class="mt-1 text-sm text-amber-700 dark:text-amber-300">
                    {{ t('records.sessionIndex.notGeneratedHint') }}
                  </p>
                </div>
              </div>
            </div>

            <div class="rounded-lg bg-gray-50 p-4 dark:bg-gray-800/50">
              <h4 class="mb-2 text-sm font-medium text-gray-700 dark:text-gray-300">
                {{ t('records.sessionIndex.whatIsIt') }}
              </h4>
              <ul class="space-y-1 text-sm text-gray-600 dark:text-gray-400">
                <li class="flex items-start gap-2">
                  <UIcon name="i-heroicons-check" class="mt-0.5 h-4 w-4 shrink-0 text-green-500" />
                  {{ t('records.sessionIndex.benefit1') }}
                </li>
                <li class="flex items-start gap-2">
                  <UIcon name="i-heroicons-check" class="mt-0.5 h-4 w-4 shrink-0 text-green-500" />
                  {{ t('records.sessionIndex.benefit2') }}
                </li>
                <li class="flex items-start gap-2">
                  <UIcon name="i-heroicons-check" class="mt-0.5 h-4 w-4 shrink-0 text-green-500" />
                  {{ t('records.sessionIndex.benefit3') }}
                </li>
              </ul>
            </div>
          </div>

          <!-- English UI note -->
          <div v-else class="space-y-4">
            <div
              class="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800/50 dark:bg-green-900/20"
            >
              <div class="flex gap-3">
                <UIcon name="i-heroicons-check-circle" class="h-5 w-5 shrink-0 text-green-500" />
                <div>
                  <p class="text-sm font-medium text-green-800 dark:text-green-200">
                    {{ t('records.sessionIndex.generated') }}
                  </p>
                  <p class="mt-1 text-sm text-green-700 dark:text-green-300">
                    {{ t('records.sessionIndex.sessionCount', { count: sessionCount }) }}
                  </p>
                </div>
              </div>
            </div>

            <p class="text-sm text-gray-500 dark:text-gray-400">
              {{ t('records.sessionIndex.regenerateHint') }}
            </p>
          </div>
        </template>

        <!-- English UI note -->
        <div class="mt-6 flex justify-end gap-2">
          <UButton v-if="canClose" variant="ghost" @click="close">
            {{ t('records.sessionIndex.cancel') }}
          </UButton>
          <UButton color="primary" :loading="isGenerating" @click="generateSessionIndex">
            <UIcon
              v-if="!isGenerating"
              :name="hasIndex ? 'i-heroicons-arrow-path' : 'i-heroicons-sparkles'"
              class="mr-1 h-4 w-4"
            />
            {{
              isGenerating
                ? t('records.sessionIndex.generating')
                : hasIndex
                  ? t('records.sessionIndex.regenerate')
                  : t('records.sessionIndex.generate')
            }}
          </UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
