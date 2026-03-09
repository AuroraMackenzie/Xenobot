<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'

const emit = defineEmits<{
  openChangelog: []
  openTerms: []
}>()

const { t, locale } = useI18n()

// English engineering note.
const CONFIG_BASE_URL = 'https://xenobot.app'
const configUrl = computed(() => {
  const langPath = locale.value === 'zh-CN' ? 'cn' : 'en'
  return `${CONFIG_BASE_URL}/${langPath}/config.json`
})

// English engineering note.
const storageKey = computed(() => `xenobot_app_config_${locale.value}`)

// English engineering note.
interface FooterLink {
  id: string
  icon: string
  title: string
  url?: string
  action?: 'changelog'
}

// English engineering note.
function getDefaultLinks(): FooterLink[] {
  const isChinese = locale.value === 'zh-CN'
  return [
    {
      id: 'website',
      icon: 'i-heroicons-globe-alt',
      title: isChinese ? '官网' : 'Website',
      url: isChinese ? 'https://xenobot.app/cn/' : 'https://xenobot.app/en/',
    },
    {
      id: 'github',
      icon: 'i-simple-icons-github',
      title: 'Github',
      url: 'https://github.com/xenobot-labs/Xenobot',
    },
    {
      id: 'terms',
      icon: 'i-heroicons-document-text',
      title: isChinese ? '使用条款' : 'Terms of Use',
      action: 'terms',
    },
    {
      id: 'changelog',
      icon: 'i-heroicons-document-text',
      title: t('home.changelog.title'),
      action: 'changelog',
    },
  ]
}

const footerLinks = ref<FooterLink[]>(getDefaultLinks())

// English engineering note.
interface SocialConfig {
  xiaohongshu?: { show: boolean; url: string }
  x?: { show: boolean; url: string }
}

const socialConfig = ref<SocialConfig>({})

// English engineering note.
const socialLink = computed(() => {
  const isChinese = locale.value === 'zh-CN'

  if (isChinese && socialConfig.value.xiaohongshu?.show) {
    return {
      title: '小红书',
      url: socialConfig.value.xiaohongshu.url,
    }
  }

  if (!isChinese && socialConfig.value.x?.show) {
    return {
      title: 'X',
      url: socialConfig.value.x.url,
    }
  }

  return null
})

/**
 * English note.
 */
function getCachedConfigObject(): Record<string, unknown> | null {
  const raw = localStorage.getItem(storageKey.value) || localStorage.getItem('xenobot_app_config')
  if (!raw) return null
  return JSON.parse(raw) as Record<string, unknown>
}

function loadCachedExtraLinks(): FooterLink[] | null {
  try {
    const config = getCachedConfigObject()
    if (!config) return null
    // English engineering note.
    return (config.homeFooterExtraLinks as FooterLink[] | undefined) || null
  } catch (error) {
    // English engineering note.
    console.warn('[HomeFooter] Failed to read cached extra links. Falling back to defaults.', error)
  }
  return null
}

/**
 * English note.
 */
function loadCachedSocialConfig(): SocialConfig | null {
  try {
    const config = getCachedConfigObject()
    if (!config) return null
    return (config.social as SocialConfig | undefined) || null
  } catch (error) {
    // English engineering note.
    console.warn('[HomeFooter] Failed to read cached social config. Falling back to defaults.', error)
  }
  return null
}

/**
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 */
async function fetchConfig(): Promise<void> {
  // English engineering note.
  const cachedExtra = loadCachedExtraLinks()
  if (cachedExtra && cachedExtra.length > 0) {
    footerLinks.value = [...getDefaultLinks(), ...cachedExtra]
  }

  // English engineering note.
  const cachedSocial = loadCachedSocialConfig()
  if (cachedSocial) {
    socialConfig.value = cachedSocial
  }

  try {
    const result = await window.api.app.fetchRemoteConfig(configUrl.value)
    if (!result.success || !result.data) return

    const config = result.data as Record<string, unknown>
    // English engineering note.
    localStorage.setItem(storageKey.value, JSON.stringify(config))
    // English engineering note.
    localStorage.setItem('xenobot_app_config', JSON.stringify(config))

    // English engineering note.
    if (config.homeFooterExtraLinks && Array.isArray(config.homeFooterExtraLinks)) {
      footerLinks.value = [...getDefaultLinks(), ...(config.homeFooterExtraLinks as FooterLink[])]
    }

    // English engineering note.
    if (config.social) {
      socialConfig.value = config.social as SocialConfig
    }
  } catch (error) {
    // English engineering note.
    console.warn('[HomeFooter] Failed to fetch remote config. Keeping the local fallback.', error)
  }
}

// English engineering note.
function handleLinkClick(link: FooterLink) {
  if (link.action === 'changelog') {
    emit('openChangelog')
  } else if (link.action === 'terms') {
    emit('openTerms')
  } else if (link.url) {
    window.open(link.url, '_blank')
  }
}

// English engineering note.
function openSocialLink() {
  if (socialLink.value?.url) {
    window.open(socialLink.value.url, '_blank')
  }
}

// English engineering note.
onMounted(() => {
  fetchConfig()
})

// English engineering note.
watch(locale, () => {
  // English engineering note.
  footerLinks.value = getDefaultLinks()
  // English engineering note.
  fetchConfig()
})
</script>

<template>
  <div class="xeno-home-footer-wrap absolute bottom-4 left-0 right-0">
    <div class="xeno-home-footer flex items-center justify-center">
      <template v-for="(link, index) in footerLinks" :key="link.id">
        <!-- English UI note -->
        <span v-if="index > 0" class="mx-2 text-gray-300 dark:text-gray-600">·</span>
        <!-- English UI note -->
        <button
          class="xeno-home-footer-link text-sm text-gray-500 transition-colors hover:text-primary dark:text-gray-400 dark:hover:text-primary"
          @click="handleLinkClick(link)"
        >
          {{ link.title }}
        </button>
      </template>

      <!-- English UI note -->
      <template v-if="socialLink">
        <span class="mx-2 text-gray-300 dark:text-gray-600">·</span>
        <button
          class="xeno-home-footer-link text-sm text-gray-500 transition-colors hover:text-primary dark:text-gray-400 dark:hover:text-primary"
          @click="openSocialLink"
        >
          {{ socialLink.title }}
        </button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.xeno-home-footer-wrap {
  pointer-events: none;
}

.xeno-home-footer {
  width: fit-content;
  margin: 0 auto;
  pointer-events: auto;
  border: 1px solid var(--xeno-border-soft);
  border-radius: 9999px;
  padding: 0.7rem 1rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 120%),
    rgba(7, 20, 31, 0.78);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 36px rgba(1, 7, 15, 0.2);
  backdrop-filter: blur(14px) saturate(124%);
}

.xeno-home-footer-link:hover {
  color: #74dcff;
}
</style>
