import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { EmbeddingServiceConfigDisplay } from '@electron/preload/index'

/**
 * English note.
 * English note.
 */
export const useEmbeddingStore = defineStore('embedding', () => {
  // English engineering note.

  /** English note.
  const configs = ref<EmbeddingServiceConfigDisplay[]>([])

  /** English note.
  const activeConfigId = ref<string | null>(null)

  /** English note.
  const isLoading = ref(false)

  /** English note.
  const isInitialized = ref(false)

  /** English note.
  const vectorStoreStats = ref<{
    enabled: boolean
    count?: number
    sizeBytes?: number
  }>({ enabled: false })

  // English engineering note.

  /** English note.
  const activeConfig = computed(() => configs.value.find((c) => c.id === activeConfigId.value) || null)

  /** English note.
  const hasConfig = computed(() => configs.value.length > 0)

  /** English note.
  const isMaxConfigs = computed(() => configs.value.length >= 10)

  /** English note.
  const vectorStoreSizeFormatted = computed(() => {
    const bytes = vectorStoreStats.value.sizeBytes ?? 0
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
  })

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
      const [configsData, activeId, isEnabled, stats] = await Promise.all([
        window.embeddingApi.getAllConfigs(),
        window.embeddingApi.getActiveConfigId(),
        window.embeddingApi.isEnabled(),
        window.embeddingApi.getVectorStoreStats(),
      ])
      configs.value = configsData
      activeConfigId.value = activeId
      vectorStoreStats.value = stats
    } catch (error) {
      console.error('[Embedding Store] 加载配置失败：', error)
    } finally {
      isLoading.value = false
    }
  }

  /**
   * English note.
   */
  async function setActiveConfig(id: string): Promise<boolean> {
    try {
      const result = await window.embeddingApi.setActiveConfig(id)
      if (result.success) {
        activeConfigId.value = id
        return true
      }
      console.error('[Embedding Store] 设置激活配置失败：', result.error)
      return false
    } catch (error) {
      console.error('[Embedding Store] 设置激活配置失败：', error)
      return false
    }
  }

  /**
   * English note.
   */
  async function deleteConfig(id: string): Promise<boolean> {
    try {
      const result = await window.embeddingApi.deleteConfig(id)
      if (result.success) {
        await loadConfigs()
        return true
      }
      console.error('[Embedding Store] 删除配置失败：', result.error)
      return false
    } catch (error) {
      console.error('[Embedding Store] 删除配置失败：', error)
      return false
    }
  }

  /**
   * English note.
   */
  async function clearVectorStore(): Promise<boolean> {
    try {
      const result = await window.embeddingApi.clearVectorStore()
      if (result.success) {
        vectorStoreStats.value.count = 0
        vectorStoreStats.value.sizeBytes = 0
        return true
      }
      console.error('[Embedding Store] 清空向量存储失败：', result.error)
      return false
    } catch (error) {
      console.error('[Embedding Store] 清空向量存储失败：', error)
      return false
    }
  }

  /**
   * English note.
   */
  async function refreshConfigs() {
    await loadConfigs()
  }

  return {
    // English engineering note.
    configs,
    activeConfigId,
    isLoading,
    isInitialized,
    vectorStoreStats,
    // English engineering note.
    activeConfig,
    hasConfig,
    isMaxConfigs,
    vectorStoreSizeFormatted,
    // English engineering note.
    init,
    loadConfigs,
    setActiveConfig,
    deleteConfig,
    clearVectorStore,
    refreshConfigs,
  }
})
