<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { SubTabs } from "@/components/UI";
import UserSelect from "@/components/common/UserSelect.vue";
import { XenoMessageView } from "@/vendors/insight";

const { t } = useI18n();

interface TimeFilter {
  startTs?: number;
  endTs?: number;
}

const props = defineProps<{
  sessionId: string;
  timeFilter?: TimeFilter;
}>();

// English engineering note.
const subTabs = computed(() => [
  {
    id: "message",
    label: t("analysis.subTabs.view.message"),
    icon: "i-heroicons-chat-bubble-left-right",
  },
]);

const activeSubTab = ref("message");

// English engineering note.
const selectedMemberId = ref<number | null>(null);

// English engineering note.
const viewTimeFilter = computed(() => ({
  ...props.timeFilter,
  memberId: selectedMemberId.value,
}));
</script>

<template>
  <div class="xeno-view-shell flex h-full flex-col">
    <SubTabs
      v-model="activeSubTab"
      :items="subTabs"
      persist-key="privateViewTab"
    >
      <template #right>
        <UserSelect v-model="selectedMemberId" :session-id="props.sessionId" />
      </template>
    </SubTabs>

    <div class="xeno-view-stage flex-1 min-h-0 overflow-auto rounded-2xl">
      <Transition name="fade" mode="out-in">
        <XenoMessageView
          v-if="activeSubTab === 'message'"
          :session-id="props.sessionId"
          :time-filter="viewTimeFilter"
        />
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.xeno-view-shell {
  gap: 1rem;
}

.xeno-view-stage {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background:
    radial-gradient(
      circle at top right,
      rgba(59, 130, 246, 0.08),
      transparent 24%
    ),
    linear-gradient(180deg, rgba(15, 23, 42, 0.76), rgba(15, 23, 42, 0.62));
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.05),
    0 18px 38px rgba(2, 6, 23, 0.18);
  backdrop-filter: blur(18px);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
