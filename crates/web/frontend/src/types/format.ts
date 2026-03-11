/**
 * Xenobot format contracts.
 * Covers native export format, merge workflow types, and chat viewer models.
 */

import type { ChatPlatform, ChatType, MessageType, MemberRole } from "./base";

// ==================== Xenobot native format ====================

/**
 * Header metadata for a Xenobot export.
 */
export interface XenobotHeader {
  version: string; // Format version, for example "0.0.1"
  exportedAt: number; // Export timestamp (unix seconds)
  generator?: string; // Generator tool name
  description?: string; // Optional user-facing description
}

/**
 * Source metadata when multiple files are merged.
 */
export interface MergeSource {
  filename: string; // Original file name
  platform?: string; // Source platform label
  messageCount: number; // Number of messages from this source
}

/**
 * Core chat metadata for Xenobot format.
 */
export interface XenobotMeta {
  name: string; // Group or conversation title
  platform: ChatPlatform; // Platform id (`mixed` when merged)
  type: ChatType; // Chat topology
  sources?: MergeSource[]; // Optional merged-source details
  groupId?: string; // Group identifier (group chats)
  groupAvatar?: string; // Group avatar (base64 Data URL)
  ownerId?: string; // Export owner platform id
}

/**
 * Member entry for Xenobot export.
 */
export interface XenobotMember {
  platformId: string; // Stable platform identity
  accountName: string; // Account display name
  groupNickname?: string; // Group nickname snapshot
  aliases?: string[]; // User-defined aliases
  avatar?: string; // Avatar as base64 Data URL
  roles?: MemberRole[]; // Role labels (can contain multiple roles)
}

/**
 * Message entry for Xenobot export.
 */
export interface XenobotMessage {
  sender: string; // Sender platform id
  accountName: string; // Account name at send time
  groupNickname?: string; // Group nickname at send time
  timestamp: number; // Unix timestamp in seconds
  type: MessageType; // Message kind
  content: string | null; // Message payload text
}

/**
 * Full Xenobot export payload.
 */
export interface XenobotFormat {
  xenobot: XenobotHeader;
  meta: XenobotMeta;
  members: XenobotMember[];
  messages: XenobotMessage[];
}

// ==================== Merge workflow models ====================

/**
 * Parsed file summary used by merge preview UI.
 */
export interface FileParseInfo {
  name: string; // Conversation title
  format: string; // Parser format id
  platform: string; // Platform id
  messageCount: number; // Message count
  memberCount: number; // Member count
  fileSize?: number; // File size in bytes
}

/**
 * Collision candidate detected during merge.
 */
export interface MergeConflict {
  id: string; // Conflict id
  timestamp: number; // Candidate timestamp
  sender: string; // Candidate sender
  contentLength1: number; // Content length from source A
  contentLength2: number; // Content length from source B
  content1: string; // Content from source A
  content2: string; // Content from source B
}

/**
 * Merge conflict scan result.
 */
export interface ConflictCheckResult {
  conflicts: MergeConflict[];
  totalMessages: number; // Estimated total after merge
}

/**
 * Conflict resolution instruction.
 */
export interface ConflictResolution {
  id: string;
  resolution: "keep1" | "keep2" | "keepBoth";
}

/**
 * Merge output format.
 */
export type OutputFormat = "json" | "jsonl";

/**
 * Merge command payload.
 */
export interface MergeParams {
  filePaths: string[];
  outputName: string;
  outputDir?: string;
  outputFormat?: OutputFormat; // Output format (default: json)
  conflictResolutions: ConflictResolution[];
  andAnalyze: boolean;
}

/**
 * Merge execution result.
 */
export interface MergeResult {
  success: boolean;
  outputPath?: string;
  sessionId?: string; // Analysis session id when `andAnalyze` is enabled
  error?: string;
}

// ==================== Chat viewer models ====================

/**
 * Query shape used by the chat record viewer.
 * Multiple conditions can be combined in one request.
 */
export interface ChatRecordQuery {
  /** Focus the initial viewport around this message id. */
  scrollToMessageId?: number;

  /** Member filter: only include messages from the selected member. */
  memberId?: number;
  /** Readable member label used by the UI. */
  memberName?: string;

  /** Inclusive time-window start (unix seconds). */
  startTs?: number;
  /** Inclusive time-window end (unix seconds). */
  endTs?: number;

  /** Keyword clauses, matched with OR semantics. */
  keywords?: string[];

  /** UI-only highlight tokens for matched content. */
  highlightKeywords?: string[];

  /** Search strategy for keyword-driven requests. */
  searchMode?: "keyword" | "semantic";

  /** Semantic similarity threshold passed to the API when enabled. */
  semanticThreshold?: number;
}

/**
 * Message record rendered in the chat viewer.
 */
export interface ChatRecordMessage {
  id: number;
  senderName: string;
  senderPlatformId: string;
  senderAliases: string[];
  senderAvatar: string | null; // Sender avatar
  content: string;
  timestamp: number;
  type: number;
  replyToMessageId: string | null; // Original platform id of replied message
  replyToContent: string | null; // Preview of replied message content
  replyToSenderName: string | null; // Sender name of replied message
}
