/**
 * English note.
 * English note.
 */

// English engineering note.

/**
 * English note.
 *
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 */
export enum MessageType {
  // English engineering note.
  TEXT = 0, // English engineering note.
  IMAGE = 1, // English engineering note.
  VOICE = 2, // English engineering note.
  VIDEO = 3, // English engineering note.
  FILE = 4, // English engineering note.
  EMOJI = 5, // English engineering note.
  LINK = 7, // English engineering note.
  LOCATION = 8, // English engineering note.

  // English engineering note.
  RED_PACKET = 20, // English engineering note.
  TRANSFER = 21, // English engineering note.
  POKE = 22, // English engineering note.
  CALL = 23, // English engineering note.
  SHARE = 24, // English engineering note.
  REPLY = 25, // English engineering note.
  FORWARD = 26, // English engineering note.
  CONTACT = 27, // English engineering note.

  // English engineering note.
  SYSTEM = 80, // English engineering note.
  RECALL = 81, // English engineering note.

  // English engineering note.
  OTHER = 99, // English engineering note.
}

/**
 * English note.
 */
const MESSAGE_TYPE_KEYS: Record<number, string> = {
  // English engineering note.
  [MessageType.TEXT]: "text",
  [MessageType.IMAGE]: "image",
  [MessageType.VOICE]: "voice",
  [MessageType.VIDEO]: "video",
  [MessageType.FILE]: "file",
  [MessageType.EMOJI]: "emoji",
  [MessageType.LINK]: "link",
  [MessageType.LOCATION]: "location",
  // English engineering note.
  [MessageType.RED_PACKET]: "redPacket",
  [MessageType.TRANSFER]: "transfer",
  [MessageType.POKE]: "poke",
  [MessageType.CALL]: "call",
  [MessageType.SHARE]: "share",
  [MessageType.REPLY]: "reply",
  [MessageType.FORWARD]: "forward",
  [MessageType.CONTACT]: "contact",
  // English engineering note.
  [MessageType.SYSTEM]: "system",
  [MessageType.RECALL]: "recall",
  // English engineering note.
  [MessageType.OTHER]: "other",
};

/**
 * English note.
 * English note.
 * English note.
 */
export function getMessageTypeName(
  type: MessageType | number,
  t?: (key: string) => string,
): string {
  const key = MESSAGE_TYPE_KEYS[type];
  if (t && key) return t(`common.messageType.${key}`);
  return t ? t("common.messageType.unknown") : "未知";
}

/**
 * English note.
 * English note.
 * English note.
 */
export type ChatPlatform = string;

/**
 * English note.
 */
export const KNOWN_PLATFORMS = {
  QQ: "qq",
  WECHAT: "weixin",
  DISCORD: "discord",
  WHATSAPP: "whatsapp",
  TELEGRAM: "telegram",
  INSTAGRAM: "instagram",
  LINE: "line",
  UNKNOWN: "unknown",
} as const;

/**
 * English note.
 */
export enum ChatType {
  GROUP = "group", // English engineering note.
  PRIVATE = "private", // English engineering note.
}

// English engineering note.

/**
 * English note.
 * English note.
 */
export interface MemberRole {
  // English engineering note.
  id: string;
  // English engineering note.
  name?: string;
}

/**
 * English note.
 */
export const STANDARD_ROLE_IDS = {
  // English engineering note.
  OWNER: "owner",
  // English engineering note.
  ADMIN: "admin",
} as const;

// English engineering note.

/**
 * English note.
 */
export interface DbMeta {
  name: string; // English engineering note.
  platform: ChatPlatform; // English engineering note.
  type: ChatType; // English engineering note.
  imported_at: number; // English engineering note.
  group_id: string | null; // English engineering note.
  group_avatar: string | null; // English engineering note.
  owner_id: string | null; // English engineering note.
  session_gap_threshold: number | null; // English engineering note.
}

/**
 * English note.
 */
export interface DbMember {
  id: number; // English engineering note.
  platform_id: string; // English engineering note.
  account_name: string | null; // English engineering note.
  group_nickname: string | null; // English engineering note.
  aliases: string; // English engineering note.
  avatar: string | null; // English engineering note.
  roles: string; // English engineering note.
}

/**
 * English note.
 */
export interface DbMessage {
  id: number; // English engineering note.
  sender_id: number; // FK -> member.id
  sender_account_name: string | null; // English engineering note.
  sender_group_nickname: string | null; // English engineering note.
  ts: number; // English engineering note.
  type: MessageType; // English engineering note.
  content: string | null; // English engineering note.
  reply_to_message_id: string | null; // English engineering note.
}

// English engineering note.

/**
 * English note.
 */
export interface ParsedMember {
  platformId: string; // English engineering note.
  accountName: string; // English engineering note.
  groupNickname?: string; // English engineering note.
  avatar?: string; // English engineering note.
  roles?: MemberRole[]; // English engineering note.
}

/**
 * English note.
 */
export interface ParsedMessage {
  platformMessageId?: string; // English engineering note.
  senderPlatformId: string; // English engineering note.
  senderAccountName: string; // English engineering note.
  senderGroupNickname?: string; // English engineering note.
  timestamp: number; // English engineering note.
  type: MessageType; // English engineering note.
  content: string | null; // English engineering note.
  replyToMessageId?: string; // English engineering note.
}

/**
 * English note.
 */
export interface ParseResult {
  meta: {
    name: string;
    platform: ChatPlatform;
    type: ChatType;
    groupId?: string; // English engineering note.
    groupAvatar?: string; // English engineering note.
    ownerId?: string; // English engineering note.
  };
  members: ParsedMember[];
  messages: ParsedMessage[];
}

// English engineering note.

/**
 * English note.
 */
export interface AnalysisSession {
  id: string; // English engineering note.
  name: string; // English engineering note.
  platform: ChatPlatform;
  type: ChatType;
  importedAt: number; // English engineering note.
  messageCount: number; // English engineering note.
  memberCount: number; // English engineering note.
  dbPath: string; // English engineering note.
  groupId: string | null; // English engineering note.
  groupAvatar: string | null; // English engineering note.
  ownerId: string | null; // English engineering note.
  memberAvatar: string | null; // English engineering note.
  summaryCount: number; // English engineering note.
  aiConversationCount: number; // English engineering note.
}

/**
 * English note.
 */
export interface ImportProgress {
  stage: "detecting" | "reading" | "parsing" | "saving" | "done" | "error";
  progress: number; // 0-100
  message?: string;
  // English engineering note.
  bytesRead?: number;
  totalBytes?: number;
  messagesProcessed?: number;
}

/**
 * English note.
 */
export interface ExportProgress {
  stage: "preparing" | "exporting" | "done" | "error";
  currentBlock: number;
  totalBlocks: number;
  percentage: number; // 0-100
  message: string;
}

/**
 * English note.
 */
export interface ImportResult {
  success: boolean;
  sessionId?: string; // English engineering note.
  error?: string; // English engineering note.
}

// English engineering note.

/**
 * English note.
 */
export interface ChatSession {
  id: number; // English engineering note.
  startTs: number; // English engineering note.
  endTs: number; // English engineering note.
  messageCount: number; // English engineering note.
  isManual: boolean; // English engineering note.
  summary: string | null; // English engineering note.
}

/**
 * English note.
 */
export interface MessageContext {
  messageId: number; // English engineering note.
  sessionId: number; // English engineering note.
  topicId: number | null; // English engineering note.
}

/**
 * English note.
 */
export interface SessionConfig {
  // English engineering note.
  defaultGapThreshold: number;
}

/**
 * English note.
 */
export interface SessionStats {
  // English engineering note.
  sessionCount: number;
  // English engineering note.
  hasIndex: boolean;
  // English engineering note.
  gapThreshold: number;
}
