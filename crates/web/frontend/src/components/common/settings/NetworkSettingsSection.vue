<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

type ProxyMode = 'off' | 'system' | 'manual'
type McpCallProtocol = 'rpc' | 'http'
type SignalTone = 'neutral' | 'success' | 'warning'

type RuntimeProbe = {
  success: boolean
  checkedAt: string
  statusCode: number
  message: string
  service: string
  version: string
  bindAddr: string
  apiBasePath: string
  corsEnabled: boolean
  requestTimeoutSeconds: number
  maxBodySizeBytes: number
  runtimeOs: string
  runtimeArch: string
  endpoints: string[]
  features: Record<string, boolean>
  healthBody: string
}

type SandboxDoctorProbe = {
  success: boolean
  checkedAt: string
  recommendedMode: string
  recommendedCommand: string
  tcpAllowed: boolean
  tcpError: string
  udsSupported: boolean
  udsAllowed: boolean
  udsPath: string
  udsError: string
  fileGatewayDir: string
  fileGatewayWritable: boolean
  fileGatewayError: string
}

const proxyMode = ref<ProxyMode>('system')
const proxyUrl = ref('')
const proxyUrlError = ref('')
const isSavingProxy = ref(false)
const isTestingProxy = ref(false)
const proxyTestResult = ref<{ success: boolean; message: string } | null>(null)

const runtimeProbe = ref<RuntimeProbe | null>(null)
const isRefreshingRuntime = ref(false)
const sandboxProbe = ref<SandboxDoctorProbe | null>(null)
const isRefreshingSandbox = ref(false)
const sandboxGatewayDir = ref('')

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

const integrationTargets = ref<Array<{ id: string; name: string; description: string }>>([])
const selectedIntegrationTarget = ref('pencil')
const isLoadingIntegrationCatalog = ref(false)
const isLoadingIntegrationPreset = ref(false)
const integrationPresetError = ref('')
const integrationPresetText = ref('')

const proxyModeOptions = computed(() => [
  { label: t('settings.basic.network.modeOff'), value: 'off' },
  { label: t('settings.basic.network.modeSystem'), value: 'system' },
  { label: t('settings.basic.network.modeManual'), value: 'manual' },
])

const proxyModeLabel = computed(() => {
  return proxyModeOptions.value.find((item) => item.value === proxyMode.value)?.label ?? proxyMode.value
})

const enabledFeatureCount = computed(() => {
  return Object.values(runtimeProbe.value?.features ?? {}).filter(Boolean).length
})

const publishedEndpointCount = computed(() => runtimeProbe.value?.endpoints.length ?? 0)

const runtimeFeatureEntries = computed(() => {
  return Object.entries(runtimeProbe.value?.features ?? {})
    .filter(([, enabled]) => enabled)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key]) => ({
      key,
      label: formatFeatureLabel(key),
    }))
})

const integrationTargetOptions = computed(() => {
  return integrationTargets.value.map((item) => ({
    label: item.name,
    value: item.id,
  }))
})

const runtimeMetaRows = computed(() => {
  if (!runtimeProbe.value) {
    return []
  }

  return [
    {
      label: t('settings.basic.network.localApiTarget'),
      value: '/api',
    },
    {
      label: t('settings.basic.network.bindAddress'),
      value: runtimeProbe.value.bindAddr || t('settings.basic.network.unknownValue'),
    },
    {
      label: t('settings.basic.network.apiBasePath'),
      value: runtimeProbe.value.apiBasePath || '/',
    },
    {
      label: t('settings.basic.network.requestTimeout'),
      value:
        runtimeProbe.value.requestTimeoutSeconds > 0
          ? `${runtimeProbe.value.requestTimeoutSeconds}s`
          : t('settings.basic.network.unknownValue'),
    },
    {
      label: t('settings.basic.network.maxBodySize'),
      value: formatBytes(runtimeProbe.value.maxBodySizeBytes),
    },
    {
      label: t('settings.basic.network.operatingSystem'),
      value: runtimeProbe.value.runtimeOs || t('settings.basic.network.unknownValue'),
    },
    {
      label: t('settings.basic.network.architecture'),
      value: runtimeProbe.value.runtimeArch || t('settings.basic.network.unknownValue'),
    },
    {
      label: 'CORS',
      value: runtimeProbe.value.corsEnabled ? 'Enabled' : 'Disabled',
    },
  ]
})

const signalCards = computed(() => {
  const runtimeTone: SignalTone = runtimeProbe.value
    ? runtimeProbe.value.success
      ? 'success'
      : 'warning'
    : 'neutral'
  const runtimeValue = runtimeProbe.value
    ? runtimeProbe.value.success
      ? t('settings.basic.network.signalHealthy')
      : t('settings.basic.network.signalAttention')
    : t('settings.basic.network.signalUnchecked')
  const runtimeDetail = runtimeProbe.value
    ? `${runtimeProbe.value.service || 'xenobot-api'} · ${
        runtimeProbe.value.runtimeOs || t('settings.basic.network.unknownValue')
      }/${runtimeProbe.value.runtimeArch || t('settings.basic.network.unknownValue')}`
    : t('settings.basic.network.runtimeDesc')

  const proxyTone: SignalTone = proxyTestResult.value
    ? proxyTestResult.value.success
      ? 'success'
      : 'warning'
    : 'neutral'
  const proxyDetail = proxyTestResult.value
    ? proxyTestResult.value.message
    : proxyMode.value === 'manual'
      ? proxyUrl.value.trim() || t('settings.basic.network.proxyHelp')
      : t('settings.basic.network.proxyModeDesc')

  const mcpTone: SignalTone = mcpCheckResult.value
    ? mcpCheckResult.value.success
      ? 'success'
      : 'warning'
    : 'neutral'
  const mcpValue = mcpCheckResult.value
    ? mcpCheckResult.value.success
      ? t('settings.basic.network.signalHealthy')
      : t('settings.basic.network.signalAttention')
    : t('settings.basic.network.signalUnchecked')
  const mcpDetail = mcpCheckResult.value
    ? `${mcpCheckResult.value.httpToolCount} HTTP · ${mcpCheckResult.value.rpcToolCount} RPC`
    : t('settings.basic.network.mcpDesc')

  const modulesTone: SignalTone = runtimeProbe.value ? 'success' : 'neutral'
  const modulesDetail = runtimeProbe.value
    ? t('settings.basic.network.routesPublished', { count: publishedEndpointCount.value })
    : t('settings.basic.network.runtimeHint')

  const sandboxTone: SignalTone = sandboxProbe.value
    ? sandboxProbe.value.recommendedMode === 'file-gateway'
      ? 'warning'
      : 'success'
    : 'neutral'
  const sandboxValue = sandboxProbe.value
    ? sandboxProbe.value.recommendedMode
    : t('settings.basic.network.signalUnchecked')
  const sandboxDetail = sandboxProbe.value
    ? sandboxProbe.value.fileGatewayWritable
      ? t('settings.basic.network.sandboxWritableReady')
      : sandboxProbe.value.fileGatewayError || t('settings.basic.network.sandboxRequiresReview')
    : t('settings.basic.network.sandboxDesc')

  return [
    {
      key: 'runtime',
      icon: 'i-heroicons-server-stack',
      label: t('settings.basic.network.signalRuntime'),
      value: runtimeValue,
      detail: runtimeDetail,
      tone: runtimeTone,
    },
    {
      key: 'proxy',
      icon: 'i-heroicons-globe-alt',
      label: t('settings.basic.network.signalProxy'),
      value: proxyModeLabel.value,
      detail: proxyDetail,
      tone: proxyTone,
    },
    {
      key: 'mcp',
      icon: 'i-heroicons-bolt',
      label: t('settings.basic.network.signalMcp'),
      value: mcpValue,
      detail: mcpDetail,
      tone: mcpTone,
    },
    {
      key: 'modules',
      icon: 'i-heroicons-squares-2x2',
      label: t('settings.basic.network.signalModules'),
      value: String(enabledFeatureCount.value),
      detail: modulesDetail,
      tone: modulesTone,
    },
    {
      key: 'sandbox',
      icon: 'i-heroicons-shield-check',
      label: t('settings.basic.network.signalSandbox'),
      value: sandboxValue,
      detail: sandboxDetail,
      tone: sandboxTone,
    },
  ]
})

function normalizeProxyMode(mode: string): ProxyMode {
  if (mode === 'off' || mode === 'manual') {
    return mode
  }
  return 'system'
}

async function loadProxyConfig() {
  try {
    const config = await window.networkApi.getProxyConfig()
    proxyMode.value = normalizeProxyMode(String(config.mode || 'system'))
    proxyUrl.value = config.url || ''
  } catch (error) {
    console.error('Failed to load proxy configuration:', error)
  }
}

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

async function saveProxyConfig() {
  proxyTestResult.value = null

  if (proxyMode.value === 'manual' && !proxyUrl.value.trim()) {
    proxyUrlError.value = t('settings.basic.network.enterProxyFirst')
    return
  }

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
    console.error('Failed to save proxy configuration:', error)
    proxyUrlError.value = t('settings.basic.network.saveFailed')
  } finally {
    isSavingProxy.value = false
  }
}

async function handleProxyModeChange(mode: string | number) {
  proxyMode.value = normalizeProxyMode(String(mode))
  proxyTestResult.value = null
  proxyUrlError.value = ''

  if (proxyMode.value !== 'manual') {
    await saveProxyConfig()
  }
}

function handleProxyUrlInput() {
  proxyTestResult.value = null
  if (proxyUrl.value) {
    validateProxyUrl(proxyUrl.value)
  } else {
    proxyUrlError.value = ''
  }
}

async function handleProxyUrlBlur() {
  if (proxyMode.value === 'manual' && proxyUrl.value.trim()) {
    await saveProxyConfig()
  }
}

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
        t('settings.basic.network.connectionFailed') +
        ': ' +
        (error instanceof Error ? error.message : String(error)),
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

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {}
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback
}

function asNumber(value: unknown, fallback = 0): number {
  const number = Number(value)
  return Number.isFinite(number) ? number : fallback
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.map((item) => String(item)) : []
}

function asBooleanRecord(value: unknown): Record<string, boolean> {
  return Object.fromEntries(
    Object.entries(asRecord(value)).map(([key, flag]) => [key, Boolean(flag)])
  )
}

function formatFeatureLabel(feature: string): string {
  return String(feature)
    .replace(/[_-]+/g, ' ')
    .replace(/\b\w/g, (token) => token.toUpperCase())
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B'
  }

  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unitIndex = 0

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }

  const digits = value >= 100 || unitIndex === 0 ? 0 : 1
  return `${value.toFixed(digits)} ${units[unitIndex]}`
}

async function refreshRuntimeSurface() {
  isRefreshingRuntime.value = true

  try {
    const [health, status, index] = await Promise.all([
      window.networkApi.getRuntimeHealth(),
      window.networkApi.getRuntimeStatus(),
      window.networkApi.getServiceIndex(),
    ])

    const statusBody = asRecord(status.body)
    const indexBody = asRecord(index.body)
    const runtimeBody = asRecord(statusBody.runtime)
    const featureBody = asBooleanRecord(statusBody.features)
    const endpoints = asStringArray(indexBody.endpoints)
    const service = asString(statusBody.service, asString(indexBody.service, 'xenobot-api'))
    const version = asString(statusBody.version)
    const bindAddr = asString(statusBody.bindAddr)
    const apiBasePath = asString(statusBody.apiBasePath)
    const runtimeOs = asString(runtimeBody.os)
    const runtimeArch = asString(runtimeBody.arch)
    const healthBody =
      typeof health.body === 'string' ? health.body : JSON.stringify(health.body ?? {}, null, 2)

    const success = health.success && status.success && index.success
    const partiallyAvailable = health.success || status.success || index.success

    runtimeProbe.value = {
      success,
      checkedAt: new Date().toLocaleString(),
      statusCode: status.status || index.status || health.status,
      message: success
        ? t('settings.basic.network.runtimeReady')
        : partiallyAvailable
          ? t('settings.basic.network.runtimeDegraded')
          : t('settings.basic.network.runtimeUnavailable'),
      service,
      version,
      bindAddr,
      apiBasePath,
      corsEnabled: Boolean(statusBody.corsEnabled),
      requestTimeoutSeconds: asNumber(statusBody.requestTimeoutSeconds),
      maxBodySizeBytes: asNumber(statusBody.maxBodySizeBytes),
      runtimeOs,
      runtimeArch,
      endpoints,
      features: featureBody,
      healthBody,
    }
  } catch (error) {
    runtimeProbe.value = {
      success: false,
      checkedAt: new Date().toLocaleString(),
      statusCode: 0,
      message: error instanceof Error ? error.message : String(error),
      service: 'xenobot-api',
      version: '',
      bindAddr: '',
      apiBasePath: '',
      corsEnabled: false,
      requestTimeoutSeconds: 0,
      maxBodySizeBytes: 0,
      runtimeOs: '',
      runtimeArch: '',
      endpoints: [],
      features: {},
      healthBody: '',
    }
  } finally {
    isRefreshingRuntime.value = false
  }
}

function boolToken(value: boolean, positiveKey: string, negativeKey: string): string {
  return value ? t(positiveKey) : t(negativeKey)
}

async function refreshSandboxDoctor() {
  isRefreshingSandbox.value = true

  try {
    const result = await window.networkApi.getSandboxDoctor(sandboxGatewayDir.value.trim())
    const report = asRecord(result.report)
    const tcp = asRecord(report.tcp)
    const uds = asRecord(report.uds)
    const fileGateway = asRecord(report.fileGateway)
    const recommended = asRecord(report.recommended)

    sandboxProbe.value = {
      success: result.success,
      checkedAt: new Date().toLocaleString(),
      recommendedMode: asString(recommended.mode, 'unknown'),
      recommendedCommand: asString(recommended.command),
      tcpAllowed: Boolean(tcp.allowed),
      tcpError: asString(tcp.error),
      udsSupported: Boolean(uds.supported),
      udsAllowed: Boolean(uds.allowed),
      udsPath: asString(uds.path),
      udsError: asString(uds.error),
      fileGatewayDir: asString(fileGateway.dir),
      fileGatewayWritable: Boolean(fileGateway.writable),
      fileGatewayError: asString(fileGateway.error),
    }
  } catch (error) {
    sandboxProbe.value = {
      success: false,
      checkedAt: new Date().toLocaleString(),
      recommendedMode: 'unknown',
      recommendedCommand: '',
      tcpAllowed: false,
      tcpError: error instanceof Error ? error.message : String(error),
      udsSupported: false,
      udsAllowed: false,
      udsPath: '',
      udsError: '',
      fileGatewayDir: sandboxGatewayDir.value.trim(),
      fileGatewayWritable: false,
      fileGatewayError: '',
    }
  } finally {
    isRefreshingSandbox.value = false
  }
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
        message: t('settings.basic.network.mcpRuntimeMissing'),
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
    const statusCode = Number(httpTools.status || health.status || 0)

    mcpCheckResult.value = {
      success,
      httpStatus: statusCode,
      httpToolCount,
      rpcToolCount,
      message: success
        ? t('settings.basic.network.mcpHealthyMessage')
        : `${t('settings.basic.network.mcpFailedMessage')} (health=${health.status}, tools=${httpTools.status}).`,
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

async function loadMcpIntegrationCatalog() {
  isLoadingIntegrationCatalog.value = true
  integrationPresetError.value = ''

  try {
    const mcpApi = window.mcpApi
    if (!mcpApi) {
      integrationPresetError.value = t('settings.basic.network.mcpRuntimeMissing')
      return
    }

    const baseUrl = normalizeEndpointBase(mcpBaseUrl.value)
    const result = await mcpApi.listIntegrations(baseUrl)
    const rawItems = Array.isArray(result.integrations) ? result.integrations : []
    integrationTargets.value = rawItems.map((item) => {
      const row = asRecord(item)
      return {
        id: asString(row.id),
        name: asString(row.name, asString(row.id)),
        description: asString(row.description),
      }
    })

    if (
      integrationTargets.value.length > 0 &&
      !integrationTargets.value.some((item) => item.id === selectedIntegrationTarget.value)
    ) {
      selectedIntegrationTarget.value = integrationTargets.value[0]?.id || 'pencil'
    }
  } catch (error) {
    integrationPresetError.value = error instanceof Error ? error.message : String(error)
  } finally {
    isLoadingIntegrationCatalog.value = false
  }
}

function handleIntegrationTargetChange(value: string | number) {
  selectedIntegrationTarget.value = String(value || 'pencil')
  integrationPresetError.value = ''
}

async function loadSelectedIntegrationPreset() {
  isLoadingIntegrationPreset.value = true
  integrationPresetError.value = ''
  integrationPresetText.value = ''

  try {
    const mcpApi = window.mcpApi
    if (!mcpApi) {
      throw new Error(t('settings.basic.network.mcpRuntimeMissing'))
    }
    const baseUrl = normalizeEndpointBase(mcpBaseUrl.value)
    const result = await mcpApi.getIntegrationPreset(selectedIntegrationTarget.value, baseUrl)
    if (!result.success) {
      throw new Error(`Preset request failed with status ${result.status}`)
    }
    integrationPresetText.value = prettyJson(result.preset)
  } catch (error) {
    integrationPresetError.value = error instanceof Error ? error.message : String(error)
  } finally {
    isLoadingIntegrationPreset.value = false
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
      throw new Error(t('settings.basic.network.mcpRuntimeMissing'))
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

onMounted(() => {
  void loadProxyConfig()
  void refreshRuntimeSurface()
  void refreshSandboxDoctor()
  void loadMcpIntegrationCatalog()
})
</script>

<template>
  <div class="network-surface space-y-3">
    <div class="flex items-center gap-2">
      <UIcon name="i-heroicons-globe-alt" class="h-4 w-4 text-cyan-500" />
      <div>
        <h3 class="text-sm font-semibold text-gray-900 dark:text-white">
          {{ t('settings.basic.network.title') }}
        </h3>
        <p class="text-xs text-gray-500 dark:text-gray-400">
          {{ t('settings.basic.network.surfaceDesc') }}
        </p>
      </div>
    </div>

    <div class="signal-grid">
      <article
        v-for="card in signalCards"
        :key="card.key"
        class="signal-card"
        :class="[
          card.tone === 'success'
            ? 'signal-card--success'
            : card.tone === 'warning'
              ? 'signal-card--warning'
              : 'signal-card--neutral',
        ]"
      >
        <div class="signal-icon">
          <UIcon :name="card.icon" class="h-4 w-4" />
        </div>
        <div class="min-w-0">
          <p class="signal-label">{{ card.label }}</p>
          <p class="signal-value">{{ card.value }}</p>
          <p class="signal-detail">{{ card.detail }}</p>
        </div>
      </article>
    </div>

    <section class="xeno-panel rounded-2xl border border-gray-200/80 p-4 dark:border-gray-700/80">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium text-gray-900 dark:text-white">
            {{ t('settings.basic.network.proxyMode') }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            {{ t('settings.basic.network.proxyModeDesc') }}
          </p>
        </div>
        <div class="w-full max-w-xs">
          <UTabs
            :model-value="proxyMode"
            size="sm"
            class="gap-0"
            :items="proxyModeOptions"
            @update:model-value="handleProxyModeChange"
          />
        </div>
      </div>

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
          <p v-else class="mt-1 text-xs text-gray-400">
            {{ t('settings.basic.network.proxyHelp') }}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <UButton
            :loading="isTestingProxy"
            :disabled="isTestingProxy || isSavingProxy || !proxyUrl.trim()"
            color="neutral"
            variant="soft"
            size="sm"
            @click="testProxyConnection"
          >
            <UIcon name="i-heroicons-signal" class="mr-1 h-4 w-4" />
            {{ isTestingProxy ? t('settings.basic.network.testing') : t('settings.basic.network.testConnection') }}
          </UButton>

          <Transition name="xeno-fade">
            <div v-if="proxyTestResult" class="status-note" :class="proxyTestResult.success ? 'status-note--success' : 'status-note--warning'">
              <UIcon
                :name="proxyTestResult.success ? 'i-heroicons-check-circle' : 'i-heroicons-exclamation-triangle'"
                class="h-4 w-4"
              />
              <span>{{ proxyTestResult.message }}</span>
            </div>
          </Transition>
        </div>
      </div>
    </section>

    <section class="xeno-panel rounded-2xl border border-gray-200/80 p-4 dark:border-gray-700/80">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium text-gray-900 dark:text-white">
            {{ t('settings.basic.network.runtimeTitle') }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            {{ t('settings.basic.network.runtimeDesc') }}
          </p>
        </div>
        <UButton
          :loading="isRefreshingRuntime"
          :disabled="isRefreshingRuntime"
          color="neutral"
          variant="soft"
          size="sm"
          @click="refreshRuntimeSurface"
        >
          <UIcon name="i-heroicons-arrow-path" class="mr-1 h-4 w-4" />
          {{
            isRefreshingRuntime
              ? t('settings.basic.network.refreshing')
              : t('settings.basic.network.refreshRuntime')
          }}
        </UButton>
      </div>

      <Transition name="xeno-fade">
        <div
          v-if="runtimeProbe"
          class="status-note mt-3"
          :class="runtimeProbe.success ? 'status-note--success' : 'status-note--warning'"
        >
          <UIcon
            :name="runtimeProbe.success ? 'i-heroicons-check-circle' : 'i-heroicons-exclamation-triangle'"
            class="h-4 w-4"
          />
          <span>{{ runtimeProbe.message }}</span>
          <span class="status-note__meta">
            {{ t('settings.basic.network.lastChecked') }} {{ runtimeProbe.checkedAt }}
          </span>
        </div>
      </Transition>

      <div class="mt-3 grid gap-3 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <div class="meta-grid">
          <div class="meta-cell">
            <p class="meta-label">Service</p>
            <p class="meta-value">{{ runtimeProbe?.service || 'xenobot-api' }}</p>
          </div>
          <div class="meta-cell">
            <p class="meta-label">Version</p>
            <p class="meta-value">{{ runtimeProbe?.version || t('settings.basic.network.unknownValue') }}</p>
          </div>
          <div
            v-for="row in runtimeMetaRows"
            :key="row.label"
            class="meta-cell"
          >
            <p class="meta-label">{{ row.label }}</p>
            <p class="meta-value">{{ row.value }}</p>
          </div>
        </div>

        <div class="space-y-3">
          <div class="meta-group">
            <div class="flex items-center justify-between gap-2">
              <p class="text-xs font-medium uppercase tracking-[0.22em] text-gray-500 dark:text-gray-400">
                {{ t('settings.basic.network.endpointsTitle') }}
              </p>
              <span class="meta-counter">{{ publishedEndpointCount }}</span>
            </div>
            <div class="chip-grid mt-2">
              <span
                v-for="endpoint in runtimeProbe?.endpoints || []"
                :key="endpoint"
                class="chip"
              >
                {{ endpoint }}
              </span>
              <span v-if="!runtimeProbe?.endpoints?.length" class="chip chip--muted">
                {{ t('settings.basic.network.runtimeHint') }}
              </span>
            </div>
          </div>

          <div class="meta-group">
            <div class="flex items-center justify-between gap-2">
              <p class="text-xs font-medium uppercase tracking-[0.22em] text-gray-500 dark:text-gray-400">
                {{ t('settings.basic.network.featuresTitle') }}
              </p>
              <span class="meta-counter">{{ enabledFeatureCount }}</span>
            </div>
            <div class="chip-grid mt-2">
              <span
                v-for="feature in runtimeFeatureEntries"
                :key="feature.key"
                class="chip"
              >
                {{ feature.label }}
              </span>
              <span v-if="!runtimeFeatureEntries.length" class="chip chip--muted">
                {{ t('settings.basic.network.runtimeHint') }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="mt-4 border-t border-gray-200/70 pt-4 dark:border-gray-700/70">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0">
            <p class="text-sm font-medium text-gray-900 dark:text-white">
              {{ t('settings.basic.network.sandboxTitle') }}
            </p>
            <p class="text-xs text-gray-500 dark:text-gray-400">
              {{ t('settings.basic.network.sandboxDesc') }}
            </p>
          </div>
          <UButton
            :loading="isRefreshingSandbox"
            :disabled="isRefreshingSandbox"
            color="neutral"
            variant="soft"
            size="sm"
            @click="refreshSandboxDoctor"
          >
            <UIcon name="i-heroicons-shield-check" class="mr-1 h-4 w-4" />
            {{
              isRefreshingSandbox
                ? t('settings.basic.network.refreshing')
                : t('settings.basic.network.refreshSandbox')
            }}
          </UButton>
        </div>

        <div class="mt-3">
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
            {{ t('settings.basic.network.fileGatewayDir') }}
          </label>
          <UInput
            v-model="sandboxGatewayDir"
            :placeholder="t('settings.basic.network.fileGatewayDirPlaceholder')"
            size="sm"
            class="w-full"
          />
        </div>

        <Transition name="xeno-fade">
          <div
            v-if="sandboxProbe"
            class="status-note mt-3"
            :class="sandboxProbe.success ? 'status-note--success' : 'status-note--warning'"
          >
            <UIcon
              :name="sandboxProbe.success ? 'i-heroicons-check-circle' : 'i-heroicons-exclamation-triangle'"
              class="h-4 w-4"
            />
            <span>
              {{ t('settings.basic.network.sandboxRecommended') }}: {{ sandboxProbe.recommendedMode }}
            </span>
            <span class="status-note__meta">
              {{ t('settings.basic.network.lastChecked') }} {{ sandboxProbe.checkedAt }}
            </span>
          </div>
        </Transition>

        <div v-if="sandboxProbe" class="meta-grid mt-3">
          <div class="meta-cell">
            <p class="meta-label">{{ t('settings.basic.network.tcpCapability') }}</p>
            <p class="meta-value">
              {{
                boolToken(
                  sandboxProbe.tcpAllowed,
                  'settings.basic.network.directAvailable',
                  'settings.basic.network.restricted'
                )
              }}
            </p>
          </div>
          <div class="meta-cell">
            <p class="meta-label">{{ t('settings.basic.network.udsCapability') }}</p>
            <p class="meta-value">
              {{
                sandboxProbe.udsSupported
                  ? boolToken(
                      sandboxProbe.udsAllowed,
                      'settings.basic.network.directAvailable',
                      'settings.basic.network.restricted'
                    )
                  : t('settings.basic.network.unsupported')
              }}
            </p>
          </div>
          <div class="meta-cell">
            <p class="meta-label">{{ t('settings.basic.network.fileGatewayCapability') }}</p>
            <p class="meta-value">
              {{
                boolToken(
                  sandboxProbe.fileGatewayWritable,
                  'settings.basic.network.writable',
                  'settings.basic.network.restricted'
                )
              }}
            </p>
          </div>
        </div>

        <div v-if="sandboxProbe" class="mt-3 grid gap-3 lg:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)]">
          <div class="space-y-3">
            <div class="meta-group">
              <p class="text-xs font-medium uppercase tracking-[0.22em] text-gray-500 dark:text-gray-400">
                {{ t('settings.basic.network.capabilityNotes') }}
              </p>
              <div class="mt-2 space-y-2 text-xs text-gray-600 dark:text-gray-300">
                <p>
                  <span class="font-semibold text-gray-800 dark:text-gray-100">TCP:</span>
                  {{ sandboxProbe.tcpError || t('settings.basic.network.noIssueDetected') }}
                </p>
                <p>
                  <span class="font-semibold text-gray-800 dark:text-gray-100">UDS:</span>
                  {{ sandboxProbe.udsError || sandboxProbe.udsPath || t('settings.basic.network.noIssueDetected') }}
                </p>
                <p>
                  <span class="font-semibold text-gray-800 dark:text-gray-100">File Gateway:</span>
                  {{ sandboxProbe.fileGatewayError || sandboxProbe.fileGatewayDir || t('settings.basic.network.noIssueDetected') }}
                </p>
              </div>
            </div>
          </div>

          <div class="result-panel">
            <p class="mb-2 text-xs font-medium uppercase tracking-[0.22em] text-gray-500 dark:text-gray-400">
              {{ t('settings.basic.network.sandboxCommand') }}
            </p>
            <pre class="max-h-72 overflow-auto whitespace-pre-wrap break-all text-[11px] leading-relaxed text-gray-700 dark:text-gray-200">{{ sandboxProbe.recommendedCommand || t('settings.basic.network.runtimeHint') }}</pre>
            <p class="mt-3 text-xs text-gray-500 dark:text-gray-400">
              {{ t('settings.basic.network.sandboxHint') }}
            </p>
          </div>
        </div>
      </div>
    </section>

    <section class="xeno-panel rounded-2xl border border-gray-200/80 p-4 dark:border-gray-700/80">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium text-gray-900 dark:text-white">
            {{ t('settings.basic.network.mcpTitle') }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            {{ t('settings.basic.network.mcpDesc') }}
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
          {{ isCheckingMcp ? t('settings.basic.network.checkingMcp') : t('settings.basic.network.checkMcp') }}
        </UButton>
      </div>

      <div class="mt-3">
        <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
          {{ t('settings.basic.network.mcpBaseUrl') }}
        </label>
        <UInput
          v-model="mcpBaseUrl"
          placeholder="http://127.0.0.1:8081"
          size="sm"
          class="w-full"
        />
      </div>

      <Transition name="xeno-fade">
        <div
          v-if="mcpCheckResult"
          class="status-note mt-3"
          :class="mcpCheckResult.success ? 'status-note--success' : 'status-note--warning'"
        >
          <UIcon
            :name="mcpCheckResult.success ? 'i-heroicons-check-circle' : 'i-heroicons-exclamation-triangle'"
            class="h-4 w-4"
          />
          <span>{{ mcpCheckResult.message }}</span>
        </div>
      </Transition>

      <div v-if="mcpCheckResult" class="meta-grid mt-3">
        <div class="meta-cell">
          <p class="meta-label">{{ t('settings.basic.network.mcpHttpStatus') }}</p>
          <p class="meta-value">{{ mcpCheckResult.httpStatus }}</p>
        </div>
        <div class="meta-cell">
          <p class="meta-label">{{ t('settings.basic.network.mcpHttpTools') }}</p>
          <p class="meta-value">{{ mcpCheckResult.httpToolCount }}</p>
        </div>
        <div class="meta-cell">
          <p class="meta-label">{{ t('settings.basic.network.mcpRpcTools') }}</p>
          <p class="meta-value">{{ mcpCheckResult.rpcToolCount }}</p>
        </div>
      </div>

      <div class="mt-4 border-t border-gray-200/70 pt-4 dark:border-gray-700/70">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0">
            <p class="text-sm font-medium text-gray-900 dark:text-white">
              {{ t('settings.basic.network.integrationPresetTitle') }}
            </p>
            <p class="text-xs text-gray-500 dark:text-gray-400">
              {{ t('settings.basic.network.integrationPresetDesc') }}
            </p>
          </div>
          <div class="flex flex-wrap gap-2">
            <UButton
              :loading="isLoadingIntegrationCatalog"
              :disabled="isLoadingIntegrationCatalog"
              color="neutral"
              variant="ghost"
              size="sm"
              @click="loadMcpIntegrationCatalog"
            >
              <UIcon name="i-heroicons-arrow-path" class="mr-1 h-4 w-4" />
              {{ t('settings.basic.network.refreshCatalog') }}
            </UButton>
            <UButton
              :loading="isLoadingIntegrationPreset"
              :disabled="isLoadingIntegrationPreset || !selectedIntegrationTarget"
              color="neutral"
              variant="soft"
              size="sm"
              @click="loadSelectedIntegrationPreset"
            >
              <UIcon name="i-heroicons-document-duplicate" class="mr-1 h-4 w-4" />
              {{ t('settings.basic.network.fetchPreset') }}
            </UButton>
          </div>
        </div>

        <div class="mt-3 grid gap-3 md:grid-cols-[minmax(0,220px)_minmax(0,1fr)]">
          <div>
            <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
              {{ t('settings.basic.network.integrationTarget') }}
            </label>
            <UTabs
              :model-value="selectedIntegrationTarget"
              size="sm"
              class="gap-0"
              :items="integrationTargetOptions"
              @update:model-value="handleIntegrationTargetChange"
            />
            <p class="mt-2 text-xs text-gray-500 dark:text-gray-400">
              {{
                integrationTargets.find((item) => item.id === selectedIntegrationTarget)?.description ||
                t('settings.basic.network.integrationPresetDesc')
              }}
            </p>
          </div>

          <div>
            <Transition name="xeno-fade">
              <div
                v-if="integrationPresetError"
                class="status-note status-note--warning"
              >
                <UIcon name="i-heroicons-exclamation-triangle" class="h-4 w-4" />
                <span>{{ integrationPresetError }}</span>
              </div>
            </Transition>

            <div v-if="integrationPresetText" class="result-panel">
              <p class="mb-2 text-xs font-medium uppercase tracking-[0.22em] text-gray-500 dark:text-gray-400">
                {{ t('settings.basic.network.presetPayload') }}
              </p>
              <pre class="max-h-72 overflow-auto whitespace-pre-wrap break-all text-[11px] leading-relaxed text-gray-700 dark:text-gray-200">{{ integrationPresetText }}</pre>
            </div>
            <div
              v-else-if="!integrationPresetError"
              class="meta-group"
            >
              <p class="text-xs text-gray-500 dark:text-gray-400">
                {{ t('settings.basic.network.presetHint') }}
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="xeno-panel rounded-2xl border border-gray-200/80 p-4 dark:border-gray-700/80">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium text-gray-900 dark:text-white">
            {{ t('settings.basic.network.playgroundTitle') }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            {{ t('settings.basic.network.playgroundDesc') }}
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
          {{
            isRunningMcpTool
              ? t('settings.basic.network.runningTool')
              : t('settings.basic.network.runTool')
          }}
        </UButton>
      </div>

      <div class="mt-3 grid gap-3 md:grid-cols-2">
        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
            {{ t('settings.basic.network.protocol') }}
          </label>
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
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
            {{ t('settings.basic.network.toolName') }}
          </label>
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
            {{ t('settings.basic.network.optionalSessionId') }}
          </label>
          <UInput
            v-model="mcpSessionId"
            placeholder="1"
            size="sm"
            class="w-full"
          />
        </div>

        <div>
          <label class="mb-1.5 block text-xs font-medium text-gray-700 dark:text-gray-300">
            {{ t('settings.basic.network.toolArgs') }}
          </label>
          <textarea
            v-model="mcpToolArgsText"
            rows="4"
            class="playground-input"
            placeholder="{}"
          />
        </div>
      </div>

      <Transition name="xeno-fade">
        <div
          v-if="mcpToolError"
          class="status-note status-note--warning mt-3"
        >
          <UIcon name="i-heroicons-exclamation-triangle" class="h-4 w-4" />
          <span>{{ mcpToolError }}</span>
        </div>
      </Transition>

      <div
        v-if="mcpToolResultText"
        class="result-panel mt-3"
      >
        <p class="mb-2 text-xs font-medium uppercase tracking-[0.22em] text-gray-500 dark:text-gray-400">
          {{ t('settings.basic.network.result') }}
        </p>
        <pre class="max-h-72 overflow-auto whitespace-pre-wrap break-all text-[11px] leading-relaxed text-gray-700 dark:text-gray-200">{{ mcpToolResultText }}</pre>
      </div>
    </section>
  </div>
</template>

<style scoped>
.network-surface {
  position: relative;
}

.signal-grid {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
}

.signal-card,
.xeno-panel,
.meta-cell,
.result-panel,
.playground-input {
  position: relative;
  overflow: hidden;
}

.signal-card {
  display: flex;
  min-height: 126px;
  gap: 0.85rem;
  border-radius: 1rem;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(248, 250, 252, 0.82)),
    radial-gradient(circle at top right, rgba(34, 211, 238, 0.08), transparent 42%);
  padding: 1rem;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.7),
    0 18px 40px rgba(15, 23, 42, 0.08);
}

.signal-card::before,
.xeno-panel::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.signal-card--success::before {
  background: radial-gradient(circle at top right, rgba(16, 185, 129, 0.18), transparent 45%);
}

.signal-card--warning::before {
  background: radial-gradient(circle at top right, rgba(251, 191, 36, 0.18), transparent 45%);
}

.signal-icon {
  display: inline-flex;
  height: 2rem;
  width: 2rem;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  border: 1px solid rgba(148, 163, 184, 0.28);
  background: rgba(255, 255, 255, 0.76);
  color: rgb(14, 116, 144);
  backdrop-filter: blur(16px);
}

.signal-label {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: rgb(100, 116, 139);
}

.signal-value {
  margin-top: 0.45rem;
  font-size: 1.05rem;
  font-weight: 700;
  color: rgb(15, 23, 42);
}

.signal-detail {
  margin-top: 0.4rem;
  font-size: 0.78rem;
  line-height: 1.45;
  color: rgb(71, 85, 105);
}

.xeno-panel {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(248, 250, 252, 0.78)),
    radial-gradient(circle at top right, rgba(56, 189, 248, 0.08), transparent 44%);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.7),
    0 24px 60px rgba(15, 23, 42, 0.08);
  backdrop-filter: blur(20px);
}

.xeno-panel::before {
  background:
    radial-gradient(circle at top left, rgba(14, 165, 233, 0.08), transparent 32%),
    linear-gradient(135deg, rgba(255, 255, 255, 0.12), transparent 60%);
}

.meta-grid {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
}

.meta-group {
  border-radius: 1rem;
  border: 1px solid rgba(148, 163, 184, 0.18);
  background: rgba(255, 255, 255, 0.54);
  padding: 0.85rem;
}

.meta-cell {
  border-radius: 0.9rem;
  border: 1px solid rgba(148, 163, 184, 0.2);
  background: rgba(255, 255, 255, 0.58);
  padding: 0.85rem 0.9rem;
  backdrop-filter: blur(16px);
}

.meta-label {
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: rgb(100, 116, 139);
}

.meta-value {
  margin-top: 0.35rem;
  font-size: 0.88rem;
  font-weight: 600;
  color: rgb(15, 23, 42);
  word-break: break-word;
}

.meta-counter {
  display: inline-flex;
  min-width: 1.8rem;
  justify-content: center;
  border-radius: 999px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.72);
  padding: 0.15rem 0.5rem;
  font-size: 0.72rem;
  font-weight: 600;
  color: rgb(30, 41, 59);
}

.chip-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  border-radius: 999px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.74);
  padding: 0.32rem 0.65rem;
  font-size: 0.72rem;
  color: rgb(51, 65, 85);
}

.chip--muted {
  opacity: 0.8;
}

.status-note {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  border-radius: 0.95rem;
  border: 1px solid rgba(148, 163, 184, 0.22);
  padding: 0.8rem 0.95rem;
  font-size: 0.78rem;
  line-height: 1.45;
}

.status-note--success {
  background: rgba(16, 185, 129, 0.08);
  color: rgb(5, 150, 105);
}

.status-note--warning {
  background: rgba(251, 191, 36, 0.1);
  color: rgb(180, 83, 9);
}

.status-note__meta {
  margin-left: auto;
  color: inherit;
  opacity: 0.78;
}

.playground-input {
  width: 100%;
  border-radius: 0.8rem;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.7);
  padding: 0.65rem 0.8rem;
  font-size: 0.75rem;
  color: rgb(51, 65, 85);
  outline: none;
}

.playground-input:focus {
  border-color: rgba(56, 189, 248, 0.6);
  box-shadow: 0 0 0 3px rgba(56, 189, 248, 0.12);
}

.result-panel {
  border-radius: 1rem;
  border: 1px solid rgba(148, 163, 184, 0.2);
  background: rgba(255, 255, 255, 0.7);
  padding: 0.9rem;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6);
}

.xeno-fade-enter-active,
.xeno-fade-leave-active {
  transition: opacity 180ms ease, transform 180ms ease;
}

.xeno-fade-enter-from,
.xeno-fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

:global(.dark) .signal-card {
  border-color: rgba(51, 65, 85, 0.75);
  background:
    linear-gradient(180deg, rgba(15, 23, 42, 0.88), rgba(2, 6, 23, 0.86)),
    radial-gradient(circle at top right, rgba(56, 189, 248, 0.12), transparent 42%);
  box-shadow:
    inset 0 1px 0 rgba(148, 163, 184, 0.08),
    0 26px 50px rgba(2, 6, 23, 0.45);
}

:global(.dark) .signal-icon,
:global(.dark) .meta-counter,
:global(.dark) .chip,
:global(.dark) .result-panel,
:global(.dark) .playground-input,
:global(.dark) .meta-cell,
:global(.dark) .meta-group {
  border-color: rgba(71, 85, 105, 0.7);
  background: rgba(15, 23, 42, 0.68);
}

:global(.dark) .xeno-panel {
  background:
    linear-gradient(180deg, rgba(15, 23, 42, 0.9), rgba(2, 6, 23, 0.86)),
    radial-gradient(circle at top right, rgba(14, 165, 233, 0.12), transparent 44%);
  box-shadow:
    inset 0 1px 0 rgba(148, 163, 184, 0.08),
    0 28px 60px rgba(2, 6, 23, 0.5);
}

:global(.dark) .signal-label,
:global(.dark) .meta-label {
  color: rgb(148, 163, 184);
}

:global(.dark) .signal-value,
:global(.dark) .meta-value {
  color: rgb(226, 232, 240);
}

:global(.dark) .signal-detail,
:global(.dark) .chip {
  color: rgb(148, 163, 184);
}

:global(.dark) .playground-input {
  color: rgb(226, 232, 240);
}

:global(.dark) .status-note--success {
  background: rgba(16, 185, 129, 0.12);
  color: rgb(110, 231, 183);
}

:global(.dark) .status-note--warning {
  background: rgba(251, 191, 36, 0.14);
  color: rgb(253, 230, 138);
}
</style>
