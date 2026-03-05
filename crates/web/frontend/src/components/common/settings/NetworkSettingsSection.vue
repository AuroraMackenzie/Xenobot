<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

// English engineering note.
type ProxyMode = 'off' | 'system' | 'manual'
type McpCallProtocol = 'rpc' | 'http'

// English engineering note.
const proxyMode = ref<ProxyMode>('system')
const proxyUrl = ref('')
const proxyUrlError = ref('')
const isSavingProxy = ref(false)
const isTestingProxy = ref(false)
const proxyTestResult = ref<{ success: boolean; message: string } | null>(null)
const mcpBaseUrl = ref('http://127.0.0.1:8081')
const isCheckingMcp = ref(false)
const mcpCheckResult = ref<{
  success: boolean
  httpStatus: number
  httpToolCount: number
  rpcToolCount: number
  message: string
} | null>(null)
const mcpCallProtocol = ref<McpCallProtocol>('rpc')
const mcpToolName = ref('get_current_time')
const mcpToolArgsText = ref('{}')
const mcpSessionId = ref('')
const isRunningMcpTool = ref(false)
const mcpToolResultText = ref('')
const mcpToolError = ref('')
const mcpQuickTools = [
  'get_current_time',
  'query_contacts',
  'query_groups',
  'recent_sessions',
  'chat_records',
]

// English engineering note.
const proxyModeOptions = computed(() => [
  { label: t('settings.basic.network.modeOff'), value: 'off' },
  { label: t('settings.basic.network.modeSystem'), value: 'system' },
  { label: t('settings.basic.network.modeManual'), value: 'manual' },
])

// English engineering note.
async function loadProxyConfig() {
  try {
    const config = await window.networkApi.getProxyConfig()
    proxyMode.value = config.mode || 'system'
    proxyUrl.value = config.url || ''
  } catch (error) {
    console.error('获取代理配置失败:', error)
  }
}

// English engineering note.
function validateProxyUrl(url: string): boolean {
  if (!url) {
    proxyUrlError.value = ''
    return true
  }

  try {
    const parsed = new URL(url)
    if (!['http:', 'https:'].includes(parsed.protocol)) {
      proxyUrlError.value = t('settings.basic.network.onlyHttpSupported')
      return false
    }
    proxyUrlError.value = ''
    return true
  } catch {
    proxyUrlError.value = t('settings.basic.network.invalidProxyUrl')
    return false
  }
}

// English engineering note.
async function saveProxyConfig() {
  // English engineering note.
  proxyTestResult.value = null

  // English engineering note.
  if (proxyMode.value === 'manual' && !proxyUrl.value.trim()) {
    proxyUrlError.value = t('settings.basic.network.enterProxyFirst')
    return
  }

  // English engineering note.
  if (proxyMode.value === 'manual' && !validateProxyUrl(proxyUrl.value)) {
    return
  }

  isSavingProxy.value = true
  try {
    const result = await window.networkApi.saveProxyConfig({
      mode: proxyMode.value,
      url: proxyUrl.value.trim(),
    })

    if (!result.success) {
      proxyUrlError.value = result.error || t('settings.basic.network.saveFailed')
    }
  } catch (error) {
    console.error('保存代理配置失败:', error)
    proxyUrlError.value = t('settings.basic.network.saveFailed')
  } finally {
    isSavingProxy.value = false
  }
}

// English engineering note.
async function handleProxyModeChange(mode: string | number) {
  const newMode = mode as ProxyMode
  proxyMode.value = newMode
  proxyTestResult.value = null
  proxyUrlError.value = ''

  // English engineering note.
  if (newMode !== 'manual') {
    await saveProxyConfig()
  }
}

// English engineering note.
function handleProxyUrlInput() {
  proxyTestResult.value = null
  if (proxyUrl.value) {
    validateProxyUrl(proxyUrl.value)
  } else {
    proxyUrlError.value = ''
  }
}

// English engineering note.
async function handleProxyUrlBlur() {
  if (proxyMode.value === 'manual' && proxyUrl.value.trim()) {
    await saveProxyConfig()
  }
}

// English engineering note.
async function testProxyConnection() {
  if (!proxyUrl.value.trim()) {
    proxyUrlError.value = t('settings.basic.network.enterProxyFirst')
    return
  }

  if (!validateProxyUrl(proxyUrl.value)) {
    return
  }

  isTestingProxy.value = true
  proxyTestResult.value = null

  try {
    const result = await window.networkApi.testProxyConnection(proxyUrl.value.trim())
    proxyTestResult.value = {
      success: result.success,
      message: result.success
        ? t('settings.basic.network.connectionSuccess')
        : result.error || t('settings.basic.network.connectionFailed'),
    }
  } catch (error) {
    proxyTestResult.value = {
      success: false,
      message:
        t('settings.basic.network.connectionFailed') + ': ' + (error instanceof Error ? error.message : String(error)),
    }
  } finally {
    isTestingProxy.value = false
  }
}

function normalizeEndpointBase(raw: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return ''
  return trimmed.replace(/\/+$/, '')
}

async function checkMcpConnectivity() {
  isCheckingMcp.value = true
  mcpCheckResult.value = null
  const baseUrl = normalizeEndpointBase(mcpBaseUrl.value)

  try {
    const mcpApi = window.mcpApi
    if (!mcpApi) {
      mcpCheckResult.value = {
        success: false,
        httpStatus: 0,
        httpToolCount: 0,
        rpcToolCount: 0,
        message: 'MCP bridge is not available in this frontend runtime.',
      }
      return
    }

    const health = await mcpApi.health(baseUrl)
    const httpTools = await mcpApi.listTools(baseUrl)
    const rpcInit = await mcpApi.initialize(baseUrl)
    const rpcTools = await mcpApi.listToolsRpc(baseUrl)

    const httpToolCount = Array.isArray(httpTools.tools) ? httpTools.tools.length : 0
    const rpcToolsResult = (rpcTools.result || {}) as { tools?: unknown[] }
    const rpcToolCount = Array.isArray(rpcToolsResult.tools) ? rpcToolsResult.tools.length : 0

    const success = health.success && httpTools.success && rpcInit.success && rpcTools.success
    const status = Number(httpTools.status || health.status || 0)
    const detail = success
      ? 'MCP HTTP + JSON-RPC contracts are reachable.'
      : `MCP check failed (health=${health.status}, tools=${httpTools.status}).`

    mcpCheckResult.value = {
      success,
      httpStatus: status,
      httpToolCount,
      rpcToolCount,
      message: detail,
    }
  } catch (error) {
    mcpCheckResult.value = {
      success: false,
      httpStatus: 0,
      httpToolCount: 0,
      rpcToolCount: 0,
      message: error instanceof Error ? error.message : String(error),
    }
  } finally {
    isCheckingMcp.value = false
  }
}

function prettyJson(payload: unknown): string {
  try {
    return JSON.stringify(payload, null, 2)
  } catch {
    return String(payload)
  }
}

function parseToolArgs(raw: string): Record<string, unknown> {
  const text = raw.trim()
  if (!text) return {}
  const parsed = JSON.parse(text)
  if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') {
    throw new Error('Tool arguments must be a JSON object.')
  }
  return parsed as Record<string, unknown>
}

function buildToolArgs(): Record<string, unknown> {
  const args = parseToolArgs(mcpToolArgsText.value)
  const toolName = mcpToolName.value.trim()
  if (toolName === 'chat_records' && mcpSessionId.value.trim() && !('session_id' in args)) {
    args.session_id = Number(mcpSessionId.value) || mcpSessionId.value.trim()
  }
  return args
}

function selectQuickTool(toolName: string) {
  mcpToolName.value = toolName
  if (toolName === 'chat_records' && !mcpToolArgsText.value.trim()) {
    mcpToolArgsText.value = '{}'
  }
}

function handleMcpCallProtocolChange(value: string | number) {
  mcpCallProtocol.value = String(value) === 'http' ? 'http' : 'rpc'
}

async function runMcpTool() {
  isRunningMcpTool.value = true
  mcpToolError.value = ''
  mcpToolResultText.value = ''
  const baseUrl = normalizeEndpointBase(mcpBaseUrl.value)
  const toolName = mcpToolName.value.trim()

  try {
    if (!toolName) {
      throw new Error('Tool name is required.')
    }
    const mcpApi = window.mcpApi
    if (!mcpApi) {
      throw new Error('MCP bridge is not available in this frontend runtime.')
    }

    const args = buildToolArgs()
    const result =
      mcpCallProtocol.value === 'rpc'
        ? await mcpApi.callToolRpc(toolName, args, baseUrl)
        : await mcpApi.callToolHttp(toolName, args, baseUrl)

    mcpToolResultText.value = prettyJson(result)
  } catch (error) {
    mcpToolError.value = error instanceof Error ? error.message : String(error)
  } finally {
    isRunningMcpTool.value = false
  }
}

// English engineering note.
onMounted(() => {
  loadProxyConfig()
})
</script>

<template>
  <div>
    <h3 class="mb-3 flex items-center gap-2 text-sm font-semibold text-gray-900 dark:text-white">
      <UIcon name="i-heroicons-globe-alt" class="h-4 w-4 text-cyan-500" />
      {{ t('settings.basic.network.title') }}
    </h3>
    <div class="rounded-lg border border-gray-200 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800/50">
      <!-- English UI note -->
      <div class="flex items-center justify-between">
        <div class="flex-1 pr-4">
          <p class="text-sm font-medium text-gray-900 dark:text-white">{{ t('settings.basic.network.proxyMode') }}</p>
          <p class="text-xs text-gray-500 dark:text-gray-400">{{ t('settings.basic.network.proxyModeDesc') }}</p>
        </div>
        <div class="w-64">
          <UTabs
            :model-value="proxyMode"
            size="sm"
            class="gap-0"
            :items="proxyModeOptions"
            @update:model-value="handleProxyModeChange"
          />
        </div>
      </div>

      <!-- English UI note -->
      <div v-if="proxyMode === 'manual'" class="mt-4 space-y-3">
        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
            {{ t('settings.basic.network.proxyAddress') }}
          </label>
          <UInput
            v-model="proxyUrl"
            :placeholder="t('settings.basic.network.proxyPlaceholder')"
            :color="proxyUrlError ? 'error' : 'neutral'"
            size="sm"
            class="w-full"
            @input="handleProxyUrlInput"
            @blur="handleProxyUrlBlur"
          />
          <p v-if="proxyUrlError" class="mt-1 text-xs text-red-500">
            {{ proxyUrlError }}
          </p>
          <p v-else class="mt-1 text-xs text-gray-400">{{ t('settings.basic.network.proxyHelp') }}</p>
        </div>

        <!-- English UI note -->
        <div class="flex items-center gap-3">
          <UButton
            :loading="isTestingProxy"
            :disabled="isTestingProxy || !proxyUrl.trim()"
            color="neutral"
            variant="soft"
            size="sm"
            @click="testProxyConnection"
          >
            <UIcon name="i-heroicons-signal" class="mr-1 h-4 w-4" />
            {{ isTestingProxy ? t('settings.basic.network.testing') : t('settings.basic.network.testConnection') }}
          </UButton>

          <div v-if="proxyTestResult" class="flex items-center gap-1.5">
            <UIcon
              :name="proxyTestResult.success ? 'i-heroicons-check-circle' : 'i-heroicons-x-circle'"
              :class="['h-4 w-4', proxyTestResult.success ? 'text-green-500' : 'text-red-500']"
            />
            <span
              :class="[
                'text-xs',
                proxyTestResult.success ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400',
              ]"
            >
              {{ proxyTestResult.message }}
            </span>
          </div>
        </div>
      </div>
    </div>

    <div class="mt-3 rounded-lg border border-gray-200 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800/50">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium text-gray-900 dark:text-white">MCP Connectivity Check</p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            Validate both HTTP endpoints and JSON-RPC contract from this frontend runtime.
          </p>
        </div>
        <UButton
          :loading="isCheckingMcp"
          :disabled="isCheckingMcp"
          color="neutral"
          variant="soft"
          size="sm"
          @click="checkMcpConnectivity"
        >
          <UIcon name="i-heroicons-bolt" class="mr-1 h-4 w-4" />
          {{ isCheckingMcp ? 'Checking...' : 'Check MCP' }}
        </UButton>
      </div>
      <div class="mt-3">
        <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">MCP Base URL</label>
        <UInput
          v-model="mcpBaseUrl"
          placeholder="http://127.0.0.1:8081"
          size="sm"
          class="w-full"
        />
      </div>
      <div v-if="mcpCheckResult" class="mt-3 rounded-md border border-gray-200 bg-white p-3 text-xs dark:border-gray-700 dark:bg-gray-900/30">
        <div class="flex items-center gap-1.5">
          <UIcon
            :name="mcpCheckResult.success ? 'i-heroicons-check-circle' : 'i-heroicons-x-circle'"
            :class="['h-4 w-4', mcpCheckResult.success ? 'text-green-500' : 'text-red-500']"
          />
          <span :class="mcpCheckResult.success ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'">
            {{ mcpCheckResult.message }}
          </span>
        </div>
        <div class="mt-2 grid grid-cols-3 gap-2 text-gray-600 dark:text-gray-300">
          <div>HTTP status: {{ mcpCheckResult.httpStatus }}</div>
          <div>HTTP tools: {{ mcpCheckResult.httpToolCount }}</div>
          <div>RPC tools: {{ mcpCheckResult.rpcToolCount }}</div>
        </div>
      </div>
    </div>

    <div class="mt-3 rounded-lg border border-gray-200 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800/50">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium text-gray-900 dark:text-white">MCP Tool Playground</p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            Run first-batch MCP tools from frontend via JSON-RPC or HTTP for contract verification.
          </p>
        </div>
        <UButton
          :loading="isRunningMcpTool"
          :disabled="isRunningMcpTool"
          color="neutral"
          variant="soft"
          size="sm"
          @click="runMcpTool"
        >
          <UIcon name="i-heroicons-play" class="mr-1 h-4 w-4" />
          {{ isRunningMcpTool ? 'Running...' : 'Run Tool' }}
        </UButton>
      </div>

      <div class="mt-3 grid gap-3 md:grid-cols-2">
        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">Protocol</label>
          <UTabs
            :model-value="mcpCallProtocol"
            size="sm"
            class="gap-0"
            :items="[
              { label: 'JSON-RPC', value: 'rpc' },
              { label: 'HTTP', value: 'http' },
            ]"
            @update:model-value="handleMcpCallProtocolChange"
          />
        </div>
        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">Tool Name</label>
          <UInput
            v-model="mcpToolName"
            placeholder="get_current_time"
            size="sm"
            class="w-full"
          />
        </div>
      </div>

      <div class="mt-3 flex flex-wrap gap-2">
        <UButton
          v-for="tool in mcpQuickTools"
          :key="tool"
          color="neutral"
          variant="ghost"
          size="xs"
          @click="selectQuickTool(tool)"
        >
          {{ tool }}
        </UButton>
      </div>

      <div class="mt-3 grid gap-3 md:grid-cols-2">
        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
            Optional Session ID (for chat_records)
          </label>
          <UInput
            v-model="mcpSessionId"
            placeholder="1"
            size="sm"
            class="w-full"
          />
        </div>
        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">Tool Arguments (JSON)</label>
          <textarea
            v-model="mcpToolArgsText"
            rows="4"
            class="w-full rounded-md border border-gray-200 bg-white px-3 py-2 text-xs text-gray-700 focus:border-cyan-400 focus:outline-none dark:border-gray-700 dark:bg-gray-900/30 dark:text-gray-200"
            placeholder='{}'
          />
        </div>
      </div>

      <div v-if="mcpToolError" class="mt-3 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-600 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300">
        {{ mcpToolError }}
      </div>

      <div v-if="mcpToolResultText" class="mt-3 rounded-md border border-gray-200 bg-white p-3 dark:border-gray-700 dark:bg-gray-900/30">
        <p class="mb-2 text-xs font-medium text-gray-700 dark:text-gray-200">Result</p>
        <pre class="max-h-72 overflow-auto whitespace-pre-wrap break-all text-[11px] leading-relaxed text-gray-700 dark:text-gray-200">{{ mcpToolResultText }}</pre>
      </div>
    </div>
  </div>
</template>
