<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

// Props
const props = defineProps<{
  open: boolean;
  sessionId: string;
}>();

// Emits
const emit = defineEmits<{
  "update:open": [value: boolean];
  generated: [sql: string, explanation: string, prompt: string];
  useSQL: [sql: string];
  runSQL: [sql: string];
}>();

// English engineering note.
const aiPrompt = ref("");
const isGenerating = ref(false);
const aiError = ref<string | null>(null);
const generatedSQL = ref("");
const generatedExplanation = ref("");
const streamingContent = ref("");
const showStreamingContent = ref(false);

// English engineering note.
function closeModal() {
  emit("update:open", false);
  // English engineering note.
  aiPrompt.value = "";
  aiError.value = null;
  generatedSQL.value = "";
  generatedExplanation.value = "";
  streamingContent.value = "";
  showStreamingContent.value = false;
}

// English engineering note.
async function generateSQL() {
  if (!aiPrompt.value.trim()) {
    aiError.value = t("ai.sqlLab.generate.errorEmptyPrompt");
    return;
  }

  isGenerating.value = true;
  aiError.value = null;
  generatedSQL.value = "";
  generatedExplanation.value = "";
  streamingContent.value = "";
  showStreamingContent.value = true;

  try {
    const result = await window.chatApi.generateSQL(
      props.sessionId,
      aiPrompt.value,
      {
        maxRows: 100,
      },
    );

    if (result?.success && result?.sql) {
      generatedSQL.value = String(result.sql).trim();
      generatedExplanation.value = String(result.explanation || "").trim();
      const warnings = Array.isArray(result.warnings)
        ? result.warnings
            .filter((item) => typeof item === "string" && item.trim())
            .map((item) => String(item))
        : [];
      streamingContent.value = warnings.join("\n");
      showStreamingContent.value = warnings.length > 0;
      emit(
        "generated",
        generatedSQL.value,
        generatedExplanation.value,
        aiPrompt.value,
      );
    } else {
      aiError.value = result.error || t("ai.sqlLab.generate.errorGenerate");
      showStreamingContent.value = false;
    }
  } catch (err: any) {
    aiError.value = err.message || String(err);
  } finally {
    isGenerating.value = false;
  }
}

// English engineering note.
function useGeneratedSQL() {
  if (generatedSQL.value) {
    emit("useSQL", generatedSQL.value);
    closeModal();
  }
}

// English engineering note.
function useAndRunSQL() {
  if (generatedSQL.value) {
    emit("runSQL", generatedSQL.value);
    closeModal();
  }
}
</script>

<template>
  <UModal
    :open="open"
    :title="t('ai.sqlLab.generate.title')"
    :description="t('ai.sqlLab.generate.description')"
    @update:open="emit('update:open', $event)"
  >
    <template #content>
      <div class="p-6">
        <div class="mb-4 flex items-center gap-2">
          <UIcon name="i-heroicons-sparkles" class="h-5 w-5 text-pink-500" />
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            {{ t("ai.sqlLab.generate.title") }}
          </h3>
        </div>

        <p class="mb-4 text-sm text-gray-500 dark:text-gray-400">
          {{ t("ai.sqlLab.generate.description") }}
        </p>

        <!-- English UI note -->
        <textarea
          v-model="aiPrompt"
          class="mb-4 h-24 w-full resize-none rounded-lg border border-gray-300 bg-white p-3 text-sm text-gray-800 focus:border-pink-500 focus:outline-none focus:ring-1 focus:ring-pink-500 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
          :placeholder="t('ai.sqlLab.generate.placeholder')"
          :disabled="isGenerating"
        />

        <!-- English UI note -->
        <div v-if="streamingContent || isGenerating" class="mb-4">
          <button
            class="mb-1.5 flex w-full items-center gap-1.5 text-left text-xs font-medium text-gray-500 dark:text-gray-400"
            @click="showStreamingContent = !showStreamingContent"
          >
            <UIcon
              :name="
                showStreamingContent
                  ? 'i-heroicons-chevron-down'
                  : 'i-heroicons-chevron-right'
              "
              class="h-3.5 w-3.5"
            />
            <UIcon name="i-heroicons-cpu-chip" class="h-3.5 w-3.5" />
            {{ t("ai.sqlLab.generate.aiOutput") }}
            <span v-if="isGenerating" class="ml-1 text-pink-500">{{
              t("common.generating")
            }}</span>
          </button>
          <div
            v-show="showStreamingContent"
            class="max-h-40 overflow-y-auto rounded-lg bg-gray-50 p-3 dark:bg-gray-900"
          >
            <pre
              class="whitespace-pre-wrap break-all font-mono text-xs text-gray-600 dark:text-gray-400"
              >{{ streamingContent || t("ai.sqlLab.generate.waitingAI") }}</pre
            >
          </div>
        </div>

        <!-- English UI note -->
        <div
          v-if="aiError"
          class="mb-4 rounded-lg bg-red-50 p-3 dark:bg-red-950"
        >
          <p class="text-sm text-red-600 dark:text-red-400">{{ aiError }}</p>
        </div>

        <!-- English UI note -->
        <div v-if="generatedSQL" class="mb-4 space-y-3">
          <!-- English UI note -->
          <div>
            <p
              class="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-gray-500 dark:text-gray-400"
            >
              <UIcon name="i-heroicons-code-bracket" class="h-3.5 w-3.5" />
              {{ t("ai.sqlLab.generate.sqlStatement") }}
            </p>
            <div class="rounded-lg bg-gray-100 p-3 dark:bg-gray-800">
              <pre
                class="whitespace-pre-wrap break-all font-mono text-sm text-gray-800 dark:text-gray-200"
                >{{ generatedSQL }}</pre
              >
            </div>
          </div>

          <!-- English UI note -->
          <div v-if="generatedExplanation">
            <p
              class="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-gray-500 dark:text-gray-400"
            >
              <UIcon name="i-heroicons-light-bulb" class="h-3.5 w-3.5" />
              {{ t("ai.sqlLab.generate.explanation") }}
            </p>
            <div class="rounded-lg bg-blue-50 p-3 dark:bg-blue-950">
              <p class="text-sm text-blue-800 dark:text-blue-200">
                {{ generatedExplanation }}
              </p>
            </div>
          </div>
        </div>

        <!-- English UI note -->
        <div class="flex justify-end gap-2">
          <UButton variant="ghost" @click="closeModal">{{
            t("common.cancel")
          }}</UButton>

          <UButton
            v-if="!generatedSQL"
            color="primary"
            :loading="isGenerating"
            :disabled="!aiPrompt.trim()"
            @click="generateSQL"
          >
            <UIcon name="i-heroicons-sparkles" class="mr-1 h-4 w-4" />
            {{ t("ai.sqlLab.generate.generateSQL") }}
          </UButton>

          <template v-else>
            <UButton
              variant="outline"
              :loading="isGenerating"
              @click="generateSQL"
            >
              {{ t("common.regenerate") }}
            </UButton>
            <UButton variant="outline" @click="useGeneratedSQL">{{
              t("ai.sqlLab.generate.useSQL")
            }}</UButton>
            <UButton color="primary" @click="useAndRunSQL">
              <UIcon name="i-heroicons-play" class="mr-1 h-4 w-4" />
              {{ t("ai.sqlLab.generate.run") }}
            </UButton>
          </template>
        </div>
      </div>
    </template>
  </UModal>
</template>
