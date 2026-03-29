<script setup lang="ts">
import { computed } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import PageHeader from "@/components/layout/PageHeader.vue";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const sessionStore = useSessionStore();
const { sessions } = storeToRefs(sessionStore);

const workbenchMetrics = computed(() => {
  const totalSessions = sessions.value.length;
  const totalMessages = sessions.value.reduce(
    (sum, session) => sum + session.messageCount,
    0,
  );
  const activePlatforms = new Set(
    sessions.value.map((session) => session.platform),
  ).size;

  return [
    {
      label: "Imported Sessions",
      value: totalSessions.toLocaleString(),
    },
    {
      label: "Message Volume",
      value: totalMessages.toLocaleString(),
    },
    {
      label: "Active Platforms",
      value: activePlatforms.toLocaleString(),
    },
  ];
});
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
          <div class="xeno-workbench-metrics">
            <article
              v-for="metric in workbenchMetrics"
              :key="metric.label"
              class="xeno-workbench-metric"
            >
              <div class="xeno-workbench-metric-label">{{ metric.label }}</div>
              <div class="xeno-workbench-metric-value">{{ metric.value }}</div>
            </article>
          </div>
        </div>

        <div class="xeno-panel xeno-workbench-panel rounded-2xl p-4 sm:p-5">
          <div class="xeno-workbench-safe-panel">
            <div class="xeno-workbench-safe-badge">SAFE MODE</div>
            <h3 class="xeno-workbench-safe-title">Destructive controls removed</h3>
            <p class="xeno-workbench-safe-copy">
              Session deletion and cleanup actions are no longer exposed in the
              frontend. This surface is now limited to read-only operational
              review while import, monitor, analysis, and export flows remain available.
            </p>
          </div>
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

.xeno-workbench-metrics {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.75rem;
  margin-top: 0.9rem;
}

.xeno-workbench-metric {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1rem;
  padding: 0.85rem 0.95rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 120%),
    rgba(7, 18, 29, 0.72);
}

.xeno-workbench-metric-label {
  font-family: var(--xeno-font-mono);
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: #78daf5;
}

.xeno-workbench-metric-value {
  margin-top: 0.35rem;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--xeno-text-main);
}

.xeno-workbench-panel {
  position: relative;
  overflow: hidden;
}

.xeno-workbench-safe-panel {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1rem;
  padding: 1rem 1.05rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 120%),
    rgba(7, 18, 29, 0.72);
}

.xeno-workbench-safe-badge {
  display: inline-flex;
  width: fit-content;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(56, 189, 248, 0.22);
  border-radius: 9999px;
  padding: 0.2rem 0.55rem;
  font-family: var(--xeno-font-mono);
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: #78daf5;
  background: rgba(255, 255, 255, 0.03);
}

.xeno-workbench-safe-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--xeno-text-main);
}

.xeno-workbench-safe-copy {
  max-width: 54rem;
  font-size: 0.92rem;
  line-height: 1.6;
  color: var(--xeno-text-secondary);
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

@media (max-width: 900px) {
  .xeno-workbench-metrics {
    grid-template-columns: 1fr;
  }
}
</style>
