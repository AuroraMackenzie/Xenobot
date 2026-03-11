<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import type { MemberWithStats } from "@/types/analysis";
import OwnerSelector from "@/components/analysis/member/OwnerSelector.vue";

const { t } = useI18n();

// Props
const props = defineProps<{
  sessionId: string;
}>();

// English engineering note.
const members = ref<MemberWithStats[]>([]);
const isLoading = ref(false);

// English engineering note.
const savingAliasesId = ref<number | null>(null);

// English engineering note.
function getDisplayName(member: MemberWithStats): string {
  return member.groupNickname || member.accountName || member.platformId;
}

// English engineering note.
function getFirstChar(member: MemberWithStats): string {
  const name = getDisplayName(member);
  return name.slice(0, 1);
}

// English engineering note.
const totalMessageCount = computed(() => {
  return members.value.reduce((sum, m) => sum + m.messageCount, 0);
});

// English engineering note.
function getPercentage(count: number): number {
  if (totalMessageCount.value === 0) return 0;
  return Math.round((count / totalMessageCount.value) * 100);
}

// English engineering note.
async function loadMembers() {
  if (!props.sessionId) return;
  isLoading.value = true;
  try {
    members.value = await window.chatApi.getMembers(props.sessionId);
  } catch (error) {
    console.error("[DirectSpaceMemberTab] Failed to load members:", error);
  } finally {
    isLoading.value = false;
  }
}

// English engineering note.
async function updateAliases(member: MemberWithStats, newAliases: string[]) {
  const aliasesToSave = JSON.parse(JSON.stringify(newAliases)) as string[];

  const currentAliases = JSON.stringify(member.aliases);
  const newAliasesStr = JSON.stringify(aliasesToSave);
  if (currentAliases === newAliasesStr) return;

  savingAliasesId.value = member.id;
  try {
    const success = await window.chatApi.updateMemberAliases(
      props.sessionId,
      member.id,
      aliasesToSave,
    );
    if (success) {
      const idx = members.value.findIndex((m) => m.id === member.id);
      if (idx !== -1) {
        members.value[idx] = {
          ...members.value[idx],
          aliases: aliasesToSave,
        };
      }
    }
  } catch (error) {
    console.error("[DirectSpaceMemberTab] Failed to save aliases:", error);
  } finally {
    savingAliasesId.value = null;
  }
}

// English engineering note.
watch(
  () => props.sessionId,
  () => {
    loadMembers();
  },
  { immediate: true },
);

onMounted(() => {
  loadMembers();
});
</script>

<template>
  <div class="main-content max-w-4xl p-6">
    <!-- English UI note -->
    <div class="mb-6">
      <div class="flex items-center gap-3">
        <div>
          <h2 class="text-xl font-bold text-gray-900 dark:text-white">
            {{ t("members.private.title") }}
          </h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {{ t("members.private.description", { count: members.length }) }}
          </p>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <OwnerSelector
      class="mb-6"
      :session-id="sessionId"
      :members="members"
      :is-loading="isLoading"
      chat-type="private"
    />

    <!-- English UI note -->
    <div v-if="isLoading" class="flex h-60 items-center justify-center">
      <UIcon
        name="i-heroicons-arrow-path"
        class="h-8 w-8 animate-spin text-pink-500"
      />
    </div>

    <!-- English UI note -->
    <div v-else class="grid gap-4 md:grid-cols-2">
      <div
        v-for="member in members"
        :key="member.id"
        class="xeno-member-card rounded-2xl p-5"
      >
        <!-- English UI note -->
        <div class="flex items-start gap-4">
          <!-- English UI note -->
          <img
            v-if="member.avatar"
            :src="member.avatar"
            :alt="getDisplayName(member)"
            class="h-14 w-14 shrink-0 rounded-full object-cover"
          />
          <div
            v-else
            class="flex h-14 w-14 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-pink-400 to-pink-600 text-lg font-medium text-white"
          >
            {{ getFirstChar(member) }}
          </div>

          <!-- English UI note -->
          <div class="flex-1 min-w-0">
            <h3
              class="text-lg font-semibold text-gray-900 dark:text-white truncate"
            >
              {{ getDisplayName(member) }}
            </h3>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              ID: {{ member.platformId }}
            </p>
          </div>
        </div>

        <!-- English UI note -->
        <div class="mt-4 flex items-center gap-4">
          <div class="flex-1">
            <div class="flex items-baseline justify-between">
              <span class="text-sm text-gray-500 dark:text-gray-400">{{
                t("members.private.messageCount")
              }}</span>
              <span class="text-lg font-bold text-gray-900 dark:text-white">
                {{ member.messageCount.toLocaleString() }}
              </span>
            </div>
            <!-- English UI note -->
            <div
              class="mt-2 h-2 w-full overflow-hidden rounded-full bg-gray-100 dark:bg-gray-800"
            >
              <div
                class="h-full rounded-full bg-gradient-to-r from-pink-400 to-pink-600 transition-all duration-500"
                :style="{ width: `${getPercentage(member.messageCount)}%` }"
              />
            </div>
            <p class="mt-1 text-xs text-gray-400 dark:text-gray-500">
              {{
                t("members.private.percentage", {
                  value: getPercentage(member.messageCount),
                })
              }}
            </p>
          </div>
        </div>

        <!-- English UI note -->
        <div class="mt-4 border-t border-gray-100 pt-4 dark:border-gray-800">
          <label
            class="mb-2 block text-sm font-medium text-gray-700 dark:text-gray-300"
          >
            {{ t("members.private.customAlias") }}
          </label>
          <div class="relative">
            <UInputTags
              :model-value="member.aliases"
              :placeholder="t('members.private.aliasPlaceholder')"
              class="w-full"
              @update:model-value="(val) => updateAliases(member, val)"
            />
            <!-- English UI note -->
            <div
              v-if="savingAliasesId === member.id"
              class="absolute right-3 top-1/2 -translate-y-1/2"
            >
              <UIcon
                name="i-heroicons-arrow-path"
                class="h-4 w-4 animate-spin text-pink-500"
              />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <div
      v-if="!isLoading && members.length === 0"
      class="flex h-60 flex-col items-center justify-center"
    >
      <UIcon
        name="i-heroicons-user-group"
        class="mb-3 h-12 w-12 text-gray-300 dark:text-gray-600"
      />
      <p class="text-gray-500 dark:text-gray-400">
        {{ t("members.private.empty") }}
      </p>
    </div>

    <!-- English UI note -->
    <div
      v-if="members.length > 0"
      class="xeno-member-tip mt-6 flex items-start gap-3 rounded-2xl p-4"
    >
      <UIcon
        name="i-heroicons-information-circle"
        class="mt-0.5 h-5 w-5 shrink-0 text-blue-500"
      />
      <div>
        <p class="text-sm font-medium text-blue-800 dark:text-blue-200">
          {{ t("members.private.tipTitle") }}
        </p>
        <p class="mt-1 text-sm text-blue-700 dark:text-blue-300">
          {{ t("members.private.tipContent") }}
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-member-card,
.xeno-member-tip {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background:
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 24%
    ),
    linear-gradient(180deg, rgba(15, 23, 42, 0.74), rgba(15, 23, 42, 0.62));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 16px 34px rgba(2, 6, 23, 0.16);
  backdrop-filter: blur(18px);
}

.xeno-member-tip {
  background: linear-gradient(
    180deg,
    rgba(30, 64, 175, 0.18),
    rgba(15, 23, 42, 0.62)
  );
}
</style>
