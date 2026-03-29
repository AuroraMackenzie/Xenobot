<script setup lang="ts">
import { ref, onMounted, watch, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import type { AnalysisSession, MessageType } from "@/types/base";
import type {
  MemberActivity,
  HourlyActivity,
  DailyActivity,
} from "@/types/analysis";
import CaptureButton from "@/components/common/CaptureButton.vue";
import TimeSelect from "@/components/common/TimeSelect.vue";
import AITab from "@/components/analysis/AITab.vue";
import OverviewTab from "./components/OverviewTab.vue";
import ViewTab from "./components/ViewTab.vue";
import QuotesTab from "./components/QuotesTab.vue";
import MemberTab from "./components/MemberTab.vue";
import PageHeader from "@/components/layout/PageHeader.vue";
import SessionIndexModal from "@/components/analysis/SessionIndexModal.vue";
import IncrementalImportModal from "@/components/analysis/IncrementalImportModal.vue";
import LoadingState from "@/components/UI/LoadingState.vue";
import { useSessionStore } from "@/stores/session";
import { useLayoutStore } from "@/stores/layout";
import { useTimeSelect } from "@/composables";

const { t } = useI18n();

const route = useRoute();
const router = useRouter();
const sessionStore = useSessionStore();
const layoutStore = useLayoutStore();
const { currentSessionId } = storeToRefs(sessionStore);

// English engineering note.
const showSessionIndexModal = ref(false);

// English engineering note.
const showIncrementalImportModal = ref(false);

// English engineering note.
function openChatRecordViewer() {
  layoutStore.openChatRecordDrawer({});
}

// English engineering note.
const isLoading = ref(true);
const session = ref<AnalysisSession | null>(null);
const memberActivity = ref<MemberActivity[]>([]);
const hourlyActivity = ref<HourlyActivity[]>([]);
const dailyActivity = ref<DailyActivity[]>([]);
const messageTypes = ref<Array<{ type: MessageType; count: number }>>([]);
const isInitialLoad = ref(true);

// English engineering note.
const allTabs = [
  {
    id: "overview",
    labelKey: "analysis.tabs.overview",
    icon: "i-heroicons-chart-pie",
  },
  {
    id: "view",
    labelKey: "analysis.tabs.view",
    icon: "i-heroicons-presentation-chart-bar",
  },
  {
    id: "quotes",
    labelKey: "analysis.tabs.groupQuotes",
    icon: "i-heroicons-chat-bubble-bottom-center-text",
  },
  {
    id: "members",
    labelKey: "analysis.tabs.members",
    icon: "i-heroicons-user-group",
  },
  { id: "ai", labelKey: "analysis.tabs.ai", icon: "i-heroicons-sparkles" },
];

// English engineering note.
const tabs = computed(() => allTabs);

const activeTab = ref((route.query.tab as string) || "overview");

// English engineering note.
const {
  timeRangeValue,
  fullTimeRange,
  availableYears,
  timeFilter,
  selectedYearForOverview,
  initialTimeState,
} = useTimeSelect(route, router, {
  activeTab,
  isInitialLoad,
  currentSessionId,
  onTimeRangeChange: () => loadAnalysisData(),
});

// English engineering note.
const topMembers = computed(() => memberActivity.value.slice(0, 3));
const bottomMembers = computed(() => {
  if (memberActivity.value.length <= 1) return [];
  return [...memberActivity.value]
    .sort((a, b) => a.messageCount - b.messageCount)
    .slice(0, 1);
});

// English engineering note.
const filteredMessageCount = computed(() => {
  return memberActivity.value.reduce((sum, m) => sum + m.messageCount, 0);
});

// English engineering note.
const filteredMemberCount = computed(() => {
  return memberActivity.value.filter((m) => m.messageCount > 0).length;
});

// Sync route param to store
function syncSession() {
  const id = route.params.id as string;
  if (id) {
    sessionStore.selectSession(id);
    // If selection failed (e.g. invalid ID), redirect to home
    if (sessionStore.currentSessionId !== id) {
      router.replace("/");
    }
  }
}

// English engineering note.
async function loadBaseData() {
  if (!currentSessionId.value) return;

  try {
    const sessionData = await window.chatApi.getSession(currentSessionId.value);
    session.value = sessionData;
  } catch (error) {
    console.error("[CircleSpace] Failed to load base data:", error);
  }
}

// English engineering note.
async function loadAnalysisData() {
  if (!currentSessionId.value) return;

  isLoading.value = true;

  try {
    const filter = timeFilter.value;

    const [members, hourly, daily, types] = await Promise.all([
      window.chatApi.getMemberActivity(currentSessionId.value, filter),
      window.chatApi.getHourlyActivity(currentSessionId.value, filter),
      window.chatApi.getDailyActivity(currentSessionId.value, filter),
      window.chatApi.getMessageTypeDistribution(currentSessionId.value, filter),
    ]);

    memberActivity.value = members;
    hourlyActivity.value = hourly;
    dailyActivity.value = daily;
    messageTypes.value = types;
  } catch (error) {
    console.error("[CircleSpace] Failed to load analysis data:", error);
  } finally {
    isLoading.value = false;
  }
}

// English engineering note.
async function loadData() {
  if (!currentSessionId.value) return;

  isInitialLoad.value = true;
  await loadBaseData();
  isInitialLoad.value = false;
}

// English engineering note.
watch(
  () => route.params.id,
  () => {
    // English engineering note.
    // English engineering note.
    // English engineering note.
    // English engineering note.
    if (!route.query.tab) {
      activeTab.value = "overview";
    } else {
      activeTab.value = route.query.tab as string;
    }
    syncSession();
  },
);

// English engineering note.
watch(
  currentSessionId,
  () => {
    loadData();
  },
  { immediate: true },
);

onMounted(() => {
  syncSession();
});
</script>

<template>
  <div
    class="xeno-analysis-shell flex h-full flex-col"
    style="padding-top: var(--titlebar-area-height)"
  >
    <!-- Loading State -->
    <LoadingState
      v-if="isInitialLoad"
      variant="page"
      :text="t('analysis.groupChat.loading')"
    />

    <!-- Content -->
    <template v-else-if="session">
      <!-- Header -->
      <PageHeader
        :title="session.name"
        :description="
          t('analysis.groupChat.description', {
            dateRange: timeRangeValue?.displayLabel ?? '',
            memberCount:
              timeRangeValue?.isFullRange !== false
                ? session.memberCount
                : filteredMemberCount,
            messageCount:
              timeRangeValue?.isFullRange !== false
                ? session.messageCount
                : filteredMessageCount,
          })
        "
        :avatar="session.groupAvatar"
        icon="i-heroicons-chat-bubble-left-right"
        icon-class="xeno-session-badge-group"
      >
        <template #actions>
          <UButton
            color="neutral"
            variant="soft"
            size="sm"
            icon="i-heroicons-plus-circle"
            @click="showIncrementalImportModal = true"
          >
            {{ t("analysis.tooltip.incrementalImport") }}
          </UButton>
          <UButton
            color="primary"
            variant="soft"
            size="sm"
            icon="i-heroicons-chat-bubble-bottom-center-text"
            @click="openChatRecordViewer"
          >
            {{ t("analysis.tooltip.chatViewer") }}
          </UButton>
          <UTooltip :text="t('analysis.tooltip.sessionIndex')">
            <UButton
              icon="i-heroicons-clock"
              color="neutral"
              variant="ghost"
              size="sm"
              @click="showSessionIndexModal = true"
            />
          </UTooltip>
          <CaptureButton />
        </template>
        <!-- Tabs -->
        <div class="mt-4 flex items-center justify-between gap-4">
          <div class="xeno-analysis-tabrail overflow-x-auto scrollbar-hide">
            <button
              v-for="tab in tabs"
              :key="tab.id"
              class="xeno-analysis-tab"
              :class="[
                activeTab === tab.id
                  ? 'xeno-analysis-tab--active xeno-analysis-tab--group'
                  : '',
              ]"
              @click="activeTab = tab.id"
            >
              <UIcon :name="tab.icon" class="h-4 w-4" />
              <span class="whitespace-nowrap">{{ t(tab.labelKey) }}</span>
            </button>
          </div>
          <!-- English UI note -->
          <TimeSelect
            v-model="timeRangeValue"
            :session-id="currentSessionId ?? undefined"
            :visible="activeTab !== 'ai'"
            :initial-state="initialTimeState"
            @update:full-range="fullTimeRange = $event"
            @update:available-years="availableYears = $event"
          />
        </div>
      </PageHeader>

      <!-- Tab Content -->
      <div class="relative flex-1 overflow-y-auto">
        <!-- Loading Overlay -->
        <LoadingState v-if="isLoading" variant="overlay" />

        <div class="h-full">
          <Transition name="tab-slide" mode="out-in">
            <OverviewTab
              v-if="activeTab === 'overview'"
              :key="'overview-' + currentSessionId"
              :session="session"
              :member-activity="memberActivity"
              :top-members="topMembers"
              :bottom-members="bottomMembers"
              :message-types="messageTypes"
              :hourly-activity="hourlyActivity"
              :daily-activity="dailyActivity"
              :time-range="fullTimeRange"
              :selected-year="selectedYearForOverview"
              :filtered-message-count="filteredMessageCount"
              :filtered-member-count="filteredMemberCount"
              :time-filter="timeFilter"
            />
            <ViewTab
              v-else-if="activeTab === 'view'"
              :key="'view-' + currentSessionId"
              :session-id="currentSessionId!"
              :time-filter="timeFilter"
            />
            <QuotesTab
              v-else-if="activeTab === 'quotes'"
              :key="'quotes-' + currentSessionId"
              :session-id="currentSessionId!"
              :time-filter="timeFilter"
            />
            <MemberTab
              v-else-if="activeTab === 'members'"
              :key="'members-' + currentSessionId"
              :session-id="currentSessionId!"
              :time-filter="timeFilter"
              @data-changed="loadData"
            />
            <AITab
              v-else-if="activeTab === 'ai'"
              :key="'ai-' + currentSessionId"
              :session-id="currentSessionId!"
              :session-name="session.name"
              chat-type="group"
            />
          </Transition>
        </div>
      </div>
    </template>

    <!-- Empty State -->
    <div v-else class="xeno-analysis-empty-state">
      <div class="xeno-analysis-empty-panel">
        <p class="xeno-analysis-empty-copy">
          {{ t("analysis.groupChat.loadError") }}
        </p>
      </div>
    </div>

    <!-- English UI note -->
    <SessionIndexModal
      v-if="currentSessionId"
      v-model="showSessionIndexModal"
      :session-id="currentSessionId"
    />

    <!-- English UI note -->
    <IncrementalImportModal
      v-if="currentSessionId && session"
      v-model="showIncrementalImportModal"
      :session-id="currentSessionId"
      :session-name="session.name"
      @imported="loadData"
    />
  </div>
</template>

<style scoped>
.xeno-analysis-shell {
  background: transparent;
  color: var(--xeno-text-main);
}

.tab-slide-enter-active,
.tab-slide-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.tab-slide-enter-from {
  opacity: 0;
  transform: translateY(10px);
}

.tab-slide-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
