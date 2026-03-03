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
  /** English note.
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
  /** English note.
  toolCalls?: ToolCallRecord[]
  /** English note.
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

/** English note.
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
  const AGENT_TOOL_ALIAS_TO_CANONICAL: Record<string, string> = {
    get_member_stats: 'member_stats',
    get_time_stats: 'time_stats',
    get_group_members: 'member_list',
    get_member_name_history: 'nickname_history',
    get_conversation_between: 'conversation_between',
    get_message_context: 'message_context',
    get_session_summaries: 'get_session_summary',
    semantic_search_messages: 'semantic_search',
  }

  function normalizeAgentToolName(rawName?: string): string {
    if (!rawName) return ''
    const normalized = rawName.trim()
    if (!normalized) return ''
    return AGENT_TOOL_ALIAS_TO_CANONICAL[normalized] || normalized
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
        console.log('[AI] Owner 信息已加载:', ownerInfo.value)
      }
    } catch (error) {
      console.error('[AI] 获取 Owner 信息失败:', error)
      ownerInfo.value = undefined
    }
  }

  // English engineering note.
  initOwnerInfo()

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
    console.log('[AI] ====== 开始处理用户消息 ======')
    console.log('[AI] 用户输入:', content)

    if (!content.trim() || isAIThinking.value) {
      console.log('[AI] 跳过：内容为空或正在思考')
      return
    }

    // English engineering note.
    console.log('[AI] 检查 LLM 配置...')
    const hasConfig = await window.llmApi.hasConfig()
    console.log('[AI] LLM 配置状态:', hasConfig)

    if (!hasConfig) {
      console.log('[AI] 未配置 LLM，显示提示')
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
    console.log('[AI] 已添加用户消息')

    // English engineering note.
    isAIThinking.value = true
    isLoadingSource.value = true
    currentToolStatus.value = null
    toolsUsedInCurrentRound.value = []
    isAborted = false
    // English engineering note.
    currentRequestId = generateId('req')
    const thisRequestId = currentRequestId
    console.log('[AI] 开始 Agent 处理...', { requestId: thisRequestId })

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

      console.log('[AI] 构建 context:', {
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

      console.log('[AI] 调用 Agent API...', {
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
            console.log('[AI] 已中止或请求已过期，忽略 chunk', {
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
              console.log('[AI] 工具开始执行:', chunk.toolName, chunk.toolParams)
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
              console.log('[AI] 工具执行结果:', chunk.toolName, chunk.toolResult)
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
              console.log('[AI] Agent 完成', chunk.usage)
              currentToolStatus.value = null
              // English engineering note.
              if (chunk.usage) {
                sessionTokenUsage.value = {
                  promptTokens: sessionTokenUsage.value.promptTokens + chunk.usage.promptTokens,
                  completionTokens: sessionTokenUsage.value.completionTokens + chunk.usage.completionTokens,
                  totalTokens: sessionTokenUsage.value.totalTokens + chunk.usage.totalTokens,
                }
                console.log('[AI] Token 使用量更新:', sessionTokenUsage.value)
              }
              break

            case 'error':
              // English engineering note.
              console.error('[AI] Agent 错误:', chunk.error)
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
                const errorMessage = chunk.error || '未知错误'
                // English engineering note.
                appendTextToBlocks(`\n\n❌ 处理失败：${errorMessage}`)
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
      console.log('[AI] Agent 请求已启动，agentReqId:', agentReqId)

      // English engineering note.
      const result = await agentPromise
      console.log('[AI] Agent 返回结果:', result)

      // English engineering note.
      if (thisRequestId !== currentRequestId) {
        console.log('[AI] 请求已过期，跳过结果处理')
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
        console.log('[AI] 保存对话...')
        await saveConversation(userMessage, messages.value[aiMessageIndex])
        console.log('[AI] 对话已保存')
      } else {
        // English engineering note.
        const detail = (result as { error?: string; errorMessage?: string }).errorMessage || result.error || '未知错误'
        const errorText = `❌ 处理失败：${detail}`
        if (!hasStreamError) {
          // English engineering note.
          appendTextToBlocks(`\n\n${errorText}`)
        }
        messages.value[aiMessageIndex] = {
          ...messages.value[aiMessageIndex],
          isStreaming: false,
        }
      }

      console.log('[AI] ====== 处理完成 ======')
    } catch (error) {
      console.error('[AI] ====== 处理失败 ======')
      console.error('[AI] 错误:', error)

      messages.value[aiMessageIndex] = {
        ...messages.value[aiMessageIndex],
        content: `❌ 处理失败：${error instanceof Error ? error.message : '未知错误'}

请检查：
- 网络连接是否正常
- API Key 是否有效
- 配置是否正确`,
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
    console.log('[AI] saveConversation 调用')

    try {
      // English engineering note.
      if (!currentConversationId.value) {
        const title = userMsg.content.slice(0, 50) + (userMsg.content.length > 50 ? '...' : '')
        const conversation = await window.aiApi.createConversation(sessionId, title)
        currentConversationId.value = conversation.id
        console.log('[AI] 创建了新对话:', conversation.id)
      }

      // English engineering note.
      await window.aiApi.addMessage(currentConversationId.value, 'user', userMsg.content)

      // English engineering note.
      // English engineering note.
      const serializableContentBlocks = aiMsg.contentBlocks
        ? JSON.parse(JSON.stringify(aiMsg.contentBlocks))
        : undefined
      console.log('[AI] 保存 AI 消息:', {
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
      console.log('[AI] 消息保存完成')
    } catch (error) {
      console.error('[AI] 保存对话失败：', error)
    }
  }

  /**
   * English note.
   */
  async function loadConversation(conversationId: string): Promise<void> {
    console.log('[AI] 加载对话历史，conversationId:', conversationId)
    try {
      const history = await window.aiApi.getMessages(conversationId)
      currentConversationId.value = conversationId

      console.log(
        '[AI] 从数据库加载的原始消息:',
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
      console.log('[AI] 加载完成，messages.value 数量:', messages.value.length)
    } catch (error) {
      console.error('[AI] 加载对话历史失败：', error)
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

    console.log('[AI] 用户停止生成')
    isAborted = true
    isAIThinking.value = false
    isLoadingSource.value = false
    currentToolStatus.value = null

    // English engineering note.
    if (currentAgentRequestId) {
      console.log('[AI] 中止 Agent 请求:', currentAgentRequestId)
      try {
        await window.agentApi.abort(currentAgentRequestId)
        console.log('[AI] Agent 请求已中止')
      } catch (error) {
        console.error('[AI] 中止 Agent 请求失败:', error)
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
