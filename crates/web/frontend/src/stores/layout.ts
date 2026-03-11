import { defineStore } from "pinia";
import { ref } from "vue";
import type { ChatRecordQuery } from "@/types/format";

/**
 * English note.
 */
export const useLayoutStore = defineStore(
  "layout",
  () => {
    const isSidebarCollapsed = ref(false);
    const showSettingModal = ref(false);
    const showScreenCaptureModal = ref(false);
    const screenCaptureImage = ref<string | null>(null);
    const showChatRecordDrawer = ref(false);
    const chatRecordQuery = ref<ChatRecordQuery | null>(null);

    // English engineering note.
    const settingTarget = ref<{
      tab: "settings" | "ai" | "storage" | "about";
      section?: string; // English engineering note.
    } | null>(null);

    // English engineering note.
    const screenshotMobileAdapt = ref(true); // English engineering note.

    /**
     * English note.
     */
    function toggleSidebar() {
      isSidebarCollapsed.value = !isSidebarCollapsed.value;
    }

    /**
     * English note.
     */
    function openScreenCaptureModal(imageData: string) {
      screenCaptureImage.value = imageData;
      showScreenCaptureModal.value = true;
    }

    /**
     * English note.
     */
    function closeScreenCaptureModal() {
      showScreenCaptureModal.value = false;
      setTimeout(() => {
        screenCaptureImage.value = null;
      }, 300);
    }

    /**
     * English note.
     */
    function openChatRecordDrawer(query: ChatRecordQuery) {
      chatRecordQuery.value = query;
      showChatRecordDrawer.value = true;
    }

    /**
     * English note.
     */
    function closeChatRecordDrawer() {
      showChatRecordDrawer.value = false;
      setTimeout(() => {
        chatRecordQuery.value = null;
      }, 300);
    }

    /**
     * English note.
     * English note.
     * English note.
     */
    function openSettingAt(
      tab: "settings" | "ai" | "storage" | "about",
      section?: string,
    ) {
      settingTarget.value = { tab, section };
      showSettingModal.value = true;
    }

    /**
     * English note.
     */
    function clearSettingTarget() {
      settingTarget.value = null;
    }

    return {
      isSidebarCollapsed,
      showSettingModal,
      showScreenCaptureModal,
      screenCaptureImage,
      showChatRecordDrawer,
      chatRecordQuery,
      settingTarget,
      screenshotMobileAdapt,
      toggleSidebar,
      openScreenCaptureModal,
      closeScreenCaptureModal,
      openChatRecordDrawer,
      closeChatRecordDrawer,
      openSettingAt,
      clearSettingTarget,
    };
  },
  {
    persist: [
      {
        pick: ["isSidebarCollapsed"],
        storage: sessionStorage,
      },
      {
        pick: ["screenshotMobileAdapt"],
        storage: localStorage,
      },
    ],
  },
);
