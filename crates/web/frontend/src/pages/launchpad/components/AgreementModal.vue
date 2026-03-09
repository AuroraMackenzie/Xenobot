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
      content: 'md:w-full max-w-3xl',
      overlay: 'backdrop-blur-sm',
    }"
  >
    <template #content>
      <div class="agreement-modal xeno-agreement-shell flex max-h-[85vh] flex-col p-6">
        <!-- Header -->
        <div class="xeno-agreement-header mb-4 flex items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div
              class="xeno-agreement-icon flex h-12 w-12 items-center justify-center rounded-xl"
            >
              <UIcon name="i-heroicons-document-text" class="h-6 w-6 text-cyan-600 dark:text-cyan-400" />
            </div>
            <div class="min-w-0">
              <h2 class="break-words text-xl font-bold text-gray-900 dark:text-white">
                {{ t('common.agreement.title') }}
              </h2>
              <p class="text-sm text-gray-500 dark:text-gray-400">{{ t('common.agreement.subtitle') }}</p>
            </div>
          </div>
          <div class="xeno-agreement-locale w-36 shrink-0">
            <UTabs v-model="currentLocale" size="sm" class="gap-0" :items="languageOptions" />
          </div>
        </div>

        <UAlert
          v-if="isVersionUpdated"
          icon="i-heroicons-exclamation-triangle"
          :title="t('common.agreement.updateNotice')"
          class="xeno-agreement-alert mb-4 pt-2"
        />

        <div class="xeno-agreement-content-wrap mb-6 flex-1 overflow-y-auto pr-4">
          <div class="agreement-content" v-html="renderedContent" />
        </div>

        <div class="xeno-agreement-footer flex items-center justify-end gap-3 pt-4">
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
.agreement-modal {
  -webkit-app-region: no-drag;
}

.xeno-agreement-shell {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.6rem;
  background:
    radial-gradient(circle at top left, rgba(84, 214, 255, 0.14), transparent 28%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 22%),
    rgba(6, 16, 26, 0.94);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 30px 70px rgba(2, 8, 16, 0.38);
  backdrop-filter: blur(22px) saturate(135%);
}

.xeno-agreement-header {
  padding-bottom: 1rem;
  border-bottom: 1px solid rgba(139, 166, 189, 0.16);
}

.xeno-agreement-icon {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    rgba(8, 31, 48, 0.84);
  border: 1px solid rgba(116, 220, 255, 0.22);
  box-shadow: 0 14px 32px rgba(4, 11, 18, 0.24);
}

.xeno-agreement-locale {
  padding: 0.25rem;
  border-radius: 9999px;
  border: 1px solid rgba(139, 166, 189, 0.16);
  background: rgba(8, 18, 28, 0.72);
}

.xeno-agreement-alert {
  border-radius: 1rem;
}

.xeno-agreement-content-wrap {
  border: 1px solid rgba(139, 166, 189, 0.14);
  border-radius: 1.25rem;
  padding: 1.15rem 1.2rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.62);
}

.xeno-agreement-footer {
  border-top: 1px solid rgba(139, 166, 189, 0.16);
}

.agreement-content {
  font-size: 0.875rem;
  line-height: 1.6;
  color: rgba(205, 222, 235, 0.86);
  word-break: break-word;
}

.agreement-content :deep(h2) {
  font-size: 0.95rem;
  font-weight: 600;
  color: rgba(243, 249, 255, 0.96);
  margin-top: 1.25rem;
  margin-bottom: 0.5rem;
  padding-bottom: 0.25rem;
  border-bottom: 1px solid rgba(139, 166, 189, 0.16);
}

.agreement-content :deep(h2:first-child) {
  margin-top: 0;
}

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

.agreement-content :deep(strong) {
  font-weight: 600;
  color: rgba(243, 249, 255, 0.95);
}

.agreement-content :deep(p) {
  margin: 0.5rem 0;
}

.agreement-content :deep(a) {
  color: #74dcff;
  text-decoration: none;
}

.agreement-content :deep(a:hover) {
  text-decoration: underline;
}
</style>
