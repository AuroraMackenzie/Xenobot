<script setup lang="ts">
import { ref, computed, nextTick, watch } from "vue";
import type { ComponentPublicInstance } from "vue";

interface TabItem {
  label: string;
  value: string | number;
}

interface Props {
  modelValue: string | number;
  items: TabItem[];
  size?: "xs" | "sm" | "md" | "lg";
}

interface Emits {
  (e: "update:modelValue", value: string | number): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

// English engineering note.
const scrollContainer = ref<HTMLElement>();
const tabsRef = ref<ComponentPublicInstance>();

// English engineering note.
const selectedValue = computed({
  get: () => props.modelValue,
  set: (value) => emit("update:modelValue", value),
});

// English engineering note.
const scrollToCenter = async () => {
  await nextTick();

  if (!scrollContainer.value || !tabsRef.value) return;

  // English engineering note.
  const selectedIndex = props.items.findIndex(
    (item) => item.value === selectedValue.value,
  );
  if (selectedIndex < 0) return;

  // English engineering note.
  const tabsElement = tabsRef.value.$el as HTMLElement;
  const allTabs = tabsElement.querySelectorAll('[role="tab"]');
  const selectedTab = allTabs[selectedIndex] as HTMLElement;

  if (!selectedTab) return;

  const container = scrollContainer.value;
  const containerWidth = container.clientWidth;

  // English engineering note.
  const tabLeft = selectedTab.offsetLeft;
  const tabWidth = selectedTab.offsetWidth;

  // English engineering note.
  const targetScrollLeft = tabLeft + tabWidth / 2 - containerWidth / 2;

  // English engineering note.
  container.scrollTo({
    left: Math.max(0, targetScrollLeft),
    behavior: "smooth",
  });
};

// English engineering note.
watch(
  selectedValue,
  () => {
    scrollToCenter();
  },
  { immediate: false },
);
</script>

<template>
  <div
    ref="scrollContainer"
    class="overflow-x-auto overflow-y-hidden scrollbar-hide"
  >
    <UTabs
      ref="tabsRef"
      v-model="selectedValue"
      :size="size"
      :items="items"
      :content="false"
      class="min-w-max"
    />
  </div>
</template>

<style scoped>
/* Hide native scrollbars while preserving horizontal touch/trackpad scrolling. */
.scrollbar-hide {
  -ms-overflow-style: none; /* IE and Edge */
  scrollbar-width: none; /* Firefox */
}

.scrollbar-hide::-webkit-scrollbar {
  display: none; /* Chrome, Safari and Opera */
}
</style>
