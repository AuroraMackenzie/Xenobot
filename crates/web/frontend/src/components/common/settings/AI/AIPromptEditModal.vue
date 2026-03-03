<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PromptPreset, PresetApplicableType } from '@/types/ai'
import {
  getDefaultRoleDefinition,
  getDefaultResponseRules,
  getLockedPromptSectionPreview,
  getOriginalBuiltinPreset,
  type LocaleType,
} from '@/config/prompts'
import { usePromptStore } from '@/stores/prompt'

const { t, locale } = useI18n()

// Props
const props = defineProps<{
  open: boolean
  mode: 'add' | 'edit'
  preset: PromptPreset | null
}>()

// Emits
const emit = defineEmits<{
  'update:open': [value: boolean]
  saved: []
}>()

// Store
const promptStore = usePromptStore()

// English engineering note.
const formData = ref({
  name: '',
  roleDefinition: '',
  responseRules: '',
  supportGroup: true,
  supportPrivate: true,
})

// English engineering note.
const isBuiltIn = computed(() => props.preset?.isBuiltIn ?? false)
const isEditMode = computed(() => props.mode === 'edit')
const isModified = computed(() => {
  if (!isBuiltIn.value || !props.preset) return false
  return promptStore.isBuiltinPresetModified(props.preset.id)
})

const modalTitle = computed(() => {
  if (isBuiltIn.value) return t('settings.aiPrompt.modal.editBuiltin')
  return isEditMode.value ? t('settings.aiPrompt.modal.editCustom') : t('settings.aiPrompt.modal.addCustom')
})

const canSave = computed(() => {
  return formData.value.name.trim() && formData.value.roleDefinition.trim() && formData.value.responseRules.trim()
})

/**
 * English note.
 */
function applicableToCheckboxes(applicableTo?: PresetApplicableType): { group: boolean; private: boolean } {
  if (!applicableTo || applicableTo === 'common') {
    return { group: true, private: true }
  }
  return {
    group: applicableTo === 'group',
    private: applicableTo === 'private',
  }
}

/**
 * English note.
 */
function checkboxesToApplicableTo(group: boolean, private_: boolean): PresetApplicableType {
  if (group && private_) return 'common'
  if (group) return 'group'
  if (private_) return 'private'
  return 'common' // English engineering note.
}

// English engineering note.
watch(
  () => props.open,
  (newVal) => {
    if (newVal) {
      if (props.preset) {
        // English engineering note.
        const checkboxes = applicableToCheckboxes(props.preset.applicableTo)
        formData.value = {
          name: props.preset.name,
          roleDefinition: props.preset.roleDefinition,
          responseRules: props.preset.responseRules,
          supportGroup: checkboxes.group,
          supportPrivate: checkboxes.private,
        }
      } else {
        // English engineering note.
        formData.value = {
          name: '',
          roleDefinition: getDefaultRoleDefinition(locale.value as LocaleType),
          responseRules: getDefaultResponseRules(locale.value as LocaleType),
          supportGroup: true,
          supportPrivate: true,
        }
      }
    }
  }
)

/** English note.
function closeModal() {
  emit('update:open', false)
}

/** English note.
function handleSave() {
  if (!canSave.value) return

  const applicableTo = checkboxesToApplicableTo(formData.value.supportGroup, formData.value.supportPrivate)

  if (isEditMode.value && props.preset) {
    // English engineering note.
    const updates: {
      name: string
      roleDefinition: string
      responseRules: string
      applicableTo?: PresetApplicableType
    } = {
      name: formData.value.name.trim(),
      roleDefinition: formData.value.roleDefinition.trim(),
      responseRules: formData.value.responseRules.trim(),
    }
    // English engineering note.
    if (!isBuiltIn.value) {
      updates.applicableTo = applicableTo
    }
    promptStore.updatePromptPreset(props.preset.id, updates)
  } else {
    // English engineering note.
    promptStore.addPromptPreset({
      name: formData.value.name.trim(),
      roleDefinition: formData.value.roleDefinition.trim(),
      responseRules: formData.value.responseRules.trim(),
      applicableTo,
    })
  }

  emit('saved')
  closeModal()
}

/** English note.
function handleReset() {
  if (!props.preset || !isBuiltIn.value) return

  const original = getOriginalBuiltinPreset(props.preset.id, locale.value as LocaleType)
  if (original) {
    // English engineering note.
    formData.value = {
      name: original.name,
      roleDefinition: original.roleDefinition,
      responseRules: original.responseRules,
      supportGroup: true,
      supportPrivate: true,
    }
    // English engineering note.
    promptStore.resetBuiltinPreset(props.preset.id)
  }
}

// English engineering note.
const previewContent = computed(() => {
  // English engineering note.
  const lockedSection = getLockedPromptSectionPreview('group', undefined, locale.value as LocaleType)

  // English engineering note.
  const responseRulesLabel = locale.value === 'zh-CN' ? '回答要求：' : 'Response requirements:'
  return `${formData.value.roleDefinition}

${lockedSection}

${responseRulesLabel}
${formData.value.responseRules}`
})
</script>

<template>
  <UModal :open="open" :ui="{ content: 'md:w-full max-w-2xl' }" @update:open="emit('update:open', $event)">
    <template #content>
      <div class="p-6">
        <!-- Header -->
        <div class="mb-4 flex items-center justify-between">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">{{ modalTitle }}</h2>
          <UButton icon="i-heroicons-x-mark" variant="ghost" size="sm" @click="closeModal" />
        </div>

        <!-- English UI note -->
        <div class="max-h-[500px] space-y-4 overflow-y-auto pr-1">
          <!-- English UI note -->
          <div>
            <label class="mb-1.5 block text-sm font-medium text-gray-700 dark:text-gray-300">
              {{ t('settings.aiPrompt.modal.presetName') }}
            </label>
            <UInput
              v-model="formData.name"
              :placeholder="t('settings.aiPrompt.modal.presetNamePlaceholder')"
              class="w-60"
            />
          </div>

          <!-- English UI note -->
          <div v-if="!isBuiltIn">
            <label class="mb-1.5 block text-sm font-medium text-gray-700 dark:text-gray-300">
              {{ t('settings.aiPrompt.modal.applicableTo') }}
              <span class="font-normal text-gray-500">{{ t('settings.aiPrompt.modal.applicableToHint') }}</span>
            </label>
            <div class="flex items-center gap-4">
              <label class="flex cursor-pointer items-center gap-2">
                <input
                  v-model="formData.supportGroup"
                  type="checkbox"
                  class="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span class="text-sm text-gray-700 dark:text-gray-300">
                  {{ t('settings.aiPrompt.modal.groupChat') }}
                </span>
              </label>
              <label class="flex cursor-pointer items-center gap-2">
                <input
                  v-model="formData.supportPrivate"
                  type="checkbox"
                  class="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span class="text-sm text-gray-700 dark:text-gray-300">
                  {{ t('settings.aiPrompt.modal.privateChat') }}
                </span>
              </label>
            </div>
          </div>

          <!-- English UI note -->
          <div>
            <label class="mb-1.5 block text-sm font-medium text-gray-700 dark:text-gray-300">
              {{ t('settings.aiPrompt.modal.roleDefinition') }}
            </label>
            <UTextarea
              v-model="formData.roleDefinition"
              :rows="8"
              :placeholder="t('settings.aiPrompt.modal.roleDefinitionPlaceholder')"
              class="w-120 font-mono text-sm"
            />
          </div>

          <!-- English UI note -->
          <div>
            <label class="mb-1.5 block text-sm font-medium text-gray-700 dark:text-gray-300">
              {{ t('settings.aiPrompt.modal.responseRules') }}
              <span class="font-normal text-gray-500">{{ t('settings.aiPrompt.modal.responseRulesHint') }}</span>
            </label>
            <UTextarea
              v-model="formData.responseRules"
              :rows="5"
              :placeholder="t('settings.aiPrompt.modal.responseRulesPlaceholder')"
              class="w-120 font-mono text-sm"
            />
          </div>

          <!-- English UI note -->
          <div>
            <label class="mb-1.5 flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-300">
              <UIcon name="i-heroicons-eye" class="h-4 w-4 text-violet-500" />
              {{ t('settings.aiPrompt.modal.preview') }}
              <span class="font-normal text-gray-500">{{ t('settings.aiPrompt.modal.previewHint') }}</span>
            </label>
            <div class="rounded-lg border border-gray-200 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800/50">
              <pre class="whitespace-pre-wrap text-sm text-gray-700 dark:text-gray-300">{{ previewContent }}</pre>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div class="mt-6 flex justify-end gap-2">
          <!-- English UI note -->
          <UButton v-if="isBuiltIn && isModified" variant="outline" color="warning" @click="handleReset">
            <UIcon name="i-heroicons-arrow-path" class="mr-1 h-4 w-4" />
            {{ t('settings.aiPrompt.modal.resetToDefault') }}
          </UButton>
          <UButton variant="ghost" @click="closeModal">{{ t('common.cancel') }}</UButton>
          <UButton color="primary" :disabled="!canSave" @click="handleSave">
            {{ isEditMode ? t('settings.aiPrompt.modal.saveChanges') : t('settings.aiPrompt.modal.addPreset') }}
          </UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
