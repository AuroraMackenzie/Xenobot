<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import type { TableSchema } from "./types";
import { getTableLabel, getColumnLabel } from "./types";
import type { LocaleType } from "@/i18n/types";

const { t, locale } = useI18n();

// Props
const props = defineProps<{
  sessionId: string;
}>();

// Emits
const emit = defineEmits<{
  insertColumn: [tableName: string, columnName: string];
}>();

// English engineering note.
const isCollapsed = ref(false);
const schema = ref<TableSchema[]>([]);
const expandedTables = ref<Set<string>>(new Set());

// English engineering note.
async function loadSchema() {
  try {
    schema.value = await window.chatApi.getSchema(props.sessionId);
    // English engineering note.
    schema.value.forEach((table) => expandedTables.value.add(table.name));
  } catch (err) {
    console.error("[SchemaPanel] Failed to load schema:", err);
  }
}

// English engineering note.
function toggleTable(tableName: string) {
  if (expandedTables.value.has(tableName)) {
    expandedTables.value.delete(tableName);
  } else {
    expandedTables.value.add(tableName);
  }
}

// English engineering note.
function expandTable(tableName: string) {
  isCollapsed.value = false;
  expandedTables.value.add(tableName);
}

// English engineering note.
function handleInsertColumn(tableName: string, columnName: string) {
  emit("insertColumn", tableName, columnName);
}

// English engineering note.
defineExpose({
  loadSchema,
  schema,
});

onMounted(() => {
  loadSchema();
});
</script>

<template>
  <div
    class="xeno-schema-shell flex flex-col border-r border-gray-200 bg-white transition-all dark:border-gray-800 dark:bg-gray-900"
    :class="isCollapsed ? 'w-10' : 'w-56'"
  >
    <!-- English UI note -->
    <div
      class="flex items-center justify-between border-b border-gray-200 p-2 dark:border-gray-800"
    >
      <span
        v-if="!isCollapsed"
        class="text-xs font-medium text-gray-500 dark:text-gray-400"
      >
        {{ t("ai.sqlLab.schema.tables") }}
      </span>
      <button
        class="rounded p-1 text-gray-400 transition-colors hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800 dark:hover:text-gray-300"
        @click="isCollapsed = !isCollapsed"
      >
        <UIcon
          :name="
            isCollapsed
              ? 'i-heroicons-chevron-right'
              : 'i-heroicons-chevron-left'
          "
          class="h-4 w-4"
        />
      </button>
    </div>

    <!-- English UI note -->
    <div v-if="!isCollapsed" class="flex-1 overflow-y-auto p-2">
      <div v-for="table in schema" :key="table.name" class="mb-2">
        <!-- English UI note -->
        <button
          class="flex w-full items-center gap-1 rounded px-2 py-1.5 text-left transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
          @click="toggleTable(table.name)"
        >
          <UIcon
            :name="
              expandedTables.has(table.name)
                ? 'i-heroicons-chevron-down'
                : 'i-heroicons-chevron-right'
            "
            class="h-3 w-3 shrink-0 text-gray-400"
          />
          <UIcon
            name="i-heroicons-table-cells"
            class="h-4 w-4 shrink-0 text-pink-500"
          />
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{{
            table.name
          }}</span>
          <span class="flex-1 truncate text-right text-xs text-gray-400">
            {{ getTableLabel(table.name, locale as LocaleType) }}
          </span>
        </button>

        <!-- English UI note -->
        <div
          v-if="expandedTables.has(table.name)"
          class="ml-4 mt-1 space-y-0.5"
        >
          <button
            v-for="column in table.columns"
            :key="column.name"
            class="flex w-full items-center gap-2 rounded px-2 py-1 text-left text-xs transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
            :title="t('ai.sqlLab.schema.doubleClickToInsert')"
            @dblclick="handleInsertColumn(table.name, column.name)"
          >
            <UIcon
              v-if="column.pk"
              name="i-heroicons-key"
              class="h-3 w-3 shrink-0 text-yellow-500"
              :title="t('ai.sqlLab.schema.primaryKey')"
            />
            <span class="font-mono text-gray-700 dark:text-gray-300">{{
              column.name
            }}</span>
            <span class="flex-1 truncate text-right text-[10px] text-gray-400">
              {{
                getColumnLabel(table.name, column.name, locale as LocaleType)
              }}
            </span>
          </button>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <div
      v-else
      class="flex flex-1 flex-col items-center gap-2 overflow-y-auto py-2"
    >
      <button
        v-for="table in schema"
        :key="table.name"
        class="rounded p-1 text-gray-400 transition-colors hover:bg-gray-100 hover:text-pink-500 dark:hover:bg-gray-800"
        :title="`${getTableLabel(table.name, locale as LocaleType)} (${table.name})`"
        @click="expandTable(table.name)"
      >
        <UIcon name="i-heroicons-table-cells" class="h-4 w-4" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.xeno-schema-shell {
  background:
    radial-gradient(circle at top, rgba(244, 114, 182, 0.12), transparent 46%),
    linear-gradient(180deg, rgba(15, 23, 42, 0.9), rgba(2, 6, 23, 0.96));
  backdrop-filter: blur(18px);
}
</style>
