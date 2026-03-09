<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useLayoutStore } from '@/stores/layout'

interface Props {
  icon: string
  title: string
  active?: boolean
  tooltip?: string
}

withDefaults(defineProps<Props>(), {
  active: false,
  tooltip: '',
})

const layoutStore = useLayoutStore()
const { isSidebarCollapsed: isCollapsed } = storeToRefs(layoutStore)
</script>

<template>
  <UTooltip :text="isCollapsed ? tooltip || title : ''" :popper="{ placement: 'right' }">
    <UButton
      :block="!isCollapsed"
      class="xeno-sidebar-button h-12 cursor-pointer rounded-2xl transition-all"
      :class="[
        isCollapsed ? 'flex w-12 items-center justify-center px-0' : 'justify-start pl-4',
        active ? 'xeno-sidebar-button-active' : '',
      ]"
      color="gray"
      variant="ghost"
    >
      <UIcon :name="icon" class="h-5 w-5 shrink-0" :class="[isCollapsed ? '' : 'mr-2']" />
      <span v-if="!isCollapsed" class="truncate">{{ title }}</span>
    </UButton>
  </UTooltip>
</template>

<style scoped>
.xeno-sidebar-button {
  border: 1px solid transparent;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 120%),
    transparent;
  color: var(--xeno-text-main);
}

.xeno-sidebar-button:hover {
  border-color: var(--xeno-border-soft);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.06), transparent 120%),
    var(--xeno-hover-bg);
}

.xeno-sidebar-button-active {
  border-color: var(--xeno-active-border);
  background:
    linear-gradient(180deg, rgba(56, 189, 248, 0.08), transparent 120%),
    var(--xeno-active-bg);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.08),
    0 12px 28px -22px rgba(14, 165, 233, 0.46);
}
</style>
