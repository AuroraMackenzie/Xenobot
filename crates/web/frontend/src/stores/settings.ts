import { defineStore } from 'pinia'
import { ref } from 'vue'
import dayjs from 'dayjs'
import 'dayjs/locale/zh-cn'
import 'dayjs/locale/en'
import { type LocaleType, setLocale as setI18nLocale, getLocale } from '@/i18n'

// English engineering note.
const LOCALE_SET_KEY = 'xenobot_locale_set_by_user'

/**
 * English note.
 * English note.
 */
export const useSettingsStore = defineStore(
  'settings',
  () => {
    // English engineering note.
    const locale = ref<LocaleType>(getLocale())

    /**
     * English note.
     */
    function setLocale(newLocale: LocaleType) {
      locale.value = newLocale

      // English engineering note.
      localStorage.setItem(LOCALE_SET_KEY, 'true')

      // English engineering note.
      setI18nLocale(newLocale)

      // English engineering note.
      dayjs.locale(newLocale === 'zh-CN' ? 'zh-cn' : 'en')

      // English engineering note.
      window.electron?.ipcRenderer.send('locale:change', newLocale)
    }

    /**
     * English note.
     * English note.
     * English note.
     */
    function initLocale() {
      // English engineering note.
      const i18nLocale = getLocale()
      if (locale.value !== i18nLocale) {
        // English engineering note.
        const hasUserSetLocale = localStorage.getItem(LOCALE_SET_KEY)
        if (!hasUserSetLocale) {
          // English engineering note.
          locale.value = i18nLocale
        } else {
          // English engineering note.
          setI18nLocale(locale.value)
        }
      }

      // English engineering note.
      dayjs.locale(locale.value === 'zh-CN' ? 'zh-cn' : 'en')
    }

    return {
      locale,
      setLocale,
      initLocale,
    }
  },
  {
    persist: true, // English engineering note.
  }
)
