<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import CaptureButton from "@/components/common/CaptureButton.vue";

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    // English engineering note.
    items: any[];
    // English engineering note.
    title: string;
    // English engineering note.
    description?: string;
    // English engineering note.
    topN?: number;
    // English engineering note.
    countTemplate?: string;
  }>(),
  {
    topN: 10,
  },
);

// English engineering note.
const isOpen = ref(false);

// English engineering note.
const cardRef = ref<HTMLElement | null>(null);
const modalBodyRef = ref<HTMLElement | null>(null);

// English engineering note.
const topNData = computed(() => props.items.slice(0, props.topN));

// English engineering note.
const showViewAll = computed(() => props.items.length > props.topN);

// English engineering note.
const formattedCount = computed(() => {
  const template =
    props.countTemplate || t("views.charts.listPro.countTemplate");
  return template.replace("{count}", String(props.items.length));
});
</script>

<template>
  <div
    ref="cardRef"
    class="rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-800 dark:bg-gray-900"
  >
    <div
      class="flex items-center justify-between border-b border-gray-200 px-5 py-3 dark:border-gray-800"
    >
      <div>
        <h3
          class="font-semibold text-gray-900 whitespace-nowrap dark:text-white"
        >
          {{ title }}
        </h3>
        <p
          v-if="description"
          class="mt-1 text-sm text-gray-500 dark:text-gray-400"
        >
          {{ description }}
        </p>
      </div>

      <div class="no-capture flex items-center gap-2">
        <!-- English UI note -->
        <slot name="headerRight" />

        <!-- English UI note -->
        <CaptureButton size="xs" type="element" :target-element="cardRef" />

        <!-- English UI note -->
        <UModal v-model:open="isOpen" :ui="{ content: 'md:w-full max-w-3xl' }">
          <UButton
            v-if="showViewAll"
            icon="i-heroicons-list-bullet"
            variant="ghost"
          >
            {{ t("views.charts.listPro.fullRanking") }}
          </UButton>
          <template #content>
            <div ref="modalBodyRef" class="section-content flex flex-col">
              <!-- Header -->
              <div
                class="flex w-full items-center justify-between border-b border-gray-200 px-6 py-4 dark:border-gray-700"
              >
                <div class="flex items-center gap-2">
                  <h3
                    class="text-lg font-semibold text-gray-900 whitespace-nowrap dark:text-white"
                  >
                    {{ title }}
                  </h3>
                  <span class="text-sm text-gray-500"
                    >（{{ formattedCount }}）</span
                  >
                </div>
                <CaptureButton
                  size="xs"
                  type="element"
                  :target-element="modalBodyRef"
                />
              </div>
              <!-- Body -->
              <div
                class="max-h-[60vh] p-4 divide-y divide-gray-100 overflow-y-auto dark:divide-gray-800"
              >
                <div
                  v-for="(item, index) in items"
                  :key="index"
                  class="px-5 py-3"
                >
                  <slot name="item" :item="item" :index="index" />
                </div>
              </div>
            </div>
          </template>
        </UModal>
      </div>
    </div>

    <!-- English UI note -->
    <slot name="config" />

    <!-- English UI note -->
    <div class="divide-y divide-gray-100 dark:divide-gray-800">
      <div v-for="(item, index) in topNData" :key="index" class="px-5 py-3">
        <slot name="item" :item="item" :index="index" />
      </div>
    </div>

    <!-- English UI note -->
    <div v-if="items.length === 0">
      <slot name="empty">
        <div class="px-5 py-8 text-center text-sm text-gray-400">
          {{ t("views.charts.listPro.empty") }}
        </div>
      </slot>
    </div>
  </div>
</template>
