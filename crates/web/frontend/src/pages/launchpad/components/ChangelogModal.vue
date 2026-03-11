<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { storeToRefs } from "pinia";
import { useSettingsStore } from "@/stores/settings";
import { sanitizeSummary } from "@/utils/sanitizeSummary";

const { t } = useI18n();
const settingsStore = useSettingsStore();
const { locale } = storeToRefs(settingsStore);

// English engineering note.
const showModal = ref(false);

// English engineering note.
const isLoading = ref(false);
const loadError = ref<string | null>(null);

// English engineering note.
// English engineering note.
const expandedState = ref<Map<string, boolean>>(new Map());

// English engineering note.
const currentAppVersion = ref<string | null>(null);

// English engineering note.
const CHANGELOG_READ_KEY = "xenobot_changelog_read_version";
// English engineering note.
const AGREEMENT_KEY = "xenobot_agreement_version";

// English engineering note.
const SUMMARY_SANITIZE_OPTIONS = {
  allowedTags: ["br", "a", "img"],
  allowedAttrs: {
    a: ["href", "target", "rel"],
    img: ["src", "alt", "title", "width", "height"],
  },
};

// English engineering note.
function toggleVersion(version: string, index: number) {
  const currentState = isExpanded(version, index);
  expandedState.value.set(version, !currentState);
}

// English engineering note.
function normalizeVersion(version?: string | null) {
  return version ? version.trim().replace(/^v/i, "") : null;
}

// English engineering note.
function isExpanded(version: string, index: number) {
  // English engineering note.
  if (expandedState.value.has(version)) {
    return expandedState.value.get(version)!;
  }
  // English engineering note.
  if (currentAppVersion.value) {
    return isCurrentVersion(version);
  }
  // English engineering note.
  return index === 0;
}

// English engineering note.
function isCurrentVersion(version: string) {
  const current = normalizeVersion(currentAppVersion.value);
  return current ? normalizeVersion(version) === current : false;
}

// English engineering note.
interface ChangelogItem {
  version: string;
  date: string;
  summary: string;
  changes: {
    type: "feat" | "fix" | "chore" | "style";
    items: string[];
  }[];
}

// English engineering note.
const changelogs = ref<ChangelogItem[]>([]);

// English engineering note.
function getChangelogUrl(lang: string) {
  const langPath = lang === "zh-CN" ? "cn" : "en";
  return `https://xenobot.app/${langPath}/changelogs.json`;
}

// English engineering note.
async function fetchChangelogs() {
  isLoading.value = true;
  loadError.value = null;

  try {
    const result = await window.api.app.fetchRemoteConfig(
      getChangelogUrl(locale.value),
    );
    if (!result.success || !result.data) {
      throw new Error(result.error || "Failed to fetch");
    }
    changelogs.value = result.data as ChangelogItem[];
  } catch (error) {
    void error;
    loadError.value = t("home.changelog.loadError");
  } finally {
    isLoading.value = false;
  }
}

// English engineering note.
watch(locale, () => {
  if (showModal.value && changelogs.value.length > 0) {
    fetchChangelogs();
  }
});

// English engineering note.
const changeTypeConfig = {
  feat: {
    icon: "i-heroicons-sparkles",
    color: "text-green-500",
    bgColor: "bg-green-100 dark:bg-green-900/30",
  },
  fix: {
    icon: "i-heroicons-wrench-screwdriver",
    color: "text-amber-500",
    bgColor: "bg-amber-100 dark:bg-amber-900/30",
  },
  chore: {
    icon: "i-heroicons-cog-6-tooth",
    color: "text-gray-500",
    bgColor: "bg-gray-100 dark:bg-gray-700/30",
  },
  style: {
    icon: "i-heroicons-paint-brush",
    color: "text-blue-500",
    bgColor: "bg-blue-100 dark:bg-blue-900/30",
  },
};

// English engineering note.
function getChangeTypeLabel(type: string) {
  const labels: Record<string, string> = {
    feat: t("home.changelog.types.feat"),
    fix: t("home.changelog.types.fix"),
    chore: t("home.changelog.types.chore"),
    style: t("home.changelog.types.style"),
  };
  return labels[type] || type;
}

// English engineering note.
function formatDate(dateStr: string) {
  const date = new Date(dateStr);
  if (locale.value === "zh-CN") {
    return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`;
  }
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

// English engineering note.
function markVersionAsRead(version: string) {
  localStorage.setItem(CHANGELOG_READ_KEY, version);
}

// English engineering note.
async function checkNewVersion() {
  try {
    // English engineering note.
    const acceptedAgreement = localStorage.getItem(AGREEMENT_KEY);
    if (!acceptedAgreement) {
      return;
    }

    // English engineering note.
    const rawVersion = await window.api.app.getVersion();
    const currentVersion = normalizeVersion(rawVersion);
    if (!currentVersion) return;

    // English engineering note.
    const rawReadVersion = localStorage.getItem(CHANGELOG_READ_KEY);
    const readVersion = normalizeVersion(rawReadVersion);

    // English engineering note.
    if (rawReadVersion === null) {
      markVersionAsRead(currentVersion);
      return;
    }

    // English engineering note.
    if (readVersion === currentVersion) {
      return;
    }

    // English engineering note.
    const result = await window.api.app.fetchRemoteConfig(
      getChangelogUrl(locale.value),
    );
    if (!result.success || !result.data) return;

    const data = result.data as ChangelogItem[];
    const latestChangelogVersion = normalizeVersion(data[0]?.version);
    if (!latestChangelogVersion) return;

    // English engineering note.
    // English engineering note.
    if (currentVersion !== latestChangelogVersion) {
      return;
    }

    // English engineering note.
    const currentVersionExists = data.some(
      (log) => normalizeVersion(log.version) === currentVersion,
    );

    // English engineering note.
    if (!currentVersionExists) {
      return;
    }

    // English engineering note.
    // English engineering note.
    // English engineering note.
    setTimeout(() => {
      openWithData(data, currentVersion);
      // English engineering note.
      markVersionAsRead(currentVersion);
    }, 500);
  } catch (error) {
    void error;
  }
}

// English engineering note.

// English engineering note.
async function open() {
  // English engineering note.
  try {
    currentAppVersion.value = normalizeVersion(
      await window.api.app.getVersion(),
    );
  } catch {
    currentAppVersion.value = null;
  }
  expandedState.value.clear();
  showModal.value = true;
  // English engineering note.
  if (changelogs.value.length === 0) {
    fetchChangelogs();
  }
}

// English engineering note.
function openWithData(data: ChangelogItem[], appVersion?: string) {
  changelogs.value = data;
  currentAppVersion.value = appVersion || null;
  expandedState.value.clear(); // English engineering note.
  showModal.value = true;
}

function close() {
  showModal.value = false;
}

// English engineering note.
function getLatestVersion() {
  return changelogs.value[0]?.version || null;
}

// English engineering note.
onMounted(() => {
  checkNewVersion();
});

defineExpose({ open, openWithData, close, fetchChangelogs, getLatestVersion });
</script>

<template>
  <UModal
    :open="showModal"
    :title="t('home.changelog.title')"
    :description="t('home.changelog.subtitle')"
    :ui="{ content: 'max-w-3xl' }"
    @update:open="showModal = $event"
  >
    <template #content>
      <div class="xeno-changelog-shell flex max-h-[80vh] flex-col">
        <!-- Header -->
        <div
          class="xeno-changelog-header flex items-center justify-between px-6 py-4"
        >
          <div class="flex items-center gap-3">
            <div
              class="xeno-changelog-icon flex h-10 w-10 items-center justify-center rounded-xl"
            >
              <UIcon
                name="i-heroicons-document-text"
                class="h-5 w-5 text-white"
              />
            </div>
            <div class="min-w-0">
              <h2
                class="break-words text-lg font-semibold text-gray-900 dark:text-white"
              >
                {{ t("home.changelog.title") }}
              </h2>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                {{ t("home.changelog.subtitle") }}
              </p>
            </div>
          </div>
          <UButton
            color="neutral"
            variant="ghost"
            icon="i-heroicons-x-mark"
            @click="close"
          />
        </div>

        <!-- Content -->
        <div class="flex-1 overflow-y-auto px-6 py-4">
          <!-- Loading State -->
          <div v-if="isLoading" class="flex items-center justify-center py-12">
            <UIcon
              name="i-heroicons-arrow-path"
              class="h-6 w-6 animate-spin text-gray-400"
            />
          </div>

          <!-- Error State -->
          <div
            v-else-if="loadError"
            class="flex flex-col items-center justify-center py-12 text-center"
          >
            <UIcon
              name="i-heroicons-exclamation-circle"
              class="h-10 w-10 text-red-400 mb-3"
            />
            <p class="text-sm text-gray-500 dark:text-gray-400">
              {{ loadError }}
            </p>
            <UButton
              color="primary"
              variant="soft"
              size="sm"
              class="mt-3"
              @click="fetchChangelogs"
            >
              {{ t("home.changelog.retry") }}
            </UButton>
          </div>

          <!-- Changelog List -->
          <div v-else class="space-y-6">
            <div
              v-for="(log, index) in changelogs"
              :key="log.version"
              class="relative"
            >
              <!-- Timeline line -->
              <div
                v-if="index < changelogs.length - 1"
                class="absolute left-[15px] top-10 h-[calc(100%-20px)] w-[2px] bg-gray-200 dark:bg-gray-700"
              />

              <!-- Version header -->
              <div class="flex items-start gap-4">
                <!-- Version badge -->
                <div
                  class="relative z-10 flex h-8 w-8 shrink-0 items-center justify-center rounded-full"
                  :class="
                    index === 0 ? 'bg-pink-500' : 'bg-gray-300 dark:bg-gray-600'
                  "
                >
                  <UIcon
                    :name="index === 0 ? 'i-heroicons-star' : 'i-heroicons-tag'"
                    class="h-4 w-4"
                    :class="
                      index === 0
                        ? 'text-white'
                        : 'text-gray-600 dark:text-gray-300'
                    "
                  />
                </div>

                <!-- Version info -->
                <div class="min-w-0 flex-1 pt-0.5">
                  <!-- Clickable header -->
                  <div
                    class="cursor-pointer select-none"
                    @click="toggleVersion(log.version, index)"
                  >
                    <div class="flex flex-wrap items-center gap-3">
                      <h3
                        class="break-all text-base font-bold text-gray-900 dark:text-white"
                      >
                        v{{ log.version }}
                      </h3>
                      <span
                        v-if="index === 0"
                        class="rounded-full bg-pink-100 px-2 py-0.5 text-xs font-medium text-pink-600 dark:bg-pink-900/30 dark:text-pink-400"
                      >
                        {{ t("home.changelog.latest") }}
                      </span>
                      <!-- English UI note -->
                      <span
                        v-if="isCurrentVersion(log.version)"
                        class="rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-600 dark:bg-green-900/30 dark:text-green-400"
                      >
                        {{ t("home.changelog.current") }}
                      </span>
                      <!-- Expand/Collapse indicator -->
                      <UIcon
                        name="i-heroicons-chevron-down"
                        class="h-4 w-4 text-gray-400 transition-transform duration-200"
                        :class="{
                          'rotate-180': isExpanded(log.version, index),
                        }"
                      />
                    </div>
                    <p class="mt-0.5 text-sm text-gray-500 dark:text-gray-400">
                      {{ formatDate(log.date) }}
                    </p>
                    <p
                      class="mt-2 break-words text-sm font-medium text-gray-700 dark:text-gray-300"
                      v-html="
                        sanitizeSummary(log.summary, SUMMARY_SANITIZE_OPTIONS)
                      "
                    />
                  </div>

                  <!-- Changes (collapsible) -->
                  <div
                    v-show="isExpanded(log.version, index)"
                    class="mt-3 space-y-3"
                  >
                    <div
                      v-for="change in log.changes"
                      :key="change.type"
                      class="xeno-changelog-block rounded-lg p-3"
                    >
                      <!-- Change type header -->
                      <div class="mb-2 flex items-center gap-2">
                        <div
                          class="flex h-5 w-5 items-center justify-center rounded"
                          :class="changeTypeConfig[change.type]?.bgColor"
                        >
                          <UIcon
                            :name="changeTypeConfig[change.type]?.icon"
                            class="h-3 w-3"
                            :class="changeTypeConfig[change.type]?.color"
                          />
                        </div>
                        <span
                          class="text-xs font-medium text-gray-600 dark:text-gray-400"
                        >
                          {{ getChangeTypeLabel(change.type) }}
                        </span>
                      </div>
                      <!-- Change items -->
                      <ul class="space-y-1.5 pl-7">
                        <li
                          v-for="(item, idx) in change.items"
                          :key="idx"
                          class="relative break-words text-sm text-gray-600 dark:text-gray-400"
                        >
                          <span
                            class="absolute -left-4 top-2 h-1.5 w-1.5 rounded-full bg-gray-300 dark:bg-gray-600"
                          />
                          {{ item }}
                        </li>
                      </ul>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <div class="xeno-changelog-footer px-6 py-4">
          <div class="flex items-center justify-between">
            <p
              v-if="changelogs.length > 0"
              class="text-sm text-gray-500 dark:text-gray-400"
            >
              {{ t("home.changelog.total", { count: changelogs.length }) }}
            </p>
            <span v-else />
            <UButton color="primary" variant="soft" @click="close">
              {{ t("home.changelog.close") }}
            </UButton>
          </div>
        </div>
      </div>
    </template>
  </UModal>
</template>

<style scoped>
.xeno-changelog-shell {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.6rem;
  background:
    radial-gradient(
      circle at top left,
      rgba(255, 122, 172, 0.14),
      transparent 28%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.05), transparent 24%),
    rgba(7, 18, 29, 0.94);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    0 32px 74px rgba(2, 8, 16, 0.36);
  backdrop-filter: blur(22px) saturate(132%);
}

.xeno-changelog-header {
  border-bottom: 1px solid rgba(139, 166, 189, 0.16);
}

.xeno-changelog-icon {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    linear-gradient(135deg, rgba(255, 122, 172, 0.92), rgba(219, 39, 119, 0.9));
  border: 1px solid rgba(255, 163, 201, 0.22);
  box-shadow: 0 14px 32px rgba(21, 7, 15, 0.24);
}

.xeno-changelog-block {
  border: 1px solid rgba(139, 166, 189, 0.14);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(7, 17, 26, 0.58);
}

.xeno-changelog-footer {
  border-top: 1px solid rgba(139, 166, 189, 0.16);
}
</style>
