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

import type { PromptPreset } from '@/types/ai'

// English engineering note.

export type LocaleType = 'zh-CN' | 'en-US'

// English engineering note.

const i18nContent = {
  'zh-CN': {
    presetName: '默认分析助手',
    // English engineering note.
    roleDefinition: `你是一个专业但风格轻松的聊天记录分析助手。
你的任务是帮助用户理解和分析他们的聊天记录数据，同时可以适度使用 B 站/网络热梗和表情/颜文字活跃气氛，但不影响结论的准确性。`,
    // English engineering note.
    responseRules: `1. 基于工具返回的数据回答，不要编造信息
2. 如果数据不足以回答问题，请说明
3. 回答要简洁明了，使用 Markdown 格式
4. 可以引用具体的发言作为证据
5. 对于统计数据，可以适当总结趋势和特点
6. 可以适度加入 B 站/网络热梗、表情/颜文字（强度适中）
7. 玩梗不得影响事实准确与结论清晰，避免低俗或冒犯性表达`,
    lockedSection: {
      chatContext: {
        group: '群聊',
        private: '对话',
      },
      ownerNoteTemplate: (displayName: string, chatContext: string) =>
        `当前用户身份：
- 用户在${chatContext}中的身份是「${displayName}」
- 当用户提到"我"、"我的"时，指的就是「${displayName}」
- 查询"我"的发言时，使用 sender_id 参数筛选该成员
`,
      memberNote: {
        group: `成员查询策略：
- 当用户提到特定群成员（如"张三说过什么"、"小明的发言"等）时，应先调用 member_list 获取成员列表
- 群成员有三种名称：accountName（原始昵称）、groupNickname（群昵称）、aliases（用户自定义别名）
- 在 member_list 返回结果中匹配这三种名称
- 找到成员后，使用其 id 字段作为 search_messages 的 sender_id 参数来获取该成员的发言`,
        private: `成员查询策略：
- 私聊只有两个人，可以直接获取成员列表
- 当用户提到"对方"、"他/她"时，通过 member_list 获取另一方信息`,
      },
      currentDatePrefix: '当前日期是',
      timeParamsTemplate: (year: number, prevYear: number) =>
        `时间参数：按用户提到的精度组合 year/month/day/hour
- "10月" → year: ${year}, month: 10
- "10月1号" → year: ${year}, month: 10, day: 1
- "10月1号下午3点" → year: ${year}, month: 10, day: 1, hour: 15
未指定年份默认${year}年，若该月份未到则用${prevYear}年`,
      conclusion: '根据用户的问题，选择合适的工具获取数据，然后基于数据给出回答。',
      responseRulesLabel: '回答要求：',
    },
  },
  'en-US': {
    presetName: 'Default Analysis Assistant',
    roleDefinition: `You are a professional chat analysis assistant.
Your task is to help users understand and analyze their chat records.`,
    responseRules: `1. Answer based on data returned by tools, do not fabricate information
2. If data is insufficient to answer the question, explain
3. Keep answers concise and clear, use Markdown format
4. Quote specific messages as evidence when possible
5. For statistics, summarize trends and characteristics appropriately`,
    lockedSection: {
      chatContext: {
        group: 'group chat',
        private: 'conversation',
      },
      ownerNoteTemplate: (displayName: string, chatContext: string) =>
        `Current user identity:
- The user's identity in the ${chatContext} is "${displayName}"
- When the user mentions "I" or "my", it refers to "${displayName}"
- When querying "my" messages, use sender_id parameter to filter by this member
`,
      memberNote: {
        group: `Member query strategy:
- When the user mentions a specific group member (e.g., "what did John say", "Mary's messages"), first call member_list to get the member list
- Group members have three name types: accountName (original nickname), groupNickname (group nickname), aliases (user-defined aliases)
- Match the returned member_list records across all three name types
- After finding the member, use their id field as the sender_id parameter for search_messages to get their messages`,
        private: `Member query strategy:
- Private chats have only two people, you can directly get the member list
- When the user mentions "the other person" or "he/she", use member_list to identify the other party`,
      },
      currentDatePrefix: 'The current date is',
      timeParamsTemplate: (year: number, prevYear: number) =>
        `Time parameters: Combine year/month/day/hour based on user's specified precision
- "October" → year: ${year}, month: 10
- "October 1st" → year: ${year}, month: 10, day: 1
- "October 1st 3pm" → year: ${year}, month: 10, day: 1, hour: 15
Default to ${year} if year not specified, use ${prevYear} if the month hasn't arrived yet`,
      conclusion:
        "Based on the user's question, select appropriate tools to retrieve data, then provide an answer based on the data.",
      responseRulesLabel: 'Response requirements:',
    },
  },
}

// English engineering note.

/** English note.
export const DEFAULT_PRESET_ID = 'builtin-default'

/** English note.
export const DEFAULT_GROUP_PRESET_ID = DEFAULT_PRESET_ID
/** English note.
export const DEFAULT_PRIVATE_PRESET_ID = DEFAULT_PRESET_ID

// English engineering note.

/**
 * English note.
 * English note.
 */
export function getDefaultRoleDefinition(locale: LocaleType = 'zh-CN'): string {
  const content = i18nContent[locale] || i18nContent['zh-CN']
  return content.roleDefinition
}

/**
 * English note.
 * English note.
 */
export function getDefaultResponseRules(locale: LocaleType = 'zh-CN'): string {
  const content = i18nContent[locale] || i18nContent['zh-CN']
  return content.responseRules
}

/**
 * English note.
 * English note.
 */
export function getBuiltinPresetName(locale: LocaleType = 'zh-CN'): string {
  const content = i18nContent[locale] || i18nContent['zh-CN']
  return content.presetName
}

// English engineering note.

/**
 * English note.
 * English note.
 */
export function getBuiltinPresets(locale: LocaleType = 'zh-CN'): PromptPreset[] {
  const now = Date.now()

  const BUILTIN_DEFAULT: PromptPreset = {
    id: DEFAULT_PRESET_ID,
    name: getBuiltinPresetName(locale),
    roleDefinition: getDefaultRoleDefinition(locale),
    responseRules: getDefaultResponseRules(locale),
    isBuiltIn: true,
    createdAt: now,
    updatedAt: now,
  }

  return [BUILTIN_DEFAULT]
}

/** English note.
export const BUILTIN_PRESETS: PromptPreset[] = getBuiltinPresets('zh-CN')

/**
 * English note.
 * English note.
 * English note.
 */
export function getOriginalBuiltinPreset(presetId: string, locale: LocaleType = 'zh-CN'): PromptPreset | undefined {
  const presets = getBuiltinPresets(locale)
  return presets.find((p) => p.id === presetId)
}

// English engineering note.

/** English note.
export interface OwnerInfoPreview {
  displayName: string
}

/**
 * English note.
 * English note.
 *
 * English note.
 * English note.
 * English note.
 */
export function getLockedPromptSectionPreview(
  chatType: 'group' | 'private' = 'group',
  ownerInfo?: OwnerInfoPreview,
  locale: LocaleType = 'zh-CN'
): string {
  const content = i18nContent[locale] || i18nContent['zh-CN']
  const now = new Date()

  // English engineering note.
  const dateLocale = locale === 'zh-CN' ? 'zh-CN' : 'en-US'
  const currentDate = now.toLocaleDateString(dateLocale, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    weekday: 'long',
  })

  const chatContext = content.lockedSection.chatContext[chatType]

  // English engineering note.
  const ownerNote = ownerInfo ? content.lockedSection.ownerNoteTemplate(ownerInfo.displayName, chatContext) : ''

  const memberNote = content.lockedSection.memberNote[chatType]
  const year = now.getFullYear()
  const prevYear = year - 1

  return `${content.lockedSection.currentDatePrefix} ${currentDate}。
${ownerNote}
${memberNote}

${content.lockedSection.timeParamsTemplate(year, prevYear)}

${content.lockedSection.conclusion}`
}

/**
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 */
export function buildPromptPreview(
  roleDefinition: string,
  responseRules: string,
  chatType: 'group' | 'private' = 'group',
  ownerInfo?: OwnerInfoPreview,
  locale: LocaleType = 'zh-CN'
): string {
  const content = i18nContent[locale] || i18nContent['zh-CN']
  const lockedSection = getLockedPromptSectionPreview(chatType, ownerInfo, locale)

  return `${roleDefinition}

${lockedSection}

${content.lockedSection.responseRulesLabel}
${responseRules}`
}
