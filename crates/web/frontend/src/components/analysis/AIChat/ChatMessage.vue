<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import dayjs from 'dayjs'
import MarkdownIt from 'markdown-it'
import userAvatar from '@/assets/images/xenobot-avatar.svg'
import type { ContentBlock, ToolBlockContent } from '@/composables/useAIChat'
import CaptureButton from '@/components/common/CaptureButton.vue'

const { t, te, locale } = useI18n()

// Props
const props = defineProps<{
  role: 'user' | 'assistant'
  content: string
  timestamp: number
  isStreaming?: boolean
  /** English note.
  contentBlocks?: ContentBlock[]
  /** English note.
  showCaptureButton?: boolean
}>()

// English engineering note.
const formattedTime = computed(() => {
  return dayjs(props.timestamp).format('HH:mm')
})

// English engineering note.
const isUser = computed(() => props.role === 'user')

// English engineering note.
const md = new MarkdownIt({
  html: false, // English engineering note.
  breaks: true, // English engineering note.
  linkify: true, // English engineering note.
  typographer: true, // English engineering note.
})

// English engineering note.
function renderMarkdown(text: string): string {
  if (!text) return ''
  return md.render(text)
}

// English engineering note.
function getThinkLabel(tag: string): string {
  const normalized = tag?.toLowerCase() || 'think'
  if (normalized === 'analysis') return t('ai.chat.message.think.labels.analysis')
  if (normalized === 'reasoning') return t('ai.chat.message.think.labels.reasoning')
  if (normalized === 'reflection') return t('ai.chat.message.think.labels.reflection')
  if (normalized === 'think' || normalized === 'thought' || normalized === 'thinking') {
    return t('ai.chat.message.think.labels.think')
  }
  return t('ai.chat.message.think.labels.other', { tag })
}

// English engineering note.
function formatThinkDuration(durationMs?: number): string {
  if (!durationMs) return ''
  const seconds = (durationMs / 1000).toFixed(1)
  return t('ai.chat.message.think.duration', { seconds })
}

// English engineering note.
const renderedContent = computed(() => {
  if (!props.content) return ''
  return md.render(props.content)
})

// English engineering note.
const visibleBlocks = computed(() => {
  const blocks = props.contentBlocks || []
  return blocks.filter((block) => {
    if (block.type === 'text' || block.type === 'think') {
      return block.text.trim().length > 0
    }
    return true
  })
})

// English engineering note.
const useBlocksRendering = computed(() => {
  return props.role === 'assistant' && visibleBlocks.value.length > 0
})

// English engineering note.
function formatTimeParams(params: Record<string, unknown>): string {
  const startTs = params.startTs || params.start_ts
  const endTs = params.endTs || params.end_ts
  if (startTs || endTs) {
    const start = startTs ? String(startTs) : ''
    const end = endTs ? String(endTs) : ''
    if (start && end) {
      return `${start} ~ ${end}`
    }
    return start || end
  }

  // English engineering note.
  if (params.start_time || params.end_time) {
    const start = params.start_time ? String(params.start_time) : ''
    const end = params.end_time ? String(params.end_time) : ''
    if (start && end) {
      return `${start} ~ ${end}`
    }
    return start || end
  }

  // English engineering note.
  if (params.year) {
    if (locale.value === 'zh-CN') {
      let result = `${params.year}年`
      if (params.month) {
        result += `${params.month}月`
        if (params.day) {
          result += `${params.day}日`
          if (params.hour !== undefined) {
            result += ` ${params.hour}点`
          }
        }
      }
      return result
    } else {
      // English format
      const monthNames = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
      let result = ''
      if (params.month) {
        result = monthNames[(params.month as number) - 1] || String(params.month)
        if (params.day) {
          result += ` ${params.day}`
          if (params.hour !== undefined) {
            const hour = params.hour as number
            const suffix = hour >= 12 ? 'pm' : 'am'
            const hour12 = hour % 12 || 12
            result += `, ${hour12}${suffix}`
          }
        }
        result += `, ${params.year}`
      } else {
        result = String(params.year)
      }
      return result
    }
  }

  return ''
}

// English engineering note.
function formatToolParams(tool: ToolBlockContent): string {
  if (!tool.params) return ''

  const name = tool.name
  const params = tool.params

  if (name === 'search_messages' || name === 'semantic_search') {
    const keywords = (params.keywords as string[] | undefined) || []
    const query = params.query ? String(params.query) : ''
    const parts: string[] = []

    if (query) {
      parts.push(`${t('ai.chat.message.toolParams.keywords')}: ${query}`)
    } else if (keywords && keywords.length > 0) {
      parts.push(`${t('ai.chat.message.toolParams.keywords')}: ${keywords.join(', ')}`)
    }

    const timeStr = formatTimeParams(params)
    if (timeStr) {
      parts.push(`${t('ai.chat.message.toolParams.time')}: ${timeStr}`)
    }

    return parts.join(' | ')
  }

  if (name === 'get_recent_messages') {
    const parts: string[] = []
    parts.push(t('ai.chat.message.toolParams.getMessages', { count: params.limit || 100 }))

    const timeStr = formatTimeParams(params)
    if (timeStr) {
      parts.push(timeStr)
    }

    return parts.join(' | ')
  }

  if (name === 'conversation_between' || name === 'get_conversation_between') {
    const parts: string[] = []
    const memberId1 = params.memberId1 || params.member_id_1
    const memberId2 = params.memberId2 || params.member_id_2
    if (memberId1 || memberId2) {
      parts.push(`${t('ai.chat.message.toolParams.memberId')}: ${memberId1 ?? '?'} ↔ ${memberId2 ?? '?'}`)
    }

    const timeStr = formatTimeParams(params)
    if (timeStr) {
      parts.push(`${t('ai.chat.message.toolParams.time')}: ${timeStr}`)
    }

    if (params.limit) {
      parts.push(t('ai.chat.message.toolParams.limit', { count: params.limit }))
    }

    return parts.join(' | ')
  }

  if (name === 'message_context' || name === 'get_message_context') {
    const ids = (params.messageIds as number[] | undefined) || (params.message_ids as number[] | undefined)
    const singleId = params.messageId || params.message_id
    const size = params.contextSize || params.context_size || 20
    if (ids && ids.length > 0) {
      return t('ai.chat.message.toolParams.contextWithMessages', { msgCount: ids.length, contextSize: size })
    }
    if (singleId) {
      return `${t('ai.chat.message.toolParams.memberId')}: ${singleId} | ${t('ai.chat.message.toolParams.context', { size })}`
    }
    return t('ai.chat.message.toolParams.context', { size })
  }

  if (name === 'member_stats' || name === 'get_member_stats') {
    return t('ai.chat.message.toolParams.topMembers', { count: params.topN || params.top_n || 10 })
  }

  if (name === 'time_stats' || name === 'get_time_stats') {
    const typeKey = (params.type || params.dimension) as string
    if (typeKey) {
      return t(`ai.chat.message.toolParams.timeStats.${typeKey}`) || String(typeKey)
    }
    return ''
  }

  if (name === 'member_list' || name === 'get_group_members') {
    if (params.search) {
      return `${t('ai.chat.message.toolParams.search')}: ${params.search}`
    }
    return t('ai.chat.message.toolParams.getMemberList')
  }

  if (name === 'nickname_history' || name === 'get_member_name_history') {
    return `${t('ai.chat.message.toolParams.memberId')}: ${params.memberId || params.member_id || '?'}`
  }

  if (name === 'search_sessions') {
    return t('ai.chat.message.toolParams.limit', { count: params.limit || 20 })
  }

  if (name === 'get_session_messages' || name === 'get_session_summary') {
    return `${t('ai.chat.message.toolParams.memberId')}: ${params.chatSessionId || params.chat_session_id || '?'}`
  }

  return ''
}
</script>

<template>
  <div class="flex items-start gap-3" :class="[isUser ? 'flex-row-reverse' : '']">
    <!-- English UI note -->
    <div v-if="isUser" class="h-8 w-8 shrink-0 overflow-hidden rounded-full">
      <img :src="userAvatar" :alt="t('ai.chat.message.userAvatar')" class="h-full w-full object-cover" />
    </div>
    <div
      v-else
      class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-linear-to-br from-cyan-500 to-teal-500"
    >
      <UIcon name="i-heroicons-sparkles" class="h-4 w-4 text-white" />
    </div>

    <!-- English UI note -->
    <div class="max-w-[80%] min-w-0">
      <!-- English UI note -->
      <template v-if="isUser">
        <div class="rounded-2xl rounded-tr-sm bg-blue-500 px-4 py-3 text-white">
          <div class="prose prose-sm prose-invert max-w-none leading-relaxed" v-html="renderedContent" />
        </div>
      </template>

      <!-- English UI note -->
      <template v-else-if="useBlocksRendering">
        <div class="space-y-3">
          <template v-for="(block, idx) in visibleBlocks" :key="idx">
            <!-- English UI note -->
            <div
              v-if="block.type === 'text'"
              class="rounded-2xl rounded-tl-sm bg-gray-100 px-4 py-3 text-gray-900 dark:bg-gray-800 dark:text-gray-100"
            >
              <div
                class="prose prose-sm dark:prose-invert max-w-none leading-relaxed"
                v-html="renderMarkdown(block.text)"
              />
              <!-- English UI note -->
              <span
                v-if="isStreaming && idx === visibleBlocks.length - 1"
                class="ml-1 inline-block h-4 w-1.5 animate-pulse rounded-sm bg-cyan-500"
              />
            </div>

            <!-- English UI note -->
            <details
              v-else-if="block.type === 'think'"
              class="rounded-2xl px-2 py-1 text-xs text-gray-600 dark:text-gray-400"
            >
              <summary class="cursor-pointer select-none text-xs font-medium text-gray-500 dark:text-gray-400">
                {{ getThinkLabel(block.tag) }}
                <span v-if="block.durationMs" class="ml-2 text-xs text-gray-400 dark:text-gray-500">
                  {{ formatThinkDuration(block.durationMs) }}
                </span>
                <span
                  v-if="isStreaming && idx === visibleBlocks.length - 1"
                  class="ml-2 inline-flex items-center gap-1 text-[11px] text-gray-400 dark:text-gray-500"
                >
                  <span>{{ t('ai.chat.message.think.loading') }}</span>
                  <span class="flex gap-0.5">
                    <span class="h-1 w-1 animate-bounce rounded-full bg-gray-400 [animation-delay:0ms]" />
                    <span class="h-1 w-1 animate-bounce rounded-full bg-gray-400 [animation-delay:150ms]" />
                    <span class="h-1 w-1 animate-bounce rounded-full bg-gray-400 [animation-delay:300ms]" />
                  </span>
                </span>
              </summary>
              <div class="mt-2 prose prose-sm dark:prose-invert max-w-none leading-relaxed text-xs">
                <div v-html="renderMarkdown(block.text)" />
              </div>
            </details>

            <!-- English UI note -->
            <div
              v-else-if="block.type === 'tool'"
              class="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm"
              :class="[
                block.tool.status === 'running'
                  ? 'border-pink-200 bg-pink-50 dark:border-pink-800/50 dark:bg-pink-900/20'
                  : block.tool.status === 'done'
                    ? 'border-green-200 bg-green-50 dark:border-green-800/50 dark:bg-green-900/20'
                    : 'border-red-200 bg-red-50 dark:border-red-800/50 dark:bg-red-900/20',
              ]"
            >
              <!-- English UI note -->
              <UIcon
                :name="
                  block.tool.status === 'running'
                    ? 'i-heroicons-arrow-path'
                    : block.tool.status === 'done'
                      ? 'i-heroicons-check-circle'
                      : 'i-heroicons-x-circle'
                "
                class="h-4 w-4 shrink-0"
                :class="[
                  block.tool.status === 'running'
                    ? 'animate-spin text-pink-500'
                    : block.tool.status === 'done'
                      ? 'text-green-500'
                      : 'text-red-500',
                ]"
              />
              <!-- English UI note -->
              <div class="min-w-0 flex-1">
                <!-- English UI note -->
                <span class="text-xs text-gray-400 dark:text-gray-500 mr-1">{{ t('ai.chat.message.calling') }}</span>
                <span class="font-medium text-gray-700 dark:text-gray-300">
                  {{
                    te(`ai.chat.message.tools.${block.tool.name}`)
                      ? t(`ai.chat.message.tools.${block.tool.name}`)
                      : block.tool.displayName
                  }}
                </span>
                <span v-if="formatToolParams(block.tool)" class="ml-2 text-xs text-gray-500 dark:text-gray-400">
                  {{ formatToolParams(block.tool) }}
                </span>
              </div>
            </div>
          </template>

          <!-- English UI note -->
          <div
            v-if="isStreaming && visibleBlocks.length > 0 && visibleBlocks[visibleBlocks.length - 1].type === 'tool'"
            class="flex items-center gap-2 rounded-lg bg-gray-100 px-3 py-2 text-sm text-gray-600 dark:bg-gray-800 dark:text-gray-400"
          >
            <span class="flex gap-1">
              <span class="h-1.5 w-1.5 animate-bounce rounded-full bg-pink-500 [animation-delay:0ms]" />
              <span class="h-1.5 w-1.5 animate-bounce rounded-full bg-pink-500 [animation-delay:150ms]" />
              <span class="h-1.5 w-1.5 animate-bounce rounded-full bg-pink-500 [animation-delay:300ms]" />
            </span>
            <span>{{ t('ai.chat.message.generating') }}</span>
          </div>
        </div>
      </template>

      <!-- English UI note -->
      <template v-else>
        <div class="rounded-2xl rounded-tl-sm bg-gray-100 px-4 py-3 text-gray-900 dark:bg-gray-800 dark:text-gray-100">
          <div class="prose prose-sm dark:prose-invert max-w-none leading-relaxed" v-html="renderedContent" />
          <span v-if="isStreaming" class="ml-1 inline-block h-4 w-1.5 animate-pulse rounded-sm bg-pink-500" />
        </div>
      </template>

      <!-- English UI note -->
      <div class="mt-1 flex items-center gap-2 px-1" :class="[isUser ? 'flex-row-reverse' : '']">
        <span class="text-xs text-gray-400">{{ formattedTime }}</span>
        <!-- English UI note -->
        <CaptureButton
          v-if="showCaptureButton && !isUser && !isStreaming"
          size="xs"
          type="element"
          target-selector=".qa-pair"
        />
      </div>
    </div>
  </div>
</template>

<!-- English UI note -->
