<script setup lang="ts">
import { ref, computed, watch, onMounted, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'

/**
 * English note.
 * English note.
 * English note.
 * English note.
 */
interface TabItem {
  id: string
  label: string
  icon?: string
}

interface Props {
  modelValue: string
  items: TabItem[]
  /** English note.
  persistKey?: string
  /** English note.
  orientation?: 'horizontal' | 'vertical'
}

interface Emits {
  (e: 'update:modelValue', value: string): void
  (e: 'change', value: string): void
}

const props = withDefaults(defineProps<Props>(), {
  orientation: 'horizontal',
})
const emit = defineEmits<Emits>()

const route = useRoute()
const router = useRouter()

// English engineering note.
const isVertical = computed(() => props.orientation === 'vertical')

// English engineering note.
const tabRefs = ref<Record<string, HTMLElement | null>>({})
const containerRef = ref<HTMLElement | null>(null)

// English engineering note.
const indicatorStyle = ref<Record<string, string>>({})

// English engineering note.
const activeTab = computed({
  get: () => props.modelValue,
  set: (value) => {
    emit('update:modelValue', value)
    emit('change', value)
  },
})

// English engineering note.
function updateIndicator() {
  const activeButton = tabRefs.value[activeTab.value]
  if (activeButton && containerRef.value) {
    const containerRect = containerRef.value.getBoundingClientRect()
    const buttonRect = activeButton.getBoundingClientRect()

    if (isVertical.value) {
      // English engineering note.
      indicatorStyle.value = {
        top: `${buttonRect.top - containerRect.top}px`,
        height: `${buttonRect.height}px`,
        right: '0px',
        width: '2px',
      }
    } else {
      // English engineering note.
      indicatorStyle.value = {
        left: `${buttonRect.left - containerRect.left}px`,
        width: `${buttonRect.width}px`,
        bottom: '0px',
        height: '2px',
      }
    }
  }
}

// English engineering note.
const handleTabClick = (tabId: string) => {
  activeTab.value = tabId
}

// English engineering note.
function setTabRef(id: string, el: HTMLElement | null) {
  tabRefs.value[id] = el
}

// English engineering note.
onMounted(() => {
  if (props.persistKey) {
    const savedTab = route.query[props.persistKey] as string
    // English engineering note.
    if (savedTab && props.items.some((item) => item.id === savedTab)) {
      activeTab.value = savedTab
    }
  }
  // English engineering note.
  nextTick(() => {
    updateIndicator()
  })
})

// English engineering note.
watch(
  () => props.modelValue,
  (newValue) => {
    if (props.persistKey && newValue) {
      // English engineering note.
      router.replace({
        query: {
          ...route.query,
          [props.persistKey]: newValue,
        },
      })
    }
    // English engineering note.
    nextTick(() => {
      updateIndicator()
    })
  }
)

// English engineering note.
watch(
  () => props.items,
  () => {
    nextTick(() => {
      updateIndicator()
    })
  },
  { deep: true }
)
</script>

<template>
  <div
    :class="[
      isVertical
        ? 'h-full border-r border-gray-200/50 dark:border-gray-700/50'
        : 'flex items-center justify-between border-b border-gray-200/50 px-6 dark:border-gray-800/50',
    ]"
  >
    <div ref="containerRef" class="relative" :class="[isVertical ? 'flex flex-col gap-1' : 'flex gap-1']">
      <button
        v-for="tab in items"
        :key="tab.id"
        :ref="(el) => setTabRef(tab.id, el as HTMLElement)"
        class="flex items-center gap-2 text-sm font-medium transition-colors"
        :class="[
          isVertical ? 'justify-start px-3 py-2' : 'px-4 py-3',
          activeTab === tab.id
            ? 'text-primary-600 dark:text-primary-400'
            : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300',
        ]"
        @click="handleTabClick(tab.id)"
      >
        <UIcon v-if="tab.icon" :name="tab.icon" class="h-4 w-4" />
        {{ tab.label }}
      </button>
      <!-- English UI note -->
      <div class="absolute bg-primary-500 transition-all duration-300 ease-out" :style="indicatorStyle" />
    </div>
    <!-- English UI note -->
    <slot v-if="!isVertical" name="right" />
  </div>
</template>
