<script setup lang="ts">
import { useI18n } from "vue-i18n";
import BatchManageTab from "./components/BatchManageTab.vue";
import PageHeader from "@/components/layout/PageHeader.vue";

const { t } = useI18n();
</script>

<template>
  <div class="xeno-analysis-shell relative z-0 flex h-full flex-col pt-4">
    <PageHeader
      :title="t('tools.title')"
      :description="t('tools.description')"
      icon="i-heroicons-rectangle-stack"
      icon-class="bg-primary-600 dark:bg-primary-500"
    >
      <template #actions>
        <div class="xeno-workbench-chip">SESSION FORGE</div>
      </template>
    </PageHeader>

    <div class="flex-1 overflow-auto p-6">
      <div class="xeno-workbench-shell">
        <div class="xeno-workbench-bar">
          <div class="xeno-workbench-kicker">OPERATIONAL SURFACE</div>
          <div class="xeno-workbench-copy">
            Manage imported sessions, review merge readiness, and enforce
            cleanup from one controlled workspace.
          </div>
        </div>

        <div class="xeno-panel xeno-workbench-panel rounded-2xl p-4 sm:p-5">
          <BatchManageTab />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.xeno-analysis-shell {
  background: transparent;
  color: var(--xeno-text-main);
}

.xeno-workbench-shell {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.xeno-workbench-bar {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.2rem;
  padding: 0.95rem 1rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.06), transparent 120%),
    var(--xeno-surface-muted);
  backdrop-filter: blur(12px) saturate(124%);
}

.xeno-workbench-kicker,
.xeno-workbench-chip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--xeno-border-soft);
  border-radius: 9999px;
  padding: 0.2rem 0.55rem;
  font-family: "JetBrains Mono", monospace;
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: #6bcff0;
  background: rgba(255, 255, 255, 0.03);
}

.xeno-workbench-copy {
  margin-top: 0.6rem;
  max-width: 54rem;
  font-size: 0.9rem;
  line-height: 1.55;
  color: var(--xeno-text-secondary);
}

.xeno-workbench-panel {
  position: relative;
  overflow: hidden;
}

.xeno-workbench-panel::before {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(56, 189, 248, 0.22),
    transparent
  );
  opacity: 0.78;
  pointer-events: none;
}
</style>
