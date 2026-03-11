<script setup lang="ts">
import { useScreenCapture } from "@/composables";
import { ref, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useLayoutStore } from "@/stores/layout";
import { useI18n } from "vue-i18n";

/**
 * English note.
 * English note.
 */

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    // English engineering note.
    label?: string;
    // English engineering note.
    size?: "xs" | "sm" | "md";
    // English engineering note.
    type?: "page" | "element";
    // English engineering note.
    targetElement?: HTMLElement | null;
    // English engineering note.
    targetSelector?: string;
  }>(),
  {
    size: "sm",
    type: "page",
  },
);

const { isCapturing, capturePage, captureElement } = useScreenCapture();
const layoutStore = useLayoutStore();
const { screenshotMobileAdapt } = storeToRefs(layoutStore);

// English engineering note.
const buttonId = ref("");
onMounted(() => {
  buttonId.value = `capture-btn-${Math.random().toString(36).slice(2, 8)}`;
});

async function handleCapture(event: Event) {
  const btn = event.currentTarget as HTMLElement;

  // English engineering note.
  const defaultOptions = {
    hideSelectors: [`#${buttonId.value}`],
    mobileWidth: screenshotMobileAdapt.value ? true : undefined,
  };

  if (props.type === "page") {
    await capturePage(defaultOptions);
  } else if (props.type === "element") {
    let target: HTMLElement | null = null;

    if (props.targetElement) {
      target = props.targetElement;
    } else if (props.targetSelector) {
      target = btn.closest(props.targetSelector) as HTMLElement | null;
    }

    if (target) {
      await captureElement(target, defaultOptions);
    }
  }
}
</script>

<template>
  <UTooltip :text="t('common.capture')" class="no-capture">
    <UButton
      :id="buttonId"
      icon="i-heroicons-camera"
      variant="ghost"
      color="primary"
      :size="size"
      :loading="isCapturing"
      @click="handleCapture"
    >
      <template v-if="label">{{ label }}</template>
    </UButton>
  </UTooltip>
</template>
