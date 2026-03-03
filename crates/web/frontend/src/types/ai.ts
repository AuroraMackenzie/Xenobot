/**
 * English note.
 * English note.
 */

// English engineering note.

/**
 * English note.
 * English note.
 * English note.
 * English note.
 */
export type PresetApplicableType = 'group' | 'private' | 'common'

/**
 * English note.
 *
 * English note.
 * English note.
 * English note.
 * English note.
 *
 * English note.
 */
export interface PromptPreset {
  id: string
  name: string // English engineering note.
  roleDefinition: string // English engineering note.
  responseRules: string // English engineering note.
  isBuiltIn: boolean // English engineering note.
  applicableTo?: PresetApplicableType // English engineering note.
  createdAt: number
  updatedAt: number
}

/**
 * English note.
 */
export interface AIPromptSettings {
  activePresetId: string // English engineering note.
}

// English engineering note.

/**
 * English note.
 */
export type PromptPresetChatType = 'group' | 'private'
