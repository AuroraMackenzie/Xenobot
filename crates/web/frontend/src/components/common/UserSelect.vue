<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { MemberWithStats } from "@/types/analysis";

const { t } = useI18n();

// Props
const props = defineProps<{
  sessionId: string;
  modelValue: number | null;
}>();

// Emits
const emit = defineEmits<{
  (e: "update:modelValue", value: number | null): void;
}>();

// English engineering note.
const members = ref<MemberWithStats[]>([]);
const isLoading = ref(false);

// English engineering note.
const ALL_MEMBERS_VALUE = "__ALL__";

// English engineering note.
const internalValue = computed({
  get: () => {
    return props.modelValue === null
      ? ALL_MEMBERS_VALUE
      : String(props.modelValue);
  },
  set: (val: string) => {
    emit("update:modelValue", val === ALL_MEMBERS_VALUE ? null : parseInt(val));
  },
});

// English engineering note.
const memberOptions = computed(() => {
  const options: { value: string; label: string }[] = [
    { value: ALL_MEMBERS_VALUE, label: t("common.userSelect.allMembers") },
  ];
  members.value.forEach((m) => {
    const displayName = m.groupNickname || m.accountName || m.platformId;
    options.push({
      value: String(m.id),
      label: `${displayName} (${m.messageCount})`,
    });
  });
  return options;
});

// English engineering note.
async function loadMembers() {
  if (!props.sessionId) return;
  isLoading.value = true;
  try {
    const result = await window.chatApi.getMembers(props.sessionId);
    // English engineering note.
    members.value = result.sort((a, b) => b.messageCount - a.messageCount);
  } catch (error) {
    console.error("加载成员列表失败:", error);
  } finally {
    isLoading.value = false;
  }
}

// English engineering note.
watch(
  () => props.sessionId,
  () => {
    emit("update:modelValue", null); // English engineering note.
    loadMembers();
  },
  { immediate: true },
);
</script>

<template>
  <USelectMenu
    v-model="internalValue"
    :items="memberOptions"
    :loading="isLoading"
    :virtualize="{ estimateSize: 32, overscan: 10 }"
    value-key="value"
    class="w-48"
  />
</template>
