import { createI18n } from "vue-i18n";
import zhCN from "./locales/zh-CN";
import enUS from "./locales/en-US";
import { detectSystemLocale, type LocaleType } from "./types";

// English engineering note.
export type { LocaleType } from "./types";
export {
  availableLocales,
  defaultLocale,
  detectSystemLocale,
  isFeatureSupported,
  featureLocaleRestrictions,
} from "./types";

// English engineering note.
const LOCALE_SET_KEY = "xenobot_locale_set_by_user";
const PINIA_SETTINGS_KEY = "settings"; // English engineering note.

/**
 * English note.
 * English note.
 * English note.
 */
function getInitialLocale(): LocaleType {
  const hasUserSetLocale = localStorage.getItem(LOCALE_SET_KEY);

  if (hasUserSetLocale) {
    // English engineering note.
    try {
      const piniaSettings = localStorage.getItem(PINIA_SETTINGS_KEY);
      if (piniaSettings) {
        const parsed = JSON.parse(piniaSettings);
        if (parsed.locale === "zh-CN" || parsed.locale === "en-US") {
          return parsed.locale;
        }
      }
    } catch {
      // English engineering note.
    }
  }

  // English engineering note.
  return detectSystemLocale();
}

/**
 * English note.
 */
export const i18n = createI18n({
  legacy: false, // English engineering note.
  locale: getInitialLocale(), // English engineering note.
  fallbackLocale: "en-US", // English engineering note.
  messages: {
    "zh-CN": zhCN,
    "en-US": enUS,
  },
});

/**
 * English note.
 */
export function setLocale(locale: LocaleType) {
  i18n.global.locale.value = locale;
}

/**
 * English note.
 */
export function getLocale(): LocaleType {
  return i18n.global.locale.value as LocaleType;
}

export default i18n;
