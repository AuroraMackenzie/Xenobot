/**
 * English note.
 */
export type LocaleType = 'zh-CN' | 'en-US'

/**
 * English note.
 */
export interface LocaleOption {
  code: LocaleType
  name: string
  nativeName: string
}

/**
 * English note.
 */
export const availableLocales: LocaleOption[] = [
  { code: 'zh-CN', name: 'Chinese (Simplified)', nativeName: '简体中文' },
  { code: 'en-US', name: 'English (US)', nativeName: 'English' },
]

/**
 * English note.
 */
export const defaultLocale: LocaleType = 'zh-CN'

/**
 * English note.
 */
export function detectSystemLocale(): LocaleType {
  const systemLocale = navigator.language
  if (systemLocale.startsWith('zh')) {
    return 'zh-CN'
  }
  return 'en-US'
}

/**
 * English note.
 * English note.
 */
export interface FeatureLocaleSupport {
  /** English note.
  feature: string
  /** English note.
  supportedLocales: LocaleType[]
}

/**
 * English note.
 * English note.
 */
export const featureLocaleRestrictions: Record<string, LocaleType[]> = {
  // English engineering note.
  groupRanking: ['zh-CN'],
  // English engineering note.
}

/**
 * English note.
 */
export function isFeatureSupported(feature: string, currentLocale: LocaleType): boolean {
  const supportedLocales = featureLocaleRestrictions[feature]
  // English engineering note.
  if (!supportedLocales || supportedLocales.length === 0) {
    return true
  }
  return supportedLocales.includes(currentLocale)
}
