<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useLayoutStore } from "@/stores/layout";
import { useSettingsStore } from "@/stores/settings";
import { useColorMode } from "@vueuse/core";
import { availableLocales, type LocaleType } from "@/i18n";
import NetworkSettingsSection from "./NetworkSettingsSection.vue";

const { t } = useI18n();

// Store
const layoutStore = useLayoutStore();
const settingsStore = useSettingsStore();
const { screenshotMobileAdapt } = storeToRefs(layoutStore);
const { locale } = storeToRefs(settingsStore);

// Color Mode
const colorMode = useColorMode({
  emitAuto: true,
  initialValue: "dark",
});

// Color mode options
const colorModeOptions = computed(() => [
  { label: t("settings.basic.appearance.auto"), value: "auto" },
  { label: t("settings.basic.appearance.light"), value: "light" },
  { label: t("settings.basic.appearance.dark"), value: "dark" },
]);

// Language options
const languageOptions = computed(() =>
  availableLocales.map((l) => ({
    label: l.nativeName,
    value: l.code,
  })),
);

// Handle language change with writable computed for v-model support
const currentLocale = computed({
  get: () => locale.value,
  set: (val: LocaleType) => settingsStore.setLocale(val),
});

// Sync theme with main process
import { watch } from "vue";
watch(
  colorMode,
  (val) => {
    const mode = val === "auto" ? "system" : (val as "light" | "dark");
    window.api.setThemeSource(mode);
  },
  { immediate: true },
);
</script>

<template>
  <div class="space-y-6">
    <!-- English UI note -->
    <div>
      <h3
        class="xeno-settings-title mb-3 flex items-center gap-2 text-sm font-semibold"
      >
        <UIcon name="i-heroicons-language" class="h-4 w-4 text-green-500" />
        {{ t("settings.basic.language.title") }}
      </h3>
      <div class="xeno-settings-card p-4">
        <div class="flex items-center justify-between">
          <div class="flex-1 pr-4">
            <p class="xeno-settings-copy text-sm font-medium">
              {{ t("settings.basic.language.description") }}
            </p>
          </div>
          <div class="w-48">
            <UTabs
              v-model="currentLocale"
              size="sm"
              class="gap-0"
              :items="languageOptions"
            ></UTabs>
          </div>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <div>
      <h3
        class="xeno-settings-title mb-3 flex items-center gap-2 text-sm font-semibold"
      >
        <UIcon name="i-heroicons-paint-brush" class="h-4 w-4 text-pink-500" />
        {{ t("settings.basic.appearance.title") }}
      </h3>
      <div class="xeno-settings-card p-4">
        <div class="flex items-center justify-between">
          <div class="flex-1 pr-4">
            <p class="xeno-settings-copy text-sm font-medium">
              {{ t("settings.basic.appearance.themeMode") }}
            </p>
          </div>
          <div class="w-64">
            <UTabs
              v-model="colorMode"
              size="sm"
              class="gap-0"
              :items="colorModeOptions"
            ></UTabs>
          </div>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <div>
      <h3
        class="xeno-settings-title mb-3 flex items-center gap-2 text-sm font-semibold"
      >
        <UIcon name="i-heroicons-camera" class="h-4 w-4 text-blue-500" />
        {{ t("settings.basic.screenshot.title") }}
      </h3>
      <div class="xeno-settings-card p-4">
        <div class="flex items-center justify-between">
          <div class="flex-1 pr-4">
            <p class="xeno-settings-copy text-sm font-medium">
              {{ t("settings.basic.screenshot.mobileAdapt") }}
            </p>
            <p class="xeno-settings-caption text-xs">
              {{ t("settings.basic.screenshot.mobileAdaptDesc") }}
            </p>
          </div>
          <USwitch v-model="screenshotMobileAdapt" />
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <NetworkSettingsSection />
  </div>
</template>

<style scoped>
.xeno-settings-title {
  color: var(--xeno-text-main);
}

.xeno-settings-card {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(7, 18, 29, 0.66);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.04),
    0 18px 42px rgba(4, 10, 19, 0.16);
}

.xeno-settings-copy {
  color: var(--xeno-text-main);
}

.xeno-settings-caption {
  color: var(--xeno-text-secondary);
}
</style>
