/**
 * English note.
 */

import type { ChatRecordQuery, ChatRecordMessage } from '@/types/format'

// English engineering note.
export type { ChatRecordQuery, ChatRecordMessage }

/**
 * English note.
 */
export interface FilterFormData {
  /** English note.
  messageId: string
  /** English note.
  memberName: string
  /** English note.
  keywords: string
  /** English note.
  startDate: string
  /** English note.
  endDate: string
}

/**
 * English note.
 */
export interface FilterUpdateEvent {
  query: ChatRecordQuery
  shouldReload: boolean
}
