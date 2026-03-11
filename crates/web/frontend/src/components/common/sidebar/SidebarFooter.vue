<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter, useRoute } from "vue-router";
import { useLayoutStore } from "@/stores/layout";
import SidebarButton from "./SidebarButton.vue";

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const layoutStore = useLayoutStore();

// English engineering note.
const isManagePage = computed(() => route.name === "workbench");

// English engineering note.
// English engineering note.
</script>

<template>
  <div class="px-4 py-2 dark:border-gray-800 space-y-2 mb-2">
    <!-- English UI note -->
    <SidebarButton
      icon="i-heroicons-rectangle-stack"
      :title="t('tools.title')"
      :active="isManagePage"
      @click="router.push({ name: 'workbench' })"
    />
    <!-- English UI note -->
    <SidebarButton
      icon="i-heroicons-cog-6-tooth"
      :title="t('layout.footer.settings')"
      @click="layoutStore.showSettingModal = true"
    />
  </div>
</template>
