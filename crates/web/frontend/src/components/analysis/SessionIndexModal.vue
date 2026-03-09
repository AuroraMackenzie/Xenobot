<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  sessionId: string
  modelValue?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'generated', sessionCount: number): void
}>()

const { t } = useI18n()

const hasIndex = ref(false)
const sessionCount = ref(0)
const isGenerating = ref(false)
const isLoading = ref(true)
const forceMode = ref(false)

const isOpen = computed({
  get: () => props.modelValue ?? false,
  set: (value) => emit('update:modelValue', value),
})

const canClose = computed(() => {
  return !forceMode.value
})

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
    console.error('[SessionIndexModal] Failed to inspect session index:', error)
  } finally {
    isLoading.value = false
  }
}

async function refreshStatus() {
  if (!props.sessionId) return

  isLoading.value = true
  try {
    const stats = await window.sessionApi.getStats(props.sessionId)
    hasIndex.value = stats.hasIndex
    sessionCount.value = stats.sessionCount
  } catch (error) {
    console.error('[SessionIndexModal] Failed to refresh session index status:', error)
  } finally {
    isLoading.value = false
  }
}

async function generateSessionIndex() {
  if (!props.sessionId) return

  isGenerating.value = true
  try {
    const savedThreshold = localStorage.getItem('sessionGapThreshold')
    const gapThreshold = savedThreshold ? parseInt(savedThreshold, 10) : 1800

    const count = await window.sessionApi.generate(props.sessionId, gapThreshold)
    hasIndex.value = true
    sessionCount.value = count
    emit('generated', count)

    forceMode.value = false
    isOpen.value = false
  } catch (error) {
    console.error('[SessionIndexModal] Failed to generate session index:', error)
  } finally {
    isGenerating.value = false
  }
}

function close() {
  if (!canClose.value) return
  isOpen.value = false
}

function handleOpenChange(value: boolean) {
  if (!value && !canClose.value) {
    return
  }

  isOpen.value = value

  if (value && !forceMode.value) {
    refreshStatus()
  }
}

watch(
  () => props.sessionId,
  () => {
    checkAndAutoOpen()
  }
)

onMounted(() => {
  checkAndAutoOpen()
})
</script>

<template>
  <UModal :open="isOpen" :dismissible="canClose" @update:open="handleOpenChange">
    <template #content>
      <div class="xeno-session-index-card p-6">
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <div class="xeno-session-index-icon flex h-10 w-10 items-center justify-center rounded-full">
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

        <div v-if="isLoading" class="flex items-center justify-center py-8">
          <UIcon name="i-heroicons-arrow-path" class="h-6 w-6 animate-spin text-gray-400" />
        </div>

        <template v-else>
          <div v-if="!hasIndex" class="space-y-4">
            <div
              class="xeno-session-index-alert rounded-2xl p-4"
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

            <div class="xeno-session-index-panel rounded-2xl p-4">
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

          <div v-else class="space-y-4">
            <div
              class="xeno-session-index-success rounded-2xl p-4"
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

<style scoped>
.xeno-session-index-card {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 1.5rem;
  background:
    radial-gradient(circle at top right, rgba(59, 130, 246, 0.12), transparent 30%),
    linear-gradient(180deg, rgba(15, 23, 42, 0.78), rgba(15, 23, 42, 0.64));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 20px 44px rgba(2, 6, 23, 0.22);
  backdrop-filter: blur(18px);
}

.xeno-session-index-icon {
  background: rgba(59, 130, 246, 0.12);
}

.xeno-session-index-panel,
.xeno-session-index-alert,
.xeno-session-index-success {
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
}

.xeno-session-index-panel {
  background: rgba(15, 23, 42, 0.54);
}

.xeno-session-index-alert {
  background: rgba(251, 191, 36, 0.1);
}

.xeno-session-index-success {
  background: rgba(34, 197, 94, 0.1);
}
</style>
