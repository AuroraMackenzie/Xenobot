import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

/**
 * English note.
 */
export interface AIServiceConfigDisplay {
  id: string
  name: string
  provider: string
  apiKeySet: boolean
  model?: string
  baseUrl?: string
  createdAt: number
  updatedAt: number
}

/**
 * English note.
 */
export interface LLMProvider {
  id: string
  name: string
  description: string
  defaultBaseUrl: string
  models: Array<{ id: string; name: string; description?: string }>
}

/**
 * English note.
 * English note.
 */
export const useLLMStore = defineStore('llm', () => {
  // English engineering note.

  /** English note.
  const configs = ref<AIServiceConfigDisplay[]>([])

  /** English note.
  const providers = ref<LLMProvider[]>([])

  /** English note.
  const activeConfigId = ref<string | null>(null)

  /** English note.
  const isLoading = ref(false)

  /** English note.
  const isInitialized = ref(false)

  // English engineering note.

  /** English note.
  const activeConfig = computed(() => configs.value.find((c) => c.id === activeConfigId.value) || null)

  /** English note.
  const hasConfig = computed(() => !!activeConfigId.value)

  /** English note.
  const isMaxConfigs = computed(() => configs.value.length >= 10)

  // English engineering note.

  /**
   * English note.
   */
  async function init() {
    if (isInitialized.value) return
    await loadConfigs()
    isInitialized.value = true
  }

  /**
   * English note.
   */
  async function loadConfigs() {
    isLoading.value = true
    try {
      const [providersData, configsData, activeId] = await Promise.all([
        window.llmApi.getProviders(),
        window.llmApi.getAllConfigs(),
        window.llmApi.getActiveConfigId(),
      ])
      providers.value = providersData
      configs.value = configsData
      activeConfigId.value = activeId
    } catch (error) {
      console.error('[LLM Store] 加载配置失败：', error)
    } finally {
      isLoading.value = false
    }
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  async function setActiveConfig(id: string): Promise<boolean> {
    try {
      const result = await window.llmApi.setActiveConfig(id)
      if (result.success) {
        activeConfigId.value = id
        return true
      }
      console.error('[LLM Store] 设置激活配置失败：', result.error)
      return false
    } catch (error) {
      console.error('[LLM Store] 设置激活配置失败：', error)
      return false
    }
  }

  /**
   * English note.
   * English note.
   */
  async function refreshConfigs() {
    await loadConfigs()
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  function getProviderName(providerId: string): string {
    return providers.value.find((p) => p.id === providerId)?.name || providerId
  }

  return {
    // English engineering note.
    configs,
    providers,
    activeConfigId,
    isLoading,
    isInitialized,
    // English engineering note.
    activeConfig,
    hasConfig,
    isMaxConfigs,
    // English engineering note.
    init,
    loadConfigs,
    setActiveConfig,
    refreshConfigs,
    getProviderName,
  }
})
