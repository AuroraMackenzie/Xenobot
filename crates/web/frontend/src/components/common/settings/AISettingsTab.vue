<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import AIModelConfigTab from "./AI/AIModelConfigTab.vue";
import AIPromptConfigTab from "./AI/AIPromptConfigTab.vue";
import AIPromptPresetTab from "./AI/AIPromptPresetTab.vue";
// English engineering note.
// import RAGConfigTab from './AI/RAGConfigTab.vue'
import SubTabs from "@/components/UI/SubTabs.vue";
import { useSubTabsScroll } from "@/composables/useSubTabsScroll";

const { t } = useI18n();

// Emits
const emit = defineEmits<{
  "config-changed": [];
}>();

// English engineering note.
const navItems = computed(() => [
  { id: "model", label: t("settings.tabs.aiConfig") },
  // English engineering note.
  // { id: 'rag', label: t('settings.tabs.aiRAG') },
  { id: "chat", label: t("settings.tabs.aiPrompt") },
  { id: "preset", label: t("settings.tabs.aiPreset") },
]);

// English engineering note.
const {
  activeNav,
  scrollContainerRef,
  setSectionRef,
  handleNavChange,
  scrollToId,
} = useSubTabsScroll(navItems);
void scrollContainerRef; // English engineering note.

// English engineering note.
function handleAIConfigChanged() {
  emit("config-changed");
}

/**
 * English note.
 */
function scrollToSection(sectionId: string) {
  scrollToId(sectionId);
}

// English engineering note.
defineExpose({
  scrollToSection,
});

// Template refs
const aiModelConfigRef = ref<InstanceType<typeof AIModelConfigTab> | null>(
  null,
);
void aiModelConfigRef.value;
</script>

<template>
  <div class="flex h-full gap-6">
    <!-- English UI note -->
    <div class="w-28 shrink-0">
      <SubTabs
        v-model="activeNav"
        :items="navItems"
        orientation="vertical"
        @change="handleNavChange"
      />
    </div>

    <!-- English UI note -->
    <div ref="scrollContainerRef" class="min-w-0 flex-1 overflow-y-auto">
      <div class="space-y-8">
        <!-- English UI note -->
        <div :ref="(el) => setSectionRef('model', el as HTMLElement)">
          <AIModelConfigTab
            ref="aiModelConfigRef"
            @config-changed="handleAIConfigChanged"
          />
        </div>

        <!-- English UI note -->
        <!--
        <div class="border-t border-gray-200 dark:border-gray-700" />
        <div :ref="(el) => setSectionRef('rag', el as HTMLElement)">
          <RAGConfigTab @config-changed="handleAIConfigChanged" />
        </div>
        -->

        <!-- English UI note -->
        <div class="border-t border-gray-200 dark:border-gray-700" />

        <!-- English UI note -->
        <div :ref="(el) => setSectionRef('chat', el as HTMLElement)">
          <AIPromptConfigTab @config-changed="handleAIConfigChanged" />
        </div>

        <!-- English UI note -->
        <div class="border-t border-gray-200 dark:border-gray-700" />

        <!-- English UI note -->
        <div :ref="(el) => setSectionRef('preset', el as HTMLElement)">
          <AIPromptPresetTab @config-changed="handleAIConfigChanged" />
        </div>
      </div>
    </div>
  </div>
</template>
