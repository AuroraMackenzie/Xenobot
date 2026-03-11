<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";

export interface ChatInfo {
  index: number;
  name: string;
  type: string;
  id: number;
  messageCount: number;
}

const props = defineProps<{
  open: boolean;
  filePath: string;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  select: [chats: ChatInfo[]];
}>();

const { t } = useI18n();

// English engineering note.
const isOpen = computed({
  get: () => props.open,
  set: (value) => emit("update:open", value),
});

// English engineering note.
const chats = ref<ChatInfo[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const selectedIndexes = ref<Set<number>>(new Set());

// English engineering note.
const selectedCount = computed(() => selectedIndexes.value.size);

// English engineering note.
const isAllSelected = computed(
  () =>
    chats.value.length > 0 && selectedIndexes.value.size === chats.value.length,
);

// English engineering note.

function getChatTypeIcon(type: string): string {
  const t = type.toLowerCase();
  if (
    t.includes("personal") ||
    t.includes("private_chat") ||
    t.includes("bot") ||
    t.includes("saved")
  ) {
    return "i-heroicons-user";
  }
  if (
    t.includes("group") ||
    t.includes("supergroup") ||
    t.includes("channel")
  ) {
    return "i-heroicons-user-group";
  }
  return "i-heroicons-chat-bubble-left-right";
}

function formatTypeLabel(type: string): string {
  return type.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

// English engineering note.

async function scan() {
  loading.value = true;
  error.value = null;
  chats.value = [];
  selectedIndexes.value = new Set();

  try {
    const result = await window.chatApi.scanMultiChatFile(props.filePath);
    if (result.success) {
      chats.value = result.chats;
    } else {
      error.value = result.error || t("home.chatSelector.scanFailed");
    }
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

// English engineering note.
watch(
  () => props.open,
  (val) => {
    if (val && props.filePath) {
      scan();
    }
  },
);

// English engineering note.

function toggleSelect(index: number) {
  const newSet = new Set(selectedIndexes.value);
  if (newSet.has(index)) {
    newSet.delete(index);
  } else {
    newSet.add(index);
  }
  selectedIndexes.value = newSet;
}

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedIndexes.value = new Set();
  } else {
    selectedIndexes.value = new Set(chats.value.map((c) => c.index));
  }
}

function confirmSelection() {
  const selected = chats.value.filter((c) =>
    selectedIndexes.value.has(c.index),
  );
  isOpen.value = false;
  emit("select", selected);
}

function handleClose() {
  isOpen.value = false;
}
</script>

<template>
  <UModal v-model:open="isOpen" :title="t('home.chatSelector.title')">
    <template #body>
      <div class="xeno-chat-selector min-h-[200px]">
        <!-- English UI note -->
        <div
          v-if="loading"
          class="xeno-chat-selector-state flex flex-col items-center justify-center py-12"
        >
          <UIcon
            name="i-heroicons-arrow-path"
            class="mb-4 h-8 w-8 animate-spin text-pink-500"
          />
          <p class="text-gray-500 dark:text-gray-400">
            {{ t("home.chatSelector.scanning") }}
          </p>
        </div>

        <!-- English UI note -->
        <div
          v-else-if="error"
          class="xeno-chat-selector-state flex flex-col items-center justify-center py-12"
        >
          <UIcon
            name="i-heroicons-exclamation-circle"
            class="mb-4 h-8 w-8 text-red-500"
          />
          <p class="text-red-600 dark:text-red-400">{{ error }}</p>
          <UButton class="mt-4" size="sm" variant="soft" @click="scan">
            {{ t("home.chatSelector.retry") }}
          </UButton>
        </div>

        <!-- English UI note -->
        <div v-else-if="chats.length > 0">
          <!-- English UI note -->
          <div
            class="xeno-chat-selector-toolbar mb-2 flex items-center justify-between"
          >
            <div class="flex items-center gap-2">
              <UCheckbox
                :model-value="isAllSelected"
                :label="t('home.chatSelector.selectAll')"
                size="sm"
                @update:model-value="toggleSelectAll"
              />
              <span class="text-xs text-gray-400">
                ({{
                  t("home.chatSelector.chatCount", { count: chats.length })
                }})
              </span>
            </div>
            <span
              v-if="selectedCount > 0"
              class="text-sm font-medium text-pink-600 dark:text-pink-400"
            >
              {{ t("home.chatSelector.selected", { count: selectedCount }) }}
            </span>
          </div>

          <!-- English UI note -->
          <div
            class="xeno-chat-selector-list max-h-[420px] space-y-0.5 overflow-y-auto pr-1"
          >
            <div
              v-for="chat in chats"
              :key="chat.index"
              class="xeno-chat-selector-row flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-1.5 transition-colors"
              :class="
                selectedIndexes.has(chat.index)
                  ? 'bg-pink-50 dark:bg-pink-500/10'
                  : 'hover:bg-gray-100 dark:hover:bg-gray-800'
              "
              @click="toggleSelect(chat.index)"
            >
              <UCheckbox
                :model-value="selectedIndexes.has(chat.index)"
                size="sm"
                @click.stop
              />
              <div
                class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md"
                :class="
                  selectedIndexes.has(chat.index)
                    ? 'bg-pink-100 dark:bg-pink-500/20'
                    : 'bg-gray-100 dark:bg-gray-700'
                "
              >
                <UIcon
                  :name="getChatTypeIcon(chat.type)"
                  class="h-3.5 w-3.5"
                  :class="
                    selectedIndexes.has(chat.index)
                      ? 'text-pink-600 dark:text-pink-400'
                      : 'text-gray-500 dark:text-gray-400'
                  "
                />
              </div>
              <div class="min-w-0 flex-1">
                <p
                  class="truncate text-sm font-medium text-gray-900 dark:text-white"
                >
                  {{ chat.name || `Chat ${chat.id}` }}
                </p>
                <p class="text-xs text-gray-500 dark:text-gray-400">
                  {{ formatTypeLabel(chat.type) }} ·
                  {{
                    t("home.chatSelector.messageCount", {
                      count: chat.messageCount.toLocaleString(),
                    })
                  }}
                </p>
              </div>
            </div>
          </div>
        </div>

        <!-- English UI note -->
        <div
          v-else
          class="xeno-chat-selector-state flex flex-col items-center justify-center py-12"
        >
          <UIcon
            name="i-heroicons-chat-bubble-left-right"
            class="mb-4 h-8 w-8 text-gray-400"
          />
          <p class="text-gray-500 dark:text-gray-400">
            {{ t("home.chatSelector.noChats") }}
          </p>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton variant="ghost" color="neutral" @click="handleClose">
          {{ t("common.cancel") }}
        </UButton>
        <UButton :disabled="selectedCount === 0" @click="confirmSelection">
          {{ t("home.chatSelector.import", { count: selectedCount }) }}
        </UButton>
      </div>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-chat-selector {
  color: var(--xeno-text-main);
}

.xeno-chat-selector-state,
.xeno-chat-selector-toolbar,
.xeno-chat-selector-list {
  border: 1px solid var(--xeno-border-soft);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 130%),
    var(--xeno-surface-muted);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.xeno-chat-selector-state,
.xeno-chat-selector-toolbar {
  border-radius: 1rem;
  padding: 0.8rem 0.9rem;
}

.xeno-chat-selector-list {
  border-radius: 1rem;
  padding: 0.4rem;
}

.xeno-chat-selector-row {
  position: relative;
}

.xeno-chat-selector-row::before {
  content: "";
  position: absolute;
  left: 0;
  top: 20%;
  bottom: 20%;
  width: 2px;
  border-radius: 999px;
  background: linear-gradient(
    180deg,
    rgba(111, 218, 255, 0),
    rgba(111, 218, 255, 0.72),
    rgba(111, 218, 255, 0)
  );
  opacity: 0;
  transition: opacity 0.2s ease;
}

.xeno-chat-selector-row:hover::before {
  opacity: 0.85;
}
</style>
