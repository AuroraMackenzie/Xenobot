import { defineStore, storeToRefs } from "pinia";
import { ref, computed } from "vue";
import type { PromptPreset, AIPromptSettings } from "@/types/ai";
import type { KeywordTemplate } from "@/types/analysis";
import {
  DEFAULT_PRESET_ID,
  getBuiltinPresets,
  getOriginalBuiltinPreset,
  type LocaleType,
} from "@/config/prompts";
import { useSettingsStore } from "./settings";

// English engineering note.
const REMOTE_PRESET_BASE_URL = "https://xenobot.app";

/**
 * English note.
 */
export interface RemotePresetData {
  id: string;
  name: string;
  // English engineering note.
  path: string;
  // English engineering note.
  description?: string;
  // English engineering note.
  roleDefinition?: string;
  // English engineering note.
  responseRules?: string;
  // English engineering note.
  chatType?: "common" | "group" | "private";
}

/**
 * English note.
 */
export const usePromptStore = defineStore(
  "prompt",
  () => {
    // English engineering note.
    const settingsStore = useSettingsStore();
    const { locale } = storeToRefs(settingsStore);

    const customPromptPresets = ref<PromptPreset[]>([]);
    const builtinPresetOverrides = ref<
      Record<
        string,
        {
          name?: string;
          roleDefinition?: string;
          responseRules?: string;
          updatedAt?: number;
        }
      >
    >({});
    const aiPromptSettings = ref<AIPromptSettings>({
      activePresetId: DEFAULT_PRESET_ID,
    });
    const aiConfigVersion = ref(0);
    const aiGlobalSettings = ref({
      maxMessagesPerRequest: 1000,
      maxHistoryRounds: 5, // English engineering note.
      exportFormat: "markdown" as "markdown" | "txt", // English engineering note.
      sqlExportFormat: "csv" as "csv" | "json", // English engineering note.
    });
    const customKeywordTemplates = ref<KeywordTemplate[]>([]);
    const deletedPresetTemplateIds = ref<string[]>([]);
    // English engineering note.
    const fetchedRemotePresetIds = ref<string[]>([]);

    // English engineering note.
    const builtinPresets = computed(() =>
      getBuiltinPresets(locale.value as LocaleType),
    );

    // English engineering note.
    const allPromptPresets = computed(() => {
      const mergedBuiltins = builtinPresets.value.map((preset) => {
        const override = builtinPresetOverrides.value[preset.id];
        if (override) {
          return { ...preset, ...override };
        }
        return preset;
      });
      return [...mergedBuiltins, ...customPromptPresets.value];
    });

    // English engineering note.
    const activePreset = computed(() => {
      const preset = allPromptPresets.value.find(
        (p) => p.id === aiPromptSettings.value.activePresetId,
      );
      return (
        preset || builtinPresets.value.find((p) => p.id === DEFAULT_PRESET_ID)!
      );
    });

    /**
     * English note.
     * English note.
     */
    function getPresetsForChatType(
      chatType: "group" | "private",
    ): PromptPreset[] {
      return allPromptPresets.value.filter((preset) => {
        // English engineering note.
        if (preset.isBuiltIn) return true;
        // English engineering note.
        if (!preset.applicableTo || preset.applicableTo === "common")
          return true;
        // English engineering note.
        return preset.applicableTo === chatType;
      });
    }

    /**
     * English note.
     */
    function notifyAIConfigChanged() {
      aiConfigVersion.value++;
    }

    /**
     * English note.
     */
    function updateAIGlobalSettings(
      settings: Partial<{
        maxMessagesPerRequest: number;
        maxHistoryRounds: number;
        exportFormat: "markdown" | "txt";
        sqlExportFormat: "csv" | "json";
      }>,
    ) {
      aiGlobalSettings.value = { ...aiGlobalSettings.value, ...settings };
      notifyAIConfigChanged();
    }

    /**
     * English note.
     */
    function addCustomKeywordTemplate(template: KeywordTemplate) {
      customKeywordTemplates.value.push(template);
    }

    /**
     * English note.
     */
    function updateCustomKeywordTemplate(
      templateId: string,
      updates: Partial<Omit<KeywordTemplate, "id">>,
    ) {
      const index = customKeywordTemplates.value.findIndex(
        (t) => t.id === templateId,
      );
      if (index !== -1) {
        customKeywordTemplates.value[index] = {
          ...customKeywordTemplates.value[index],
          ...updates,
        };
      }
    }

    /**
     * English note.
     */
    function removeCustomKeywordTemplate(templateId: string) {
      const index = customKeywordTemplates.value.findIndex(
        (t) => t.id === templateId,
      );
      if (index !== -1) {
        customKeywordTemplates.value.splice(index, 1);
      }
    }

    /**
     * English note.
     */
    function addDeletedPresetTemplateId(id: string) {
      if (!deletedPresetTemplateIds.value.includes(id)) {
        deletedPresetTemplateIds.value.push(id);
      }
    }

    /**
     * English note.
     */
    function addPromptPreset(preset: {
      name: string;
      roleDefinition: string;
      responseRules: string;
      applicableTo?: "common" | "group" | "private";
    }) {
      const newPreset: PromptPreset = {
        id: `custom-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        name: preset.name,
        roleDefinition: preset.roleDefinition,
        responseRules: preset.responseRules,
        isBuiltIn: false,
        applicableTo: preset.applicableTo || "common",
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
      customPromptPresets.value.push(newPreset);
      return newPreset.id;
    }

    /**
     * English note.
     */
    function updatePromptPreset(
      presetId: string,
      updates: {
        name?: string;
        roleDefinition?: string;
        responseRules?: string;
        applicableTo?: "common" | "group" | "private";
      },
    ) {
      const isBuiltin = builtinPresets.value.some((p) => p.id === presetId);
      if (isBuiltin) {
        builtinPresetOverrides.value[presetId] = {
          ...builtinPresetOverrides.value[presetId],
          name: updates.name,
          roleDefinition: updates.roleDefinition,
          responseRules: updates.responseRules,
          updatedAt: Date.now(),
        };
        return;
      }

      const index = customPromptPresets.value.findIndex(
        (p) => p.id === presetId,
      );
      if (index !== -1) {
        customPromptPresets.value[index] = {
          ...customPromptPresets.value[index],
          ...updates,
          updatedAt: Date.now(),
        };
      }
    }

    /**
     * English note.
     */
    function resetBuiltinPreset(presetId: string): boolean {
      const original = getOriginalBuiltinPreset(
        presetId,
        locale.value as LocaleType,
      );
      if (!original) return false;
      delete builtinPresetOverrides.value[presetId];
      return true;
    }

    /**
     * English note.
     */
    function isBuiltinPresetModified(presetId: string): boolean {
      return !!builtinPresetOverrides.value[presetId];
    }

    /**
     * English note.
     */
    function removePromptPreset(presetId: string) {
      const index = customPromptPresets.value.findIndex(
        (p) => p.id === presetId,
      );
      if (index !== -1) {
        customPromptPresets.value.splice(index, 1);
        // English engineering note.
        if (aiPromptSettings.value.activePresetId === presetId) {
          aiPromptSettings.value.activePresetId = DEFAULT_PRESET_ID;
        }
        // English engineering note.
        const remoteIndex = fetchedRemotePresetIds.value.indexOf(presetId);
        if (remoteIndex !== -1) {
          fetchedRemotePresetIds.value.splice(remoteIndex, 1);
        }
      }
    }

    /**
     * English note.
     */
    function duplicatePromptPreset(presetId: string) {
      const source = allPromptPresets.value.find((p) => p.id === presetId);
      if (source) {
        const copySuffix = locale.value === "zh-CN" ? "(副本)" : "(Copy)";
        return addPromptPreset({
          name: `${source.name} ${copySuffix}`,
          roleDefinition: source.roleDefinition,
          responseRules: source.responseRules,
        });
      }
      return null;
    }

    /**
     * English note.
     */
    function setActivePreset(presetId: string) {
      const preset = allPromptPresets.value.find((p) => p.id === presetId);
      if (preset) {
        aiPromptSettings.value.activePresetId = presetId;
        notifyAIConfigChanged();
      }
    }

    /**
     * English note.
     * English note.
     */
    function getActivePresetForChatType(
      _chatType?: "group" | "private",
    ): PromptPreset {
      return activePreset.value;
    }

    /**
     * English note.
     * English note.
     * @returns { roleDefinition, responseRules }
     */
    function parseMarkdownContent(content: string): {
      roleDefinition: string;
      responseRules: string;
    } {
      // English engineering note.
      const separator = /\n---\n/;
      const parts = content.split(separator);

      if (parts.length >= 2) {
        return {
          roleDefinition: parts[0].trim(),
          responseRules: parts.slice(1).join("\n---\n").trim(),
        };
      }

      // English engineering note.
      return {
        roleDefinition: content.trim(),
        responseRules: "",
      };
    }

    /**
     * English note.
     * English note.
     * English note.
     */
    async function fetchRemotePresets(
      locale: string,
    ): Promise<RemotePresetData[]> {
      const langPath = locale === "zh-CN" ? "cn" : "en";
      const indexUrl = `${REMOTE_PRESET_BASE_URL}/${langPath}/system-prompt.json`;

      try {
        const result = await window.api.app.fetchRemoteConfig(indexUrl);
        if (!result.success || !result.data) {
          return [];
        }

        const presetIndex = result.data as RemotePresetData[];
        if (!Array.isArray(presetIndex)) {
          return [];
        }

        // English engineering note.
        return presetIndex.filter((p) => p.id && p.name && p.path);
      } catch {
        return [];
      }
    }

    /**
     * English note.
     * English note.
     * English note.
     */
    async function fetchPresetContent(
      preset: RemotePresetData,
    ): Promise<
      | (RemotePresetData & { roleDefinition: string; responseRules: string })
      | null
    > {
      // English engineering note.
      if (preset.roleDefinition && preset.responseRules) {
        return preset as RemotePresetData & {
          roleDefinition: string;
          responseRules: string;
        };
      }

      const mdUrl = `${REMOTE_PRESET_BASE_URL}${preset.path}`;
      try {
        const mdResult = await window.api.app.fetchRemoteConfig(mdUrl);
        if (!mdResult.success || typeof mdResult.data !== "string") {
          return null;
        }

        const { roleDefinition, responseRules } = parseMarkdownContent(
          mdResult.data,
        );
        if (!roleDefinition || !responseRules) {
          return null;
        }

        return {
          ...preset,
          roleDefinition,
          responseRules,
        };
      } catch {
        return null;
      }
    }

    /**
     * English note.
     * English note.
     * English note.
     */
    function addRemotePreset(preset: RemotePresetData): boolean {
      // English engineering note.
      if (fetchedRemotePresetIds.value.includes(preset.id)) {
        return false;
      }

      const now = Date.now();
      // English engineering note.
      const applicableTo = preset.chatType || "common";

      const newPreset: PromptPreset = {
        id: preset.id,
        name: preset.name,
        roleDefinition: preset.roleDefinition || "",
        responseRules: preset.responseRules || "",
        isBuiltIn: false,
        applicableTo,
        createdAt: now,
        updatedAt: now,
      };

      customPromptPresets.value.push(newPreset);
      fetchedRemotePresetIds.value.push(preset.id);
      return true;
    }

    /**
     * English note.
     * English note.
     */
    function isRemotePresetAdded(presetId: string): boolean {
      return fetchedRemotePresetIds.value.includes(presetId);
    }

    // English engineering note.

    /**
     * English note.
     * English note.
     */
    function migrateOldPresets() {
      // English engineering note.
      const oldSettings = aiPromptSettings.value as unknown as {
        activeGroupPresetId?: string;
        activePrivatePresetId?: string;
        activePresetId?: string;
      };

      // English engineering note.
      if (oldSettings.activeGroupPresetId && !oldSettings.activePresetId) {
        // English engineering note.
        const oldGroupId = oldSettings.activeGroupPresetId;
        // English engineering note.
        if (
          oldGroupId === "builtin-group-default" ||
          oldGroupId === "builtin-private-default"
        ) {
          aiPromptSettings.value.activePresetId = DEFAULT_PRESET_ID;
        } else {
          aiPromptSettings.value.activePresetId = oldGroupId;
        }
        // English engineering note.
        delete (aiPromptSettings.value as Record<string, unknown>)
          .activeGroupPresetId;
        delete (aiPromptSettings.value as Record<string, unknown>)
          .activePrivatePresetId;
      }

      // English engineering note.
      for (const preset of customPromptPresets.value) {
        const oldPreset = preset as PromptPreset & { chatType?: string };
        if (oldPreset.chatType) {
          delete oldPreset.chatType;
        }
      }
    }

    // English engineering note.
    migrateOldPresets();

    return {
      // state
      customPromptPresets,
      builtinPresetOverrides,
      aiPromptSettings,
      aiConfigVersion,
      aiGlobalSettings,
      customKeywordTemplates,
      deletedPresetTemplateIds,
      fetchedRemotePresetIds,
      // getters
      allPromptPresets,
      activePreset,
      // actions
      notifyAIConfigChanged,
      updateAIGlobalSettings,
      addCustomKeywordTemplate,
      updateCustomKeywordTemplate,
      removeCustomKeywordTemplate,
      addDeletedPresetTemplateId,
      addPromptPreset,
      updatePromptPreset,
      resetBuiltinPreset,
      isBuiltinPresetModified,
      removePromptPreset,
      duplicatePromptPreset,
      setActivePreset,
      getActivePresetForChatType,
      getPresetsForChatType,
      fetchRemotePresets,
      fetchPresetContent,
      addRemotePreset,
      isRemotePresetAdded,
    };
  },
  {
    persist: [
      {
        pick: [
          "customKeywordTemplates",
          "deletedPresetTemplateIds",
          "aiGlobalSettings",
          "customPromptPresets",
          "builtinPresetOverrides",
          "aiPromptSettings",
          "fetchedRemotePresetIds",
        ],
        storage: localStorage,
      },
    ],
  },
);
