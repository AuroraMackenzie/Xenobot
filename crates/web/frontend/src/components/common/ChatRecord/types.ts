/**
 * Shared chat-record UI types re-exported for the drawer and filter workflow.
 */

import type { ChatRecordQuery, ChatRecordMessage } from '@/types/format'

// English engineering note.
export type { ChatRecordQuery, ChatRecordMessage }

/**
 * Mutable form state used by the drawer filter panel.
 */
export interface FilterFormData {
  /** Target message id for direct jump mode. */
  messageId: string
  /** Human-readable member label when sender filters are exposed. */
  memberName: string
  /** Keyword or semantic query string. */
  keywords: string
  /** Start date in YYYY-MM-DD form. */
  startDate: string
  /** End date in YYYY-MM-DD form. */
  endDate: string
}

/**
 * Result envelope used when a filter interaction needs both query state and reload intent.
 */
export interface FilterUpdateEvent {
  query: ChatRecordQuery
  shouldReload: boolean
}
