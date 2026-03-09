/**
 * English note.
 * English note.
 */

import { ref, computed } from 'vue'
import { storeToRefs } from 'pinia'
import { usePromptStore } from '@/stores/prompt'
import { useSessionStore } from '@/stores/session'

// English engineering note.
export interface ToolCallRecord {
  name: string
  displayName: string
  status: 'running' | 'done' | 'error'
  timestamp: number
  // Serialized tool invocation parameters for replay and debugging.
  params?: Record<string, unknown>
}

export interface ToolBlockContent {
  name: string
  displayName: string
  status: 'running' | 'done' | 'error'
  params?: Record<string, unknown>
}

// English engineering note.
export type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'think'; tag: string; text: string; durationMs?: number } // English engineering note.
  | {
      type: 'tool'
      tool: ToolBlockContent
    }

// English engineering note.
export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: number
  dataSource?: {
    toolsUsed: string[]
    toolRounds: number
  }
  // Ordered tool execution records attached to this message.
  toolCalls?: ToolCallRecord[]
  // Rich content blocks rendered in chat explorer (text/think/tool).
  contentBlocks?: ContentBlock[]
  isStreaming?: boolean
}

// English engineering note.
export interface SourceMessage {
  id: number
  senderName: string
  senderPlatformId: string
  content: string
  timestamp: number
  type: number
}

// English engineering note.
export interface ToolStatus {
  name: string
  displayName: string
  status: 'running' | 'done' | 'error'
  result?: unknown
}

// English engineering note.
export interface TokenUsage {
  promptTokens: number
  completionTokens: number
  totalTokens: number
}

// English engineering note.
// English engineering note.

// Session owner profile passed to agent context for better parameter inference.
interface OwnerInfo {
  platformId: string
  displayName: string
}

export function useAIChat(
  sessionId: string,
  timeFilter?: { startTs: number; endTs: number },
  chatType: 'group' | 'private' = 'group',
  locale: string = 'zh-CN'
) {
  const AGENT_TOOL_ALIAS_PAIRS: Array<[string, string]> = [
    ['searchmessages', 'search_messages'],
    ['searchMessages', 'search_messages'],
    ['recent_messages', 'get_recent_messages'],
    ['getrecentmessages', 'get_recent_messages'],
    ['recentmessages', 'get_recent_messages'],
    ['recentMessages', 'get_recent_messages'],
    ['get_member_stats', 'member_stats'],
    ['memberstats', 'member_stats'],
    ['getmemberstats', 'member_stats'],
    ['getMemberStats', 'member_stats'],
    ['get_time_stats', 'time_stats'],
    ['timestats', 'time_stats'],
    ['gettimestats', 'time_stats'],
    ['getTimeStats', 'time_stats'],
    ['get_member_list', 'member_list'],
    ['get_group_members', 'member_list'],
    ['memberlist', 'member_list'],
    ['getmemberlist', 'member_list'],
    ['getgroupmembers', 'member_list'],
    ['getMemberList', 'member_list'],
    ['getGroupMembers', 'member_list'],
    ['member_name_history', 'nickname_history'],
    ['get_member_name_history', 'nickname_history'],
    ['nicknamehistory', 'nickname_history'],
    ['membernamehistory', 'nickname_history'],
    ['getmembernamehistory', 'nickname_history'],
    ['memberNameHistory', 'nickname_history'],
    ['getMemberNameHistory', 'nickname_history'],
    ['get_conversation_between', 'conversation_between'],
    ['conversationbetween', 'conversation_between'],
    ['getconversationbetween', 'conversation_between'],
    ['getConversationBetween', 'conversation_between'],
    ['get_message_context', 'message_context'],
    ['messagecontext', 'message_context'],
    ['getmessagecontext', 'message_context'],
    ['getMessageContext', 'message_context'],
    ['searchsessions', 'search_sessions'],
    ['searchSessions', 'search_sessions'],
    ['session_messages', 'get_session_messages'],
    ['getsessionmessages', 'get_session_messages'],
    ['sessionmessages', 'get_session_messages'],
    ['sessionMessages', 'get_session_messages'],
    ['getSessionMessages', 'get_session_messages'],
    ['session_summary', 'get_session_summary'],
    ['get_session_summaries', 'get_session_summary'],
    ['sessionsummary', 'get_session_summary'],
    ['getsessionsummary', 'get_session_summary'],
    ['getsessionsummaries', 'get_session_summary'],
    ['sessionSummary', 'get_session_summary'],
    ['getSessionSummary', 'get_session_summary'],
    ['getSessionSummaries', 'get_session_summary'],
    ['semantic_search_messages', 'semantic_search'],
    ['semanticsearch', 'semantic_search'],
    ['semanticsearchmessages', 'semantic_search'],
    ['semanticSearchMessages', 'semantic_search'],
  ]

  function normalizeAgentToolAliasInput(raw: string): string {
    const trimmed = raw.trim()
    if (!trimmed) return ''
    let out = ''
    let prevWasSep = false
    for (const ch of trimmed) {
      if (ch >= 'A' && ch <= 'Z') {
        if (out && !prevWasSep) out += '_'
        out += ch.toLowerCase()
        prevWasSep = false
        continue
      }
      if (ch === '-' || ch === ' ' || ch === '/' || ch === '_') {
        if (out && !prevWasSep) out += '_'
        prevWasSep = true
        continue
      }
      out += ch.toLowerCase()
      prevWasSep = false
    }
    return out
  }

  function registerAlias(map: Record<string, string>, alias: string, canonical: string) {
    const normalizedAlias = normalizeAgentToolAliasInput(alias)
    const normalizedCanonical = normalizeAgentToolAliasInput(canonical)
    if (!normalizedAlias || !normalizedCanonical) return
    map[normalizedAlias] = normalizedCanonical
  }

  function buildFallbackAliasMap(): Record<string, string> {
    const out: Record<string, string> = {}
    for (const [alias, canonical] of AGENT_TOOL_ALIAS_PAIRS) {
      registerAlias(out, alias, canonical)
    }
    return out
  }

  const agentToolAliasToCanonical = ref<Record<string, string>>(buildFallbackAliasMap())

  async function refreshAgentToolAliasMap() {
    if (!window.agentApi?.listTools) return
    try {
      const tools = await window.agentApi.listTools()
      if (!Array.isArray(tools) || tools.length === 0) return
      const nextMap = buildFallbackAliasMap()
      for (const item of tools) {
        const canonical =
          typeof item?.name === 'string' && item.name.trim().length > 0 ? item.name : ''
        if (!canonical) continue
        registerAlias(nextMap, canonical, canonical)
        const aliases = Array.isArray(item?.aliases) ? item.aliases : []
        for (const alias of aliases) {
          if (typeof alias === 'string' && alias.trim().length > 0) {
            registerAlias(nextMap, alias, canonical)
          }
        }
      }
      agentToolAliasToCanonical.value = nextMap
    } catch (error) {
      console.warn('[AI] Failed to refresh agent tool alias map:', error)
    }
  }

  function normalizeAgentToolName(rawName?: string): string {
    if (!rawName) return ''
    const normalized = normalizeAgentToolAliasInput(rawName)
    if (!normalized) return ''
    return agentToolAliasToCanonical.value[normalized] || normalized
  }

  // English engineering note.
  const promptStore = usePromptStore()
  const sessionStore = useSessionStore()
  const { activePreset, aiGlobalSettings } = storeToRefs(promptStore)

  // English engineering note.
  const currentPromptConfig = computed(() => {
    return {
      roleDefinition: activePreset.value.roleDefinition,
      responseRules: activePreset.value.responseRules,
    }
  })

  // English engineering note.
  const messages = ref<ChatMessage[]>([])
  const sourceMessages = ref<SourceMessage[]>([])
  const currentKeywords = ref<string[]>([])
  const isLoadingSource = ref(false)
  const isAIThinking = ref(false)
  const currentConversationId = ref<string | null>(null)

  // English engineering note.
  const ownerInfo = ref<OwnerInfo | undefined>(undefined)

  // English engineering note.
  const currentToolStatus = ref<ToolStatus | null>(null)
  const toolsUsedInCurrentRound = ref<string[]>([])

  // English engineering note.
  const sessionTokenUsage = ref<TokenUsage>({ promptTokens: 0, completionTokens: 0, totalTokens: 0 })

  // English engineering note.
  async function initOwnerInfo() {
    const ownerId = sessionStore.currentSession?.ownerId
    if (!ownerId) {
      ownerInfo.value = undefined
      return
    }

    try {
      // English engineering note.
      const members = await window.chatApi.getMembers(sessionId)
      const ownerMember = members.find((m) => m.platformId === ownerId)
      if (ownerMember) {
        ownerInfo.value = {
          platformId: ownerId,
          displayName: ownerMember.groupNickname || ownerMember.accountName || ownerId,
        }
        console.log('[AI] Owner info loaded:', ownerInfo.value)
      }
    } catch (error) {
      console.error('[AI] Failed to load owner info:', error)
      ownerInfo.value = undefined
    }
  }

  // English engineering note.
  initOwnerInfo()
  void refreshAgentToolAliasMap()

  // English engineering note.
  let isAborted = false
  // English engineering note.
  let currentRequestId = ''
  // English engineering note.
  let currentAgentRequestId = ''

  // English engineering note.
  function generateId(prefix: string): string {
    return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
  }

  /**
   * English note.
   */
  async function sendMessage(content: string): Promise<void> {
    console.log('[AI] ====== Begin user message pipeline ======')
    console.log('[AI] User input:', content)

    if (!content.trim() || isAIThinking.value) {
      console.log('[AI] Skipped because content is empty or the assistant is already thinking')
      return
    }

    // English engineering note.
    console.log('[AI] Checking LLM configuration...')
    const hasConfig = await window.llmApi.hasConfig()
    console.log('[AI] LLM configuration state:', hasConfig)

    if (!hasConfig) {
      console.log('[AI] No LLM configuration found, returning setup hint')
      messages.value.push({
        id: generateId('error'),
        role: 'assistant',
        content: '⚠️ 请先配置 AI 服务。点击左下角「设置」按钮前往「模型配置Tab」进行配置。',
        timestamp: Date.now(),
      })
      return
    }

    // English engineering note.
    const userMessage: ChatMessage = {
      id: generateId('user'),
      role: 'user',
      content,
      timestamp: Date.now(),
      toolCalls: [], // English engineering note.
    }
    messages.value.push(userMessage)
    console.log('[AI] User message appended')

    // English engineering note.
    isAIThinking.value = true
    isLoadingSource.value = true
    currentToolStatus.value = null
    toolsUsedInCurrentRound.value = []
    isAborted = false
    // English engineering note.
    currentRequestId = generateId('req')
    const thisRequestId = currentRequestId
    console.log('[AI] Starting agent processing...', { requestId: thisRequestId })

    // English engineering note.
    const aiMessageId = generateId('ai')
    const aiMessage: ChatMessage = {
      id: aiMessageId,
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
      isStreaming: true,
      contentBlocks: [], // English engineering note.
    }
    messages.value.push(aiMessage)
    const aiMessageIndex = messages.value.length - 1
    let hasStreamError = false

    // English engineering note.
    const updateAIMessage = (updates: Partial<ChatMessage>) => {
      messages.value[aiMessageIndex] = {
        ...messages.value[aiMessageIndex],
        ...updates,
      }
    }

    // English engineering note.
    const appendTextToBlocks = (text: string) => {
      if (!text) return
      const blocks = messages.value[aiMessageIndex].contentBlocks || []
      const lastBlock = blocks[blocks.length - 1]

      if (text.trim().length === 0 && (!lastBlock || lastBlock.type !== 'text')) {
        // English engineering note.
        return
      }

      if (lastBlock && lastBlock.type === 'text') {
        // English engineering note.
        lastBlock.text += text
      } else {
        // English engineering note.
        blocks.push({ type: 'text', text })
      }

      updateAIMessage({
        contentBlocks: [...blocks],
        content: messages.value[aiMessageIndex].content + text, // English engineering note.
      })
    }

    // English engineering note.
    const appendThinkToBlocks = (text: string, tag?: string, durationMs?: number) => {
      if (!text && durationMs === undefined) return
      const blocks = messages.value[aiMessageIndex].contentBlocks || []
      const thinkTag = tag || 'think'
      const lastBlock = blocks[blocks.length - 1]

      if (
        text.trim().length === 0 &&
        durationMs === undefined &&
        (!lastBlock || lastBlock.type !== 'think' || lastBlock.tag !== thinkTag)
      ) {
        // English engineering note.
        return
      }

      let targetBlock = lastBlock
      if (lastBlock && lastBlock.type === 'think' && lastBlock.tag === thinkTag) {
        lastBlock.text += text
      } else if (text.trim().length > 0) {
        targetBlock = { type: 'think', tag: thinkTag, text }
        blocks.push(targetBlock)
      } else if (durationMs !== undefined) {
        // English engineering note.
        for (let i = blocks.length - 1; i >= 0; i--) {
          const block = blocks[i]
          if (block.type === 'think' && block.tag === thinkTag) {
            targetBlock = block
            break
          }
        }
      }

      if (durationMs !== undefined && targetBlock && targetBlock.type === 'think') {
        targetBlock.durationMs = durationMs
      }

      updateAIMessage({ contentBlocks: [...blocks] })
    }

    // English engineering note.
    const addToolBlock = (toolName: string, params?: Record<string, unknown>) => {
      const blocks = messages.value[aiMessageIndex].contentBlocks || []
      blocks.push({
        type: 'tool',
        tool: {
          name: toolName,
          displayName: toolName,
          status: 'running',
          params,
        },
      })
      updateAIMessage({ contentBlocks: [...blocks] })
    }

    // English engineering note.
    const updateToolBlockStatus = (toolName: string, status: 'done' | 'error') => {
      const blocks = messages.value[aiMessageIndex].contentBlocks || []
      // English engineering note.
      for (let i = blocks.length - 1; i >= 0; i--) {
        const block = blocks[i]
        if (block.type === 'tool' && block.tool.name === toolName && block.tool.status === 'running') {
          block.tool.status = status
          break
        }
      }
      updateAIMessage({ contentBlocks: [...blocks] })
    }

    try {
      // English engineering note.
      // English engineering note.
      const context = {
        sessionId,
        timeFilter: timeFilter ? { startTs: timeFilter.startTs, endTs: timeFilter.endTs } : undefined,
        maxMessagesLimit: aiGlobalSettings.value.maxMessagesPerRequest,
        ownerInfo: ownerInfo.value
          ? { platformId: ownerInfo.value.platformId, displayName: ownerInfo.value.displayName }
          : undefined,
      }

      console.log('[AI] Built context:', {
        sessionId,
        maxMessagesLimit: context.maxMessagesLimit,
        ownerInfo: context.ownerInfo,
        aiGlobalSettings: aiGlobalSettings.value,
      })

      // English engineering note.
      // English engineering note.
      const maxHistoryRounds = aiGlobalSettings.value.maxHistoryRounds ?? 5
      const maxHistoryMessages = maxHistoryRounds * 2

      const historyMessages = messages.value
        .slice(0, -2) // English engineering note.
        .filter((msg) => msg.role === 'user' || msg.role === 'assistant')
        .filter((msg) => msg.content && msg.content.trim() !== '') // English engineering note.
        .slice(-maxHistoryMessages) // English engineering note.
        .map((msg) => ({
          role: msg.role as 'user' | 'assistant',
          content: msg.content,
        }))

      console.log('[AI] Calling agent API...', {
        context,
        historyLength: historyMessages.length,
        chatType,
        promptConfig: currentPromptConfig.value,
      })

      // English engineering note.
      const { requestId: agentReqId, promise: agentPromise } = window.agentApi.runStream(
        content,
        context,
        (chunk) => {
          // English engineering note.
          if (isAborted || thisRequestId !== currentRequestId) {
            console.log('[AI] Ignoring chunk because the request was aborted or expired', {
              isAborted,
              thisRequestId,
              currentRequestId,
            })
            return
          }

          // English engineering note.
          if (chunk.type === 'tool_start' || chunk.type === 'tool_result') {
            console.log('[AI] Agent chunk:', chunk.type, chunk.toolName)
          }

          switch (chunk.type) {
            case 'content':
              // English engineering note.
              if (chunk.content) {
                currentToolStatus.value = null
                appendTextToBlocks(chunk.content)
              }
              break

            case 'think':
              // English engineering note.
              if (chunk.content) {
                appendThinkToBlocks(chunk.content, chunk.thinkTag)
              } else if (chunk.thinkDurationMs !== undefined) {
                appendThinkToBlocks('', chunk.thinkTag, chunk.thinkDurationMs)
              }
              break

            case 'tool_start':
              // English engineering note.
              console.log('[AI] Tool execution started:', chunk.toolName, chunk.toolParams)
              if (chunk.toolName) {
                const normalizedToolName = normalizeAgentToolName(chunk.toolName)
                if (!normalizedToolName) {
                  break
                }
                const toolParams = chunk.toolParams as Record<string, unknown> | undefined
                currentToolStatus.value = {
                  name: normalizedToolName,
                  displayName: normalizedToolName,
                  status: 'running',
                }
                toolsUsedInCurrentRound.value.push(normalizedToolName)

                // English engineering note.
                addToolBlock(normalizedToolName, toolParams)
              }
              break

            case 'tool_result':
              // English engineering note.
              console.log('[AI] Tool execution result:', chunk.toolName, chunk.toolResult)
              if (chunk.toolName) {
                const normalizedToolName = normalizeAgentToolName(chunk.toolName)
                if (!normalizedToolName) {
                  break
                }
                if (currentToolStatus.value?.name === normalizedToolName) {
                  currentToolStatus.value = {
                    ...currentToolStatus.value,
                    status: 'done',
                  }
                }
                // English engineering note.
                updateToolBlockStatus(normalizedToolName, 'done')
              }
              isLoadingSource.value = false
              break

            case 'done':
              // English engineering note.
              console.log('[AI] Agent completed', chunk.usage)
              currentToolStatus.value = null
              // English engineering note.
              if (chunk.usage) {
                sessionTokenUsage.value = {
                  promptTokens: sessionTokenUsage.value.promptTokens + chunk.usage.promptTokens,
                  completionTokens: sessionTokenUsage.value.completionTokens + chunk.usage.completionTokens,
                  totalTokens: sessionTokenUsage.value.totalTokens + chunk.usage.totalTokens,
                }
                console.log('[AI] Token usage updated:', sessionTokenUsage.value)
              }
              break

            case 'error':
              // English engineering note.
              console.error('[AI] Agent error:', chunk.error)
              if (currentToolStatus.value) {
                currentToolStatus.value = {
                  ...currentToolStatus.value,
                  status: 'error',
                }
                // English engineering note.
                updateToolBlockStatus(currentToolStatus.value.name, 'error')
              }
              if (!hasStreamError) {
                hasStreamError = true
                const errorCode = chunk.errorCode || 'error.agent_runtime'
                const errorMessage = chunk.errorMessage || chunk.error || 'Unknown error'
                // English engineering note.
                appendTextToBlocks(`\n\n❌ Processing failed (${errorCode}): ${errorMessage}`)
                updateAIMessage({ isStreaming: false })
              }
              break
          }
        },
        historyMessages,
        chatType,
        // English engineering note.
        {
          roleDefinition: currentPromptConfig.value.roleDefinition,
          responseRules: currentPromptConfig.value.responseRules,
        },
        locale
      )

      // English engineering note.
      currentAgentRequestId = agentReqId
      console.log('[AI] Agent request started, agentReqId:', agentReqId)

      // English engineering note.
      const result = await agentPromise
      console.log('[AI] Agent returned:', result)

      // English engineering note.
      if (thisRequestId !== currentRequestId) {
        console.log('[AI] Request expired, skipping result handling')
        return
      }

      if (result.success && result.result) {
        // English engineering note.
        messages.value[aiMessageIndex] = {
          ...messages.value[aiMessageIndex],
          dataSource: {
            toolsUsed: result.result.toolsUsed,
            toolRounds: result.result.toolRounds,
          },
          isStreaming: false,
        }

        // English engineering note.
        console.log('[AI] Saving conversation...')
        await saveConversation(userMessage, messages.value[aiMessageIndex])
        console.log('[AI] Conversation saved')
      } else {
        // English engineering note.
        const detail =
          (result as { error?: string; errorMessage?: string }).errorMessage ||
          result.error ||
          'Unknown error'
        const errorCode =
          (result as { error?: string; errorMessage?: string }).error || 'error.agent_runtime'
        const errorText = `❌ Processing failed (${errorCode}): ${detail}`
        if (!hasStreamError) {
          // English engineering note.
          appendTextToBlocks(`\n\n${errorText}`)
        }
        messages.value[aiMessageIndex] = {
          ...messages.value[aiMessageIndex],
          isStreaming: false,
        }
      }

      console.log('[AI] ====== Message pipeline completed ======')
    } catch (error) {
      console.error('[AI] ====== Message pipeline failed ======')
      console.error('[AI] Error:', error)

      messages.value[aiMessageIndex] = {
        ...messages.value[aiMessageIndex],
        content: `❌ Processing failed: ${error instanceof Error ? error.message : 'Unknown error'}

Please check:
- network connectivity
- API key validity
- model configuration`,
        isStreaming: false,
      }
    } finally {
      isAIThinking.value = false
      isLoadingSource.value = false
    }
  }

  /**
   * English note.
   */
  async function saveConversation(userMsg: ChatMessage, aiMsg: ChatMessage): Promise<void> {
    console.log('[AI] saveConversation invoked')

    try {
      // English engineering note.
      if (!currentConversationId.value) {
        const title = userMsg.content.slice(0, 50) + (userMsg.content.length > 50 ? '...' : '')
        const conversation = await window.aiApi.createConversation(sessionId, title)
        currentConversationId.value = conversation.id
        console.log('[AI] Created new conversation:', conversation.id)
      }

      // English engineering note.
      await window.aiApi.addMessage(currentConversationId.value, 'user', userMsg.content)

      // English engineering note.
      // English engineering note.
      const serializableContentBlocks = aiMsg.contentBlocks
        ? JSON.parse(JSON.stringify(aiMsg.contentBlocks))
        : undefined
      console.log('[AI] Saving AI message:', {
        contentLength: aiMsg.content?.length,
        hasContentBlocks: !!serializableContentBlocks,
        contentBlocksLength: serializableContentBlocks?.length,
      })
      await window.aiApi.addMessage(
        currentConversationId.value,
        'assistant',
        aiMsg.content,
        undefined, // English engineering note.
        undefined,
        serializableContentBlocks // English engineering note.
      )
      console.log('[AI] Message save completed')
    } catch (error) {
      console.error('[AI] Failed to save conversation:', error)
    }
  }

  /**
   * English note.
   */
  async function loadConversation(conversationId: string): Promise<void> {
    console.log('[AI] Loading conversation history, conversationId:', conversationId)
    try {
      const history = await window.aiApi.getMessages(conversationId)
      currentConversationId.value = conversationId

      console.log(
        '[AI] Raw messages loaded from database:',
        history.map((m) => ({
          id: m.id,
          role: m.role,
          contentLength: m.content?.length,
          hasContentBlocks: !!m.contentBlocks,
          contentBlocksLength: m.contentBlocks?.length,
        }))
      )

      messages.value = history.map((msg) => ({
        id: msg.id,
        role: msg.role,
        content: msg.content,
        timestamp: msg.timestamp * 1000,
        // English engineering note.
        contentBlocks: msg.contentBlocks as ContentBlock[] | undefined,
      }))
      console.log('[AI] Conversation history loaded, messages count:', messages.value.length)
    } catch (error) {
      console.error('[AI] Failed to load conversation history:', error)
    }
  }

  /**
   * English note.
   */
  function startNewConversation(welcomeMessage?: string): void {
    currentConversationId.value = null
    messages.value = []
    sourceMessages.value = []
    currentKeywords.value = []
    // English engineering note.
    sessionTokenUsage.value = { promptTokens: 0, completionTokens: 0, totalTokens: 0 }

    if (welcomeMessage) {
      messages.value.push({
        id: generateId('welcome'),
        role: 'assistant',
        content: welcomeMessage,
        timestamp: Date.now(),
      })
    }
  }

  /**
   * English note.
   */
  async function loadMoreSourceMessages(): Promise<void> {
    // English engineering note.
  }

  /**
   * English note.
   */
  async function updateMaxMessages(): Promise<void> {
    // English engineering note.
  }

  /**
   * English note.
   */
  async function stopGeneration(): Promise<void> {
    if (!isAIThinking.value) return

    console.log('[AI] User stopped generation')
    isAborted = true
    isAIThinking.value = false
    isLoadingSource.value = false
    currentToolStatus.value = null

    // English engineering note.
    if (currentAgentRequestId) {
      console.log('[AI] Aborting agent request:', currentAgentRequestId)
      try {
        await window.agentApi.abort(currentAgentRequestId)
        console.log('[AI] Agent request aborted')
      } catch (error) {
        console.error('[AI] Failed to abort agent request:', error)
      }
      currentAgentRequestId = ''
    }

    // English engineering note.
    const lastMessage = messages.value[messages.value.length - 1]
    if (lastMessage && lastMessage.role === 'assistant' && lastMessage.isStreaming) {
      lastMessage.isStreaming = false
      lastMessage.content += '\n\n_（已停止生成）_'
    }
  }

  return {
    // English engineering note.
    messages,
    sourceMessages,
    currentKeywords,
    isLoadingSource,
    isAIThinking,
    currentConversationId,
    currentToolStatus,
    toolsUsedInCurrentRound,
    sessionTokenUsage,

    // English engineering note.
    sendMessage,
    loadConversation,
    startNewConversation,
    loadMoreSourceMessages,
    updateMaxMessages,
    stopGeneration,
  }
}
