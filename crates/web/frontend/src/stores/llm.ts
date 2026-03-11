import { defineStore } from "pinia";
import { ref, computed } from "vue";

/**
 * Presentation shape for an AI provider configuration row in the frontend.
 */
export interface AIServiceConfigDisplay {
  id: string;
  name: string;
  provider: string;
  apiKeySet: boolean;
  model?: string;
  baseUrl?: string;
  createdAt: number;
  updatedAt: number;
}

/**
 * Static metadata for a supported LLM provider and its model catalog.
 */
export interface LLMProvider {
  id: string;
  name: string;
  description: string;
  defaultBaseUrl: string;
  models: Array<{ id: string; name: string; description?: string }>;
}

/**
 * Frontend store for provider metadata and the active AI model selection.
 */
export const useLLMStore = defineStore("llm", () => {
  /** Available model configurations loaded from the desktop bridge. */
  const configs = ref<AIServiceConfigDisplay[]>([]);

  /** Provider registry used by the settings UI. */
  const providers = ref<LLMProvider[]>([]);

  /** Identifier of the currently selected model configuration. */
  const activeConfigId = ref<string | null>(null);

  /** Loading state for provider and configuration requests. */
  const isLoading = ref(false);

  /** Prevents repeated bootstrap work. */
  const isInitialized = ref(false);

  /** Resolved active configuration object. */
  const activeConfig = computed(
    () => configs.value.find((c) => c.id === activeConfigId.value) || null,
  );

  /** Whether any configuration is currently active. */
  const hasConfig = computed(() => !!activeConfigId.value);

  /** Hard cap used by the UI when creating new configurations. */
  const isMaxConfigs = computed(() => configs.value.length >= 10);

  // English engineering note.

  /**
   * English note.
   */
  async function init() {
    if (isInitialized.value) return;
    await loadConfigs();
    isInitialized.value = true;
  }

  /**
   * English note.
   */
  async function loadConfigs() {
    isLoading.value = true;
    try {
      const [providersData, configsData, activeId] = await Promise.all([
        window.llmApi.getProviders(),
        window.llmApi.getAllConfigs(),
        window.llmApi.getActiveConfigId(),
      ]);
      providers.value = providersData;
      configs.value = configsData;
      activeConfigId.value = activeId;
    } catch (error) {
      console.error("[LLM Store] Failed to load model configurations:", error);
    } finally {
      isLoading.value = false;
    }
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  async function setActiveConfig(id: string): Promise<boolean> {
    try {
      const result = await window.llmApi.setActiveConfig(id);
      if (result.success) {
        activeConfigId.value = id;
        return true;
      }
      console.error(
        "[LLM Store] Failed to set active model configuration:",
        result.error,
      );
      return false;
    } catch (error) {
      console.error(
        "[LLM Store] Failed to set active model configuration:",
        error,
      );
      return false;
    }
  }

  /**
   * English note.
   * English note.
   */
  async function refreshConfigs() {
    await loadConfigs();
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  function getProviderName(providerId: string): string {
    return providers.value.find((p) => p.id === providerId)?.name || providerId;
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
  };
});
