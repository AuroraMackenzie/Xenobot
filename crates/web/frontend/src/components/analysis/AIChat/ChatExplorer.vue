<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import { useI18n } from "vue-i18n";
import ConversationList from "./ConversationList.vue";
import DataSourcePanel from "./DataSourcePanel.vue";
import ChatMessage from "./ChatMessage.vue";
import ChatInput from "./ChatInput.vue";
import AIThinkingIndicator from "./AIThinkingIndicator.vue";
import ChatStatusBar from "./ChatStatusBar.vue";
import { useAIChat } from "@/composables/useAIChat";
import CaptureButton from "@/components/common/CaptureButton.vue";
import { usePromptStore } from "@/stores/prompt";
import { useSettingsStore } from "@/stores/settings";

const { t } = useI18n();
const settingsStore = useSettingsStore();

// Props
const props = defineProps<{
  sessionId: string;
  sessionName: string;
  timeFilter?: { startTs: number; endTs: number };
  chatType?: "group" | "private";
}>();

// English engineering note.
const {
  messages,
  sourceMessages,
  currentKeywords,
  isLoadingSource,
  isAIThinking,
  currentConversationId,
  currentToolStatus,
  toolsUsedInCurrentRound,
  sessionTokenUsage,
  sendMessage,
  loadConversation,
  startNewConversation,
  loadMoreSourceMessages,
  updateMaxMessages,
  stopGeneration,
} = useAIChat(
  props.sessionId,
  props.timeFilter,
  props.chatType ?? "group",
  settingsStore.locale,
);

// Store
const promptStore = usePromptStore();

// English engineering note.
const currentChatType = computed(() => props.chatType ?? "group");

// English engineering note.
const isSourcePanelCollapsed = ref(false);
const hasLLMConfig = ref(false);
const isCheckingConfig = ref(true);
const messagesContainer = ref<HTMLElement | null>(null);
const conversationListRef = ref<InstanceType<typeof ConversationList> | null>(
  null,
);

// English engineering note.
const isStickToBottom = ref(true); // English engineering note.
const showScrollToBottom = ref(false); // English engineering note.
const RESTICK_THRESHOLD = 30; // English engineering note.

// English engineering note.
const conversationContentRef = ref<HTMLElement | null>(null);

// English engineering note.
const qaPairs = computed(() => {
  const pairs: Array<{
    user: (typeof messages.value)[0] | null;
    assistant: (typeof messages.value)[0] | null;
    id: string;
  }> = [];
  let currentUser: (typeof messages.value)[0] | null = null;

  for (const msg of messages.value) {
    if (msg.role === "user") {
      // English engineering note.
      if (currentUser) {
        pairs.push({ user: currentUser, assistant: null, id: currentUser.id });
      }
      currentUser = msg;
    } else if (msg.role === "assistant") {
      pairs.push({
        user: currentUser,
        assistant: msg,
        id: currentUser?.id || msg.id,
      });
      currentUser = null;
    }
  }

  // English engineering note.
  if (currentUser) {
    pairs.push({ user: currentUser, assistant: null, id: currentUser.id });
  }

  return pairs;
});

// English engineering note.
async function checkLLMConfig() {
  isCheckingConfig.value = true;
  try {
    hasLLMConfig.value = await window.llmApi.hasConfig();
  } catch (error) {
    console.error("[ChatExplorer] Failed to check LLM configuration:", error);
    hasLLMConfig.value = false;
  } finally {
    isCheckingConfig.value = false;
  }
}

// English engineering note.
async function refreshConfig() {
  await checkLLMConfig();
  if (hasLLMConfig.value) {
    await updateMaxMessages();
  }
  // English engineering note.
  const welcomeMsg = messages.value.find((m) => m.id.startsWith("welcome"));
  if (welcomeMsg) {
    welcomeMsg.content = generateWelcomeMessage();
  }
}

// English engineering note.
defineExpose({
  refreshConfig,
});

// English engineering note.
function generateWelcomeMessage() {
  const configHint = hasLLMConfig.value
    ? t("ai.chat.welcome.configReady")
    : t("ai.chat.welcome.configNeeded");

  return t("ai.chat.welcome.message", {
    sessionName: props.sessionName,
    configHint,
  });
}

// English engineering note.
async function handleSend(content: string) {
  await sendMessage(content);
  // English engineering note.
  scrollToBottom(true);
  // English engineering note.
  conversationListRef.value?.refresh();
}

// English engineering note.
function scrollToBottom(force = false) {
  setTimeout(() => {
    if (messagesContainer.value) {
      // English engineering note.
      if (force || isStickToBottom.value) {
        messagesContainer.value.scrollTop =
          messagesContainer.value.scrollHeight;
        isStickToBottom.value = true;
        showScrollToBottom.value = false;
      }
    }
  }, 100);
}

// English engineering note.
function handleWheel(event: WheelEvent) {
  // English engineering note.
  if (event.deltaY < 0 && isAIThinking.value) {
    // English engineering note.
    isStickToBottom.value = false;
    showScrollToBottom.value = true;
  }
}

// English engineering note.
function checkScrollPosition() {
  if (!messagesContainer.value) return;

  const { scrollTop, scrollHeight, clientHeight } = messagesContainer.value;
  const distanceFromBottom = scrollHeight - scrollTop - clientHeight;

  // English engineering note.
  if (distanceFromBottom < RESTICK_THRESHOLD) {
    isStickToBottom.value = true;
    showScrollToBottom.value = false;
  }
}

// English engineering note.
function handleScrollToBottom() {
  scrollToBottom(true);
}

// English engineering note.
function toggleSourcePanel() {
  isSourcePanelCollapsed.value = !isSourcePanelCollapsed.value;
}

// English engineering note.
async function handleLoadMore() {
  await loadMoreSourceMessages();
}

// English engineering note.
async function handleSelectConversation(convId: string) {
  await loadConversation(convId);
  scrollToBottom(true); // English engineering note.
}

// English engineering note.
function handleCreateConversation() {
  startNewConversation(generateWelcomeMessage());
}

// English engineering note.
function handleDeleteConversation(convId: string) {
  // English engineering note.
  if (currentConversationId.value === convId) {
    startNewConversation(generateWelcomeMessage());
  }
}

// English engineering note.
onMounted(async () => {
  await checkLLMConfig();
  await updateMaxMessages();

  // English engineering note.
  startNewConversation(generateWelcomeMessage());

  // English engineering note.
  if (messagesContainer.value) {
    messagesContainer.value.addEventListener("scroll", checkScrollPosition);
    messagesContainer.value.addEventListener("wheel", handleWheel, {
      passive: true,
    });
  }
});

// English engineering note.
onBeforeUnmount(() => {
  stopGeneration();
  if (messagesContainer.value) {
    messagesContainer.value.removeEventListener("scroll", checkScrollPosition);
    messagesContainer.value.removeEventListener("wheel", handleWheel);
  }
});

// English engineering note.
function handleStop() {
  stopGeneration();
}

// English engineering note.
watch(
  () => messages.value.length,
  () => {
    scrollToBottom();
  },
);

// English engineering note.
watch(
  () => messages.value[messages.value.length - 1]?.content,
  () => {
    scrollToBottom();
  },
);

// English engineering note.
watch(
  () => messages.value[messages.value.length - 1]?.contentBlocks?.length,
  () => {
    scrollToBottom();
  },
);

// English engineering note.
watch(
  () => promptStore.aiConfigVersion,
  async () => {
    await refreshConfig();
  },
);
</script>

<template>
  <div class="main-content xeno-chat-shell flex h-full overflow-hidden">
    <!-- English UI note -->
    <ConversationList
      ref="conversationListRef"
      :session-id="sessionId"
      :active-id="currentConversationId"
      class="h-full shrink-0"
      @select="handleSelectConversation"
      @create="handleCreateConversation"
      @delete="handleDeleteConversation"
    />

    <!-- English UI note -->
    <div class="flex h-full flex-1">
      <div
        class="xeno-chat-main relative flex min-w-[480px] flex-1 flex-col overflow-hidden"
      >
        <!-- English UI note -->
        <div ref="messagesContainer" class="min-h-0 flex-1 overflow-y-auto p-4">
          <div ref="conversationContentRef" class="mx-auto max-w-3xl space-y-4">
            <!-- English UI note -->
            <div
              v-if="qaPairs.length > 0 && !isAIThinking"
              class="flex justify-end"
            >
              <CaptureButton
                :label="t('ai.chat.capture')"
                size="xs"
                type="element"
                :target-element="conversationContentRef"
              />
            </div>

            <!-- English UI note -->
            <template v-for="pair in qaPairs" :key="pair.id">
              <div class="qa-pair space-y-4">
                <!-- English UI note -->
                <ChatMessage
                  v-if="
                    pair.user &&
                    (pair.user.role === 'user' || pair.user.content)
                  "
                  :role="pair.user.role"
                  :content="pair.user.content"
                  :timestamp="pair.user.timestamp"
                  :is-streaming="pair.user.isStreaming"
                  :content-blocks="pair.user.contentBlocks"
                />
                <!-- English UI note -->
                <ChatMessage
                  v-if="
                    pair.assistant &&
                    (pair.assistant.content ||
                      (pair.assistant.contentBlocks &&
                        pair.assistant.contentBlocks.length > 0))
                  "
                  :role="pair.assistant.role"
                  :content="pair.assistant.content"
                  :timestamp="pair.assistant.timestamp"
                  :is-streaming="pair.assistant.isStreaming"
                  :content-blocks="pair.assistant.contentBlocks"
                  :show-capture-button="!pair.assistant.isStreaming"
                />
              </div>
            </template>

            <!-- English UI note -->
            <AIThinkingIndicator
              v-if="
                isAIThinking &&
                !messages[messages.length - 1]?.content &&
                !(messages[messages.length - 1]?.contentBlocks?.length ?? 0)
              "
              :current-tool-status="currentToolStatus"
              :tools-used="toolsUsedInCurrentRound"
            />
          </div>
        </div>

        <!-- English UI note -->
        <Transition name="xeno-fade-chip">
          <button
            v-if="showScrollToBottom"
            class="xeno-scroll-chip absolute bottom-20 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1.5 rounded-full px-3 py-1.5 text-xs text-white transition-all"
            @click="handleScrollToBottom"
          >
            <UIcon name="i-heroicons-arrow-down" class="h-3.5 w-3.5" />
            <span>{{ t("ai.chat.scrollToBottom") }}</span>
          </button>
        </Transition>

        <!-- English UI note -->
        <div class="px-4 pb-2">
          <div class="mx-auto max-w-3xl">
            <ChatInput
              :disabled="isAIThinking"
              :status="isAIThinking ? 'streaming' : 'ready'"
              @send="handleSend"
              @stop="handleStop"
            />

            <!-- English UI note -->
            <ChatStatusBar
              :chat-type="currentChatType"
              :session-token-usage="sessionTokenUsage"
              :has-l-l-m-config="hasLLMConfig"
              :is-checking-config="isCheckingConfig"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <Transition name="xeno-side-panel">
      <div
        v-if="sourceMessages.length > 0 && !isSourcePanelCollapsed"
        class="xeno-side-source-panel w-80 shrink-0 p-4"
      >
        <DataSourcePanel
          :messages="sourceMessages"
          :keywords="currentKeywords"
          :is-loading="isLoadingSource"
          :is-collapsed="isSourcePanelCollapsed"
          class="h-full"
          @toggle="toggleSourcePanel"
          @load-more="handleLoadMore"
        />
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.xeno-chat-shell {
  background: linear-gradient(180deg, transparent, var(--xeno-surface-muted));
}

.xeno-chat-main {
  border-right: 1px solid
    color-mix(in srgb, var(--xeno-border-soft) 72%, transparent);
}

.xeno-scroll-chip {
  background: color-mix(in srgb, var(--xeno-surface-emphasis) 78%, #020617 22%);
  border: 1px solid var(--xeno-border-soft);
  box-shadow: var(--xeno-shadow-soft);
  backdrop-filter: blur(10px) saturate(130%);
}

.xeno-scroll-chip:hover {
  transform: translateX(-50%) translateY(-1px);
  border-color: var(--xeno-active-border);
  background: color-mix(in srgb, var(--xeno-surface-emphasis) 82%, #020617 18%);
}

.xeno-side-source-panel {
  border-left: 1px solid var(--xeno-border-soft);
  background: var(--xeno-surface-muted);
  backdrop-filter: blur(14px) saturate(125%);
}

.xeno-fade-chip-enter-active,
.xeno-fade-chip-leave-active {
  transition:
    opacity 0.22s cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 0.22s cubic-bezier(0.2, 0.8, 0.2, 1),
    filter 0.22s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.xeno-fade-chip-enter-from,
.xeno-fade-chip-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(8px) scale(0.985);
  filter: blur(6px);
}

.xeno-side-panel-enter-active,
.xeno-side-panel-leave-active {
  transition:
    opacity 0.28s cubic-bezier(0.22, 0.92, 0.3, 1),
    transform 0.28s cubic-bezier(0.22, 0.92, 0.3, 1),
    filter 0.28s cubic-bezier(0.22, 0.92, 0.3, 1);
}

.xeno-side-panel-enter-from,
.xeno-side-panel-leave-to {
  opacity: 0;
  transform: translateX(14px) scale(0.992);
  filter: blur(8px);
}

@media (prefers-reduced-motion: reduce) {
  .xeno-fade-chip-enter-active,
  .xeno-fade-chip-leave-active,
  .xeno-side-panel-enter-active,
  .xeno-side-panel-leave-active {
    transition-duration: 0.01ms !important;
  }

  .xeno-fade-chip-enter-from,
  .xeno-fade-chip-leave-to,
  .xeno-side-panel-enter-from,
  .xeno-side-panel-leave-to {
    opacity: 1;
    transform: none;
    filter: none;
  }
}
</style>
