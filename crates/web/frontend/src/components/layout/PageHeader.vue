<script setup lang="ts">
/**
 * Shared page header component.
 * Includes title, description, optional avatar/icon, and slots for extra actions.
 */

defineProps<{
  title: string;
  description?: string;
  icon?: string; // Fallback icon
  iconClass?: string; // Icon background style class
  avatar?: string | null; // Avatar image (base64 Data URL), preferred over icon
}>();
</script>

<template>
  <div class="xeno-page-header relative px-6 pb-2">
    <!-- Drag zone covering top safe-area with platform-aware height.
         macOS: 16px padding + 16px = 32px; Windows/Linux: 32px padding + 16px = 48px -->
    <div class="titlebar-drag-cover" />
    <div class="xeno-page-header-atmosphere" aria-hidden="true" />

    <!-- English UI note -->
    <div class="xeno-page-header-shell flex items-center justify-between gap-4">
      <div class="xeno-page-header-leading flex items-center gap-3">
        <!-- English UI note -->
        <div v-if="avatar" class="xeno-page-header-avatar-wrap">
          <img
            :src="avatar"
            :alt="title"
            class="h-10 w-10 rounded-xl object-cover"
          />
        </div>
        <!-- English UI note -->
        <div
          v-else-if="icon"
          class="xeno-page-header-icon-wrap"
          :class="iconClass"
        >
          <UIcon :name="icon" class="h-5 w-5 text-white" />
        </div>
        <div class="xeno-page-header-copy min-w-0">
          <h1 class="text-lg font-semibold text-[var(--xeno-text-main)]">
            {{ title }}
          </h1>
          <p
            v-if="description"
            class="text-xs text-[var(--xeno-text-secondary)]"
          >
            {{ description }}
          </p>
        </div>
      </div>

      <!-- English UI note -->
      <div class="flex-1 self-stretch mx-4" style="-webkit-app-region: drag" />

      <!-- English UI note -->
      <div class="xeno-page-header-actions flex items-center gap-2">
        <slot name="actions" />
      </div>
    </div>

    <!-- English UI note -->
    <slot />
  </div>
</template>

<style scoped>
/* Drag cover for the title bar using CSS variables for platform-adaptive height. */
.titlebar-drag-cover {
  position: absolute;
  left: 0;
  right: 0;
  z-index: 50;
  top: calc(-1 * var(--titlebar-area-height));
  height: calc(var(--titlebar-area-height) + 1rem);
  -webkit-app-region: drag;
}

.xeno-page-header {
  overflow: hidden;
  border-bottom: 1px solid var(--xeno-border-soft);
  background: linear-gradient(
    180deg,
    var(--xeno-surface-muted),
    transparent 78%
  );
  backdrop-filter: blur(12px) saturate(126%);
}

.xeno-page-header-atmosphere {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    radial-gradient(
      circle at 12% 10%,
      rgba(34, 211, 238, 0.1),
      transparent 28%
    ),
    linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.05),
      transparent 18%,
      transparent 82%,
      rgba(255, 255, 255, 0.02)
    );
  opacity: 0.82;
}

.xeno-page-header-shell {
  position: relative;
  min-height: 4rem;
  padding-top: 0.2rem;
}

.xeno-page-header-shell::before {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(56, 189, 248, 0.28),
    transparent
  );
  opacity: 0.7;
}

.xeno-page-header-avatar-wrap,
.xeno-page-header-icon-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2.9rem;
  height: 2.9rem;
  border-radius: 1rem;
  border: 1px solid var(--xeno-border-soft);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.06), transparent 120%),
    var(--xeno-surface-main);
  box-shadow: var(--xeno-shadow-soft);
  backdrop-filter: blur(14px) saturate(126%);
}

.xeno-page-header-copy {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.xeno-page-header-actions {
  padding: 0.35rem 0.45rem;
  border: 1px solid var(--xeno-border-soft);
  border-radius: 9999px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.06), transparent 120%),
    var(--xeno-surface-main);
  box-shadow: var(--xeno-shadow-soft);
  backdrop-filter: blur(14px) saturate(124%);
}
</style>
