<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

// English engineering note.
interface SessionIndexStatus {
  id: string;
  name: string;
  hasIndex: boolean;
  sessionCount: number;
}

// English engineering note.
const DEFAULT_GAP_MINUTES = 30; // English engineering note.
const sessionGapMinutes = ref(DEFAULT_GAP_MINUTES);

// English engineering note.
const allSessionsStatus = ref<SessionIndexStatus[]>([]);
const isLoadingSessionStatus = ref(false);
const isBatchGenerating = ref(false);
const batchProgress = ref({ current: 0, total: 0, currentName: "" });

// English engineering note.
const sessionIndexStats = computed(() => {
  const total = allSessionsStatus.value.length;
  const generated = allSessionsStatus.value.filter((s) => s.hasIndex).length;
  const notGenerated = total - generated;
  return { total, generated, notGenerated };
});

// English engineering note.
const batchProgressPercent = computed(() => {
  if (batchProgress.value.total === 0) return 0;
  return Math.round(
    (batchProgress.value.current / batchProgress.value.total) * 100,
  );
});

// English engineering note.
function saveSessionThreshold() {
  if (sessionGapMinutes.value < 1) sessionGapMinutes.value = 1;
  if (sessionGapMinutes.value > 1440) sessionGapMinutes.value = 1440;
  // English engineering note.
  localStorage.setItem(
    "sessionGapThreshold",
    String(sessionGapMinutes.value * 60),
  );
}

// English engineering note.
function loadSessionThreshold() {
  const saved = localStorage.getItem("sessionGapThreshold");
  if (saved) {
    sessionGapMinutes.value = Math.round(parseInt(saved, 10) / 60);
  }
}

// English engineering note.
async function loadSessionIndexStatus() {
  isLoadingSessionStatus.value = true;
  try {
    // English engineering note.
    const sessions = await window.chatApi.getSessions();

    // English engineering note.
    const statusList: SessionIndexStatus[] = [];
    for (const session of sessions) {
      try {
        const stats = await window.sessionApi.getStats(session.id);
        statusList.push({
          id: session.id,
          name: session.name,
          hasIndex: stats.hasIndex,
          sessionCount: stats.sessionCount,
        });
      } catch {
        statusList.push({
          id: session.id,
          name: session.name,
          hasIndex: false,
          sessionCount: 0,
        });
      }
    }

    allSessionsStatus.value = statusList;
  } catch (error) {
    console.error(
      "[SessionIndexSection] Failed to load session index status:",
      error,
    );
  } finally {
    isLoadingSessionStatus.value = false;
  }
}

// English engineering note.
async function batchGenerateIndex() {
  const notGeneratedSessions = allSessionsStatus.value.filter(
    (s) => !s.hasIndex,
  );
  if (notGeneratedSessions.length === 0) return;

  isBatchGenerating.value = true;
  batchProgress.value = {
    current: 0,
    total: notGeneratedSessions.length,
    currentName: "",
  };

  // English engineering note.
  const gapThreshold = sessionGapMinutes.value * 60;

  for (let i = 0; i < notGeneratedSessions.length; i++) {
    const session = notGeneratedSessions[i];
    batchProgress.value = {
      current: i,
      total: notGeneratedSessions.length,
      currentName: session.name,
    };

    try {
      const count = await window.sessionApi.generate(session.id, gapThreshold);
      // English engineering note.
      const statusItem = allSessionsStatus.value.find(
        (s) => s.id === session.id,
      );
      if (statusItem) {
        statusItem.hasIndex = true;
        statusItem.sessionCount = count;
      }
    } catch (error) {
      console.error(
        `[SessionIndexSection] Failed to generate index for session "${session.name}":`,
        error,
      );
    }
  }

  batchProgress.value = {
    current: notGeneratedSessions.length,
    total: notGeneratedSessions.length,
    currentName: "",
  };
  isBatchGenerating.value = false;
}

// English engineering note.
async function batchRegenerateAll() {
  if (allSessionsStatus.value.length === 0) return;

  isBatchGenerating.value = true;
  batchProgress.value = {
    current: 0,
    total: allSessionsStatus.value.length,
    currentName: "",
  };

  // English engineering note.
  const gapThreshold = sessionGapMinutes.value * 60;

  for (let i = 0; i < allSessionsStatus.value.length; i++) {
    const session = allSessionsStatus.value[i];
    batchProgress.value = {
      current: i,
      total: allSessionsStatus.value.length,
      currentName: session.name,
    };

    try {
      const count = await window.sessionApi.generate(session.id, gapThreshold);
      // English engineering note.
      session.hasIndex = true;
      session.sessionCount = count;
    } catch (error) {
      console.error(
        `[SessionIndexSection] Failed to regenerate index for session "${session.name}":`,
        error,
      );
    }
  }

  batchProgress.value = {
    current: allSessionsStatus.value.length,
    total: allSessionsStatus.value.length,
    currentName: "",
  };
  isBatchGenerating.value = false;
}

// English engineering note.
onMounted(() => {
  loadSessionThreshold();
  loadSessionIndexStatus();
});
</script>

<template>
  <div class="xeno-session-shell space-y-6">
    <!-- English UI note -->
    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <div>
          <h3
            class="flex items-center gap-2 text-sm font-semibold text-gray-900 dark:text-white"
          >
            <UIcon name="i-heroicons-clock" class="h-4 w-4 text-blue-500" />
            {{ t("settings.storage.session.title") }}
          </h3>
          <p class="mt-0.5 text-xs text-gray-500 dark:text-gray-400">
            {{ t("settings.storage.session.description") }}
          </p>
        </div>
        <!-- English UI note -->
        <UButton
          icon="i-heroicons-arrow-path"
          variant="ghost"
          size="xs"
          :loading="isLoadingSessionStatus"
          @click="loadSessionIndexStatus"
        />
      </div>

      <!-- English UI note -->
      <div
        class="xeno-session-card flex items-center justify-between rounded-xl px-4 py-3"
      >
        <div>
          <span class="text-sm text-gray-700 dark:text-gray-300">
            {{ t("settings.storage.session.defaultThreshold") }}
          </span>
          <p class="text-xs text-gray-400">
            {{ t("settings.storage.session.thresholdHelp") }}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <UInput
            v-model.number="sessionGapMinutes"
            type="number"
            :min="1"
            :max="1440"
            size="xs"
            class="w-20"
            @blur="saveSessionThreshold"
          />
          <span class="text-xs text-gray-500">{{
            t("settings.storage.session.thresholdUnit")
          }}</span>
        </div>
      </div>

      <!-- English UI note -->
      <div class="xeno-session-card rounded-xl px-4 py-3">
        <div class="flex items-center justify-between">
          <div>
            <span class="text-sm text-gray-700 dark:text-gray-300">
              {{ t("settings.storage.session.batchTitle") }}
            </span>
            <div
              v-if="!isLoadingSessionStatus"
              class="mt-1 flex items-center gap-3 text-xs"
            >
              <span class="text-gray-500">
                {{
                  t("settings.storage.session.totalSessions", {
                    count: sessionIndexStats.total,
                  })
                }}
              </span>
              <span class="text-green-600 dark:text-green-400">
                {{
                  t("settings.storage.session.generatedCount", {
                    count: sessionIndexStats.generated,
                  })
                }}
              </span>
              <span
                v-if="sessionIndexStats.notGenerated > 0"
                class="text-amber-600 dark:text-amber-400"
              >
                {{
                  t("settings.storage.session.notGeneratedCount", {
                    count: sessionIndexStats.notGenerated,
                  })
                }}
              </span>
            </div>
            <div
              v-else
              class="mt-1 flex items-center gap-1 text-xs text-gray-400"
            >
              <UIcon
                name="i-heroicons-arrow-path"
                class="h-3 w-3 animate-spin"
              />
              {{ t("settings.storage.session.loadingStatus") }}
            </div>
          </div>
          <div class="flex items-center gap-2">
            <UButton
              v-if="sessionIndexStats.notGenerated > 0"
              size="xs"
              color="primary"
              :loading="isBatchGenerating"
              :disabled="isLoadingSessionStatus"
              @click="batchGenerateIndex"
            >
              <UIcon
                v-if="!isBatchGenerating"
                name="i-heroicons-sparkles"
                class="mr-1 h-3 w-3"
              />
              {{ t("settings.storage.session.batchGenerate") }}
            </UButton>
            <UButton
              size="xs"
              variant="soft"
              :loading="isBatchGenerating"
              :disabled="
                isLoadingSessionStatus || sessionIndexStats.total === 0
              "
              @click="batchRegenerateAll"
            >
              <UIcon
                v-if="!isBatchGenerating"
                name="i-heroicons-arrow-path"
                class="mr-1 h-3 w-3"
              />
              {{ t("settings.storage.session.batchRegenerate") }}
            </UButton>
          </div>
        </div>

        <!-- English UI note -->
        <div v-if="isBatchGenerating" class="mt-3 space-y-2">
          <div class="flex items-center justify-between text-xs">
            <span class="text-gray-500">
              {{ t("settings.storage.session.generating") }}
              {{ batchProgress.currentName }}
            </span>
            <span class="font-medium text-gray-700 dark:text-gray-300">
              {{ batchProgress.current }}/{{ batchProgress.total }} ({{
                batchProgressPercent
              }}%)
            </span>
          </div>
          <UProgress :value="batchProgressPercent" size="sm" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-session-shell {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 1.5rem;
  padding: 1rem;
  background:
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 24%
    ),
    linear-gradient(180deg, rgba(15, 23, 42, 0.72), rgba(15, 23, 42, 0.6));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 38px rgba(2, 6, 23, 0.18);
  backdrop-filter: blur(18px);
}

.xeno-session-card {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: linear-gradient(
    180deg,
    rgba(15, 23, 42, 0.58),
    rgba(15, 23, 42, 0.44)
  );
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
}
</style>
