<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import MarkdownIt from 'markdown-it'
import { useSettingsStore } from '@/stores/settings'
import { availableLocales, type LocaleType } from '@/i18n'
// English engineering note.
import agreementZh from '@/assets/docs/agreement_zh.md?raw'
import agreementEn from '@/assets/docs/agreement_en.md?raw'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const { locale } = storeToRefs(settingsStore)

// English engineering note.
const languageOptions = computed(() =>
  availableLocales.map((l) => ({
    label: l.nativeName,
    value: l.code,
  }))
)

// English engineering note.
const currentLocale = computed({
  get: () => locale.value,
  set: (val: LocaleType) => settingsStore.setLocale(val),
})

// English engineering note.
const AGREEMENT_VERSION = '1.1'
const AGREEMENT_KEY = 'xenobot_agreement_version'

// English engineering note.
const isOpen = ref(false)
// English engineering note.
const isVersionUpdated = ref(false)

// English engineering note.
onMounted(() => {
  const acceptedVersion = localStorage.getItem(AGREEMENT_KEY)
  // English engineering note.
  if (acceptedVersion !== AGREEMENT_VERSION) {
    isOpen.value = true
    // English engineering note.
    if (acceptedVersion) {
      isVersionUpdated.value = true
    }
  }
})

// English engineering note.
const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
})

// English engineering note.
md.renderer.rules.link_open = (tokens, idx, options, _env, self) => {
  tokens[idx].attrSet('target', '_blank')
  tokens[idx].attrSet('rel', 'noopener noreferrer')
  return self.renderToken(tokens, idx, options)
}

// English engineering note.
const agreementText = computed(() => {
  return locale.value === 'zh-CN' ? agreementZh : agreementEn
})

// English engineering note.
const renderedContent = computed(() => md.render(agreementText.value))

// English engineering note.
function handleAgree() {
  localStorage.setItem(AGREEMENT_KEY, AGREEMENT_VERSION)
  // English engineering note.
  localStorage.setItem('xenobot_locale_set_by_user', 'true')
  isOpen.value = false
}

// English engineering note.
function handleDisagree() {
  // English engineering note.
  localStorage.removeItem(AGREEMENT_KEY)
  window.api.send('window-close')
}

// English engineering note.
function open() {
  isOpen.value = true
}

defineExpose({ open })
</script>

<template>
  <UModal
    :open="isOpen"
    prevent-close
    :ui="{
      content: 'md:w-full max-w-2xl',
      overlay: 'backdrop-blur-sm',
    }"
  >
    <template #content>
      <!-- English UI note -->
      <div class="agreement-modal flex max-h-[85vh] flex-col p-6">
        <!-- Header -->
        <div class="mb-4 flex items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div
              class="flex h-12 w-12 items-center justify-center rounded-xl bg-linear-to-br from-cyan-100 to-sky-100 dark:from-cyan-900/30 dark:to-sky-900/30"
            >
              <UIcon name="i-heroicons-document-text" class="h-6 w-6 text-cyan-600 dark:text-cyan-400" />
            </div>
            <div>
              <h2 class="text-xl font-bold text-gray-900 dark:text-white">{{ t('common.agreement.title') }}</h2>
              <p class="text-sm text-gray-500 dark:text-gray-400">{{ t('common.agreement.subtitle') }}</p>
            </div>
          </div>
          <!-- English UI note -->
          <div class="w-36 shrink-0">
            <UTabs v-model="currentLocale" size="sm" class="gap-0" :items="languageOptions" />
          </div>
        </div>

        <!-- English UI note -->
        <UAlert
          v-if="isVersionUpdated"
          icon="i-heroicons-exclamation-triangle"
          :title="t('common.agreement.updateNotice')"
          class="mb-4 pt-2"
        />

        <!-- English UI note -->
        <div class="mb-6 flex-1 overflow-y-auto pr-4">
          <div class="agreement-content" v-html="renderedContent" />
        </div>

        <!-- English UI note -->
        <div class="flex items-center justify-end gap-3 border-t border-gray-200 pt-4 dark:border-gray-700">
          <UButton variant="ghost" color="neutral" size="lg" @click="handleDisagree">
            {{ t('common.agreement.disagree') }}
          </UButton>
          <UButton
            color="primary"
            size="lg"
            class="bg-cyan-500 hover:bg-cyan-600 dark:bg-cyan-600 dark:hover:bg-cyan-700"
            @click="handleAgree"
          >
            {{ t('common.agreement.agree') }}
          </UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>

<style scoped>
/* English note.
.agreement-modal {
  -webkit-app-region: no-drag;
}

/* English note.
.agreement-content {
  font-size: 0.875rem;
  line-height: 1.6;
  color: var(--color-gray-600);
}

/* English note.
:root.dark .agreement-content {
  color: var(--color-gray-300);
}

/* English note.
.agreement-content :deep(h2) {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-gray-900);
  margin-top: 1.25rem;
  margin-bottom: 0.5rem;
  padding-bottom: 0.25rem;
  border-bottom: 1px solid var(--color-gray-200);
}

:root.dark .agreement-content :deep(h2) {
  color: var(--color-gray-100);
  border-bottom-color: var(--color-gray-700);
}

/* English note.
.agreement-content :deep(h2:first-child) {
  margin-top: 0;
}

/* English note.
.agreement-content :deep(ul) {
  margin: 0.5rem 0;
  padding-left: 1.25rem;
  list-style: none;
}

.agreement-content :deep(li) {
  position: relative;
  margin-bottom: 0.375rem;
  padding-left: 0.5rem;
}

.agreement-content :deep(li::before) {
  content: '•';
  position: absolute;
  left: -0.75rem;
  color: var(--color-cyan-500);
  font-weight: bold;
}

/* English note.
.agreement-content :deep(strong) {
  font-weight: 600;
  color: var(--color-gray-800);
}

:root.dark .agreement-content :deep(strong) {
  color: var(--color-gray-200);
}

/* English note.
.agreement-content :deep(p) {
  margin: 0.5rem 0;
}

/* English note.
.agreement-content :deep(a) {
  color: var(--color-cyan-500);
  text-decoration: none;
}

.agreement-content :deep(a:hover) {
  text-decoration: underline;
}
</style>
