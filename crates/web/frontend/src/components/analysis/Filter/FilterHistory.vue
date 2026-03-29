<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const sessionStore = useSessionStore();

// Props
const open = defineModel<boolean>("open", { default: false });

// Emits
const emit = defineEmits<{
  load: [
    condition: {
      mode: "condition" | "session";
      conditionFilter?: {
        keywords: string[];
        timeRange: { start: number; end: number } | null;
        senderIds: number[];
        contextSize: number;
      };
      selectedSessionIds?: number[];
    },
  ];
}>();

// English engineering note.
interface FilterHistoryItem {
  id: string;
  sessionId: string;
  createdAt: number;
  name: string;
  mode: "condition" | "session";
  conditionFilter?: {
    keywords: string[];
    timeRange: { start: number; end: number } | null;
    senderIds: number[];
    contextSize: number;
  };
  selectedSessionIds?: number[];
}

// English engineering note.
const historyList = ref<FilterHistoryItem[]>([]);

// English engineering note.
const editingId = ref<string | null>(null);
const editingName = ref("");

// English engineering note.
const STORAGE_KEY = "xenobot_filter_history";

// English engineering note.
function loadHistory() {
  try {
    const data = localStorage.getItem(STORAGE_KEY);
    if (data) {
      const allHistory: FilterHistoryItem[] = JSON.parse(data);
      // English engineering note.
      historyList.value = allHistory.filter(
        (h) => h.sessionId === sessionStore.currentSessionId,
      );
    }
  } catch (error) {
    console.error("[FilterHistory] Failed to load history:", error);
  }
}

// English engineering note.
function saveHistory() {
  try {
    const data = localStorage.getItem(STORAGE_KEY);
    let allHistory: FilterHistoryItem[] = data ? JSON.parse(data) : [];

    // English engineering note.
    allHistory = allHistory.filter(
      (h) => h.sessionId !== sessionStore.currentSessionId,
    );
    allHistory = [...allHistory, ...historyList.value];

    localStorage.setItem(STORAGE_KEY, JSON.stringify(allHistory));
  } catch (error) {
    console.error("[FilterHistory] Failed to save history:", error);
  }
}

// English engineering note.
function loadCondition(item: FilterHistoryItem) {
  emit("load", {
    mode: item.mode,
    conditionFilter: item.conditionFilter,
    selectedSessionIds: item.selectedSessionIds,
  });
}

// English engineering note.
function startEdit(item: FilterHistoryItem) {
  editingId.value = item.id;
  editingName.value = item.name;
}

// English engineering note.
function saveName(item: FilterHistoryItem) {
  item.name = editingName.value || item.name;
  editingId.value = null;
  saveHistory();
}

// English engineering note.
function cancelEdit() {
  editingId.value = null;
}

// English engineering note.
function formatTime(ts: number): string {
  return new Date(ts).toLocaleString();
}

// English engineering note.
function formatSummary(item: FilterHistoryItem): string {
  if (item.mode === "condition") {
    const parts: string[] = [];
    if (item.conditionFilter?.keywords.length) {
      parts.push(
        `${t("analysis.filter.historyKeywordPrefix")}: ${item.conditionFilter.keywords.join(", ")}`,
      );
    }
    if (item.conditionFilter?.senderIds.length) {
      parts.push(
        t("analysis.filter.historyMemberCount", {
          count: item.conditionFilter.senderIds.length,
        }),
      );
    }
    return parts.join(" | ") || t("analysis.filter.historyNoCriteria");
  } else {
    return t("analysis.filter.historySessionCount", {
      count: item.selectedSessionIds?.length || 0,
    });
  }
}

// English engineering note.
watch(open, (val) => {
  if (val) {
    loadHistory();
  }
});

onMounted(() => {
  loadHistory();
});

// English engineering note.
defineExpose({
  saveCondition(
    condition: Omit<
      FilterHistoryItem,
      "id" | "sessionId" | "createdAt" | "name"
    >,
  ) {
    const newItem: FilterHistoryItem = {
      id: `filter_${Date.now()}`,
      sessionId: sessionStore.currentSessionId || "",
      createdAt: Date.now(),
      name: t("analysis.filter.historyAutoName", {
        index: historyList.value.length + 1,
      }),
      ...condition,
    };
    historyList.value.unshift(newItem);
    // English engineering note.
    historyList.value = historyList.value.slice(0, 20);
    saveHistory();
  },
});
</script>

<template>
  <UModal
    v-model:open="open"
    :ui="{ overlay: 'z-[10001]', content: 'z-[10001] max-w-3xl' }"
  >
    <template #content>
      <UCard class="xeno-filter-history-card">
        <template #header>
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-semibold">
              {{ t("analysis.filter.historyTitle") }}
            </h3>
            <UButton
              variant="ghost"
              icon="i-heroicons-x-mark"
              size="sm"
              @click="open = false"
            />
          </div>
        </template>

        <div
          class="xeno-filter-history-scroll max-h-96 overflow-y-auto rounded-2xl"
        >
          <div
            v-if="historyList.length === 0"
            class="xeno-filter-history-empty py-8 text-center text-gray-500"
          >
            {{ t("analysis.filter.noHistory") }}
          </div>

          <div v-else class="space-y-2 p-1">
            <div
              v-for="item in historyList"
              :key="item.id"
              class="xeno-filter-history-item rounded-2xl px-3 py-3"
            >
              <div class="flex items-start justify-between gap-2">
                <div class="flex-1 min-w-0">
                  <div
                    v-if="editingId === item.id"
                    class="flex items-center gap-2 mb-1"
                  >
                    <UInput
                      v-model="editingName"
                      size="sm"
                      class="flex-1"
                      @keydown.enter="saveName(item)"
                    />
                    <UButton size="xs" @click="saveName(item)">{{
                      t("common.save")
                    }}</UButton>
                    <UButton size="xs" variant="ghost" @click="cancelEdit">{{
                      t("common.cancel")
                    }}</UButton>
                  </div>
                  <div v-else class="flex items-center gap-2 mb-1">
                    <span
                      class="break-words font-medium text-gray-900 dark:text-white"
                      >{{ item.name }}</span
                    >
                    <UBadge
                      :color="item.mode === 'condition' ? 'primary' : 'green'"
                      size="xs"
                    >
                      {{
                        item.mode === "condition"
                          ? t("analysis.filter.historyModeCondition")
                          : t("analysis.filter.historyModeSession")
                      }}
                    </UBadge>
                    <button
                      class="text-gray-400 hover:text-gray-600"
                      @click="startEdit(item)"
                    >
                      <UIcon name="i-heroicons-pencil" class="w-3 h-3" />
                    </button>
                  </div>

                  <p
                    class="break-words text-sm text-gray-600 dark:text-gray-400"
                  >
                    {{ formatSummary(item) }}
                  </p>

                  <p class="text-xs text-gray-400 mt-1">
                    {{ formatTime(item.createdAt) }}
                  </p>
                </div>

                <div class="flex shrink-0 items-center gap-1">
                  <UButton
                    size="xs"
                    variant="soft"
                    @click="loadCondition(item)"
                  >
                    {{ t("common.load") }}
                  </UButton>
                </div>
              </div>
            </div>
          </div>
        </div>
      </UCard>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-filter-history-card {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.6rem;
  background:
    radial-gradient(
      circle at top left,
      rgba(84, 214, 255, 0.12),
      transparent 24%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 22%),
    rgba(7, 18, 29, 0.95);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 30px 72px rgba(2, 8, 16, 0.36);
  backdrop-filter: blur(22px) saturate(134%);
}

.xeno-filter-history-scroll {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(6, 16, 24, 0.44);
}

.xeno-filter-history-empty {
  border: 1px dashed rgba(139, 166, 189, 0.16);
  border-radius: 1.2rem;
  background: rgba(8, 18, 28, 0.44);
}

.xeno-filter-history-item {
  border: 1px solid rgba(139, 166, 189, 0.1);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(8, 18, 28, 0.66);
  transition:
    border-color 160ms ease,
    transform 160ms ease,
    background 160ms ease;
}

.xeno-filter-history-item:hover {
  border-color: rgba(84, 214, 255, 0.22);
  transform: translateY(-1px);
  background:
    radial-gradient(
      circle at top left,
      rgba(84, 214, 255, 0.08),
      transparent 30%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 120%),
    rgba(10, 20, 30, 0.78);
}
</style>
