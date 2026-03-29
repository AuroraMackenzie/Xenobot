<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import AgreementModal from "./components/AgreementModal.vue";
import MigrationModal from "./components/MigrationModal.vue";
import ImportArea from "./components/ImportArea.vue";
import ChangelogModal from "./components/ChangelogModal.vue";
import HomeFooter from "./components/HomeFooter.vue";

const { t } = useI18n();

// English engineering note.
const changelogModalRef = ref<InstanceType<typeof ChangelogModal> | null>(null);
const agreementModalRef = ref<InstanceType<typeof AgreementModal> | null>(null);

// English engineering note.
async function openChangelog() {
  changelogModalRef.value?.open();
}

// English engineering note.
function openTerms() {
  agreementModalRef.value?.open();
}

const features = computed(() => [
  {
    icon: "i-heroicons-shield-check",
    title: t("home.features.privacy.title"),
    description: t("home.features.privacy.description"),
    tone: "xeno-feature-tone--secure",
  },
  {
    icon: "i-heroicons-chart-bar",
    title: t("home.features.analysis.title"),
    description: t("home.features.analysis.description"),
    tone: "xeno-feature-tone--analysis",
  },
  {
    icon: "i-heroicons-sparkles",
    title: t("home.features.ai.title"),
    description: t("home.features.ai.description"),
    tone: "xeno-feature-tone--ai",
  },
]);

const operationalMarkers = computed(() => [
  {
    label: "Authorized Intake",
    value: "Export-only ingestion surface",
  },
  {
    label: "Execution Route",
    value: "Sandbox-aware local pipeline",
  },
  {
    label: "Adapter Coverage",
    value: "17 aligned platform contracts",
  },
]);
</script>

<template>
  <div class="relative flex h-full w-full overflow-hidden pt-4">
    <div class="absolute inset-0 pointer-events-none">
      <div class="xeno-orb xeno-orb-a" />
      <div class="xeno-orb xeno-orb-b" />
      <div class="xeno-orb xeno-orb-c" />
    </div>

    <div class="relative h-full w-full overflow-y-auto">
      <div
        class="flex min-h-full w-full flex-col items-center px-4 py-10 md:py-14"
      >
        <div
          class="absolute -top-32 left-0 right-0 h-32"
          style="-webkit-app-region: drag"
        />

        <div class="w-full max-w-6xl">
          <div
            class="xeno-hero-panel xeno-panel-emphasis xeno-reveal xeno-reveal-1"
          >
            <div
              class="xeno-hero-kicker mb-4 inline-flex items-center gap-2 rounded-full px-4 py-1.5 text-xs font-semibold backdrop-blur-sm"
            >
              <UIcon name="i-heroicons-bolt" class="h-3.5 w-3.5" />
              <span>AUTHORIZED INPUTS • SANDBOX-AWARE • OPERATIONAL AI</span>
            </div>

            <div class="xeno-hero-layout">
              <div class="xeno-hero-copy">
                <h1
                  class="xeno-hero-title select-none text-4xl font-black tracking-tight sm:text-5xl lg:text-6xl"
                >
                  {{ t("home.title") }}
                </h1>
                <p
                  class="xeno-hero-subtitle mt-3 max-w-2xl text-base sm:text-lg"
                >
                  {{ t("home.subtitle") }}
                </p>
              </div>

              <div class="xeno-hero-signal xeno-panel-muted">
                <div class="xeno-hero-signal-label">SYSTEM SIGNAL</div>
                <div class="xeno-hero-signal-value">
                  Rust-first surface, original local workflow.
                </div>
                <div class="xeno-hero-signal-note">
                  Parser intake, sandbox routing, analytics, and MCP guidance
                  move through one operational shell.
                </div>
              </div>
            </div>

            <div class="xeno-hero-metrics mt-6">
              <article
                v-for="marker in operationalMarkers"
                :key="marker.label"
                class="xeno-hero-metric"
              >
                <div class="xeno-hero-metric-label">{{ marker.label }}</div>
                <div class="xeno-hero-metric-value">{{ marker.value }}</div>
              </article>
            </div>

            <div class="mt-8 grid grid-cols-1 gap-3 sm:grid-cols-3">
              <article
                v-for="(feature, idx) in features"
                :key="feature.title"
                class="group xeno-card-reveal xeno-feature-card p-4 transition-all duration-200 hover:-translate-y-0.5"
                :style="{ '--xeno-card-delay': `${120 + idx * 80}ms` }"
              >
                <div
                  class="xeno-feature-icon mb-3 inline-flex h-9 w-9 items-center justify-center rounded-xl text-white"
                  :class="feature.tone"
                >
                  <UIcon :name="feature.icon" class="h-4.5 w-4.5" />
                </div>
                <h3 class="xeno-feature-title text-sm font-semibold">
                  {{ feature.title }}
                </h3>
                <p class="xeno-feature-copy mt-1 text-xs leading-relaxed">
                  {{ feature.description }}
                </p>
              </article>
            </div>
          </div>
        </div>

        <div
          class="xeno-import-shell mt-8 xeno-reveal xeno-reveal-2 w-full max-w-6xl px-4 py-6 sm:px-6"
        >
          <div class="xeno-import-shell-head mb-5">
            <div class="xeno-import-shell-kicker">INGEST WORKSPACE</div>
            <div class="xeno-import-shell-copy">
              Stage authorized exports, inspect parser progress, and route new
              sessions into the operational graph without leaving the launchpad.
            </div>
          </div>
          <ImportArea />
        </div>
      </div>

      <HomeFooter @open-changelog="openChangelog" @open-terms="openTerms" />
    </div>

    <AgreementModal ref="agreementModalRef" />
    <MigrationModal />
    <ChangelogModal ref="changelogModalRef" />
  </div>
</template>

<style scoped>
.xeno-hero-panel {
  position: relative;
  border: 1px solid var(--xeno-border-strong);
  background:
    radial-gradient(
      circle at 72% 56%,
      rgba(56, 189, 248, 0.08),
      transparent 28%
    ),
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-stage-shell-bg);
  border-radius: 1.5rem;
  padding: 1.25rem;
  box-shadow:
    0 22px 60px -38px rgba(15, 23, 42, 0.24),
    inset 0 1px 0 rgba(255, 255, 255, 0.28);
  backdrop-filter: none;
  overflow: hidden;
}

.xeno-hero-panel::after {
  content: "";
  position: absolute;
  inset: -130% -35%;
  background: linear-gradient(
    120deg,
    transparent 28%,
    rgba(255, 255, 255, 0.2) 45%,
    transparent 62%
  );
  transform: translate3d(-14%, 0, 0);
  pointer-events: none;
  animation: xeno-sheen 10s linear infinite;
}

.xeno-hero-kicker {
  border: 1px solid rgba(103, 211, 255, 0.28);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    color-mix(in srgb, var(--xeno-surface-main) 92%, transparent);
  color: #74dcf8;
}

.xeno-hero-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(18rem, 0.9fr);
  gap: 1rem;
  align-items: start;
}

.xeno-hero-copy {
  min-width: 0;
}

.xeno-hero-title {
  color: var(--xeno-text-main);
  text-shadow: 0 2px 24px rgba(2, 6, 23, 0.34);
}

.xeno-hero-subtitle {
  color: var(--xeno-text-secondary);
  text-shadow: 0 2px 18px rgba(2, 6, 23, 0.24);
}

.xeno-hero-signal {
  border-radius: 1.25rem;
  padding: 1rem;
  min-width: 0;
}

.xeno-hero-signal-label {
  font-family: "JetBrains Mono", monospace;
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: #74ddf7;
}

.xeno-hero-signal-value {
  margin-top: 0.5rem;
  font-family: "Space Grotesk", sans-serif;
  font-size: 1.15rem;
  font-weight: 600;
  line-height: 1.25;
  color: var(--xeno-text-main);
}

.xeno-hero-signal-note {
  margin-top: 0.55rem;
  font-size: 0.8rem;
  line-height: 1.45;
  color: var(--xeno-text-secondary);
}

.xeno-hero-metrics {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.8rem;
}

.xeno-hero-metric {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1rem;
  padding: 0.85rem 0.95rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-stage-inner-bg);
  backdrop-filter: none;
}

.xeno-hero-metric-label {
  font-family: var(--xeno-font-mono);
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: #7dddf6;
}

.xeno-hero-metric-value {
  margin-top: 0.4rem;
  font-size: 0.84rem;
  line-height: 1.45;
  color: var(--xeno-text-secondary);
}

.xeno-feature-card {
  border: 1px solid rgba(102, 191, 220, 0.16);
  border-radius: 1.25rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-stage-inner-bg);
  box-shadow: var(--xeno-shadow-soft);
  backdrop-filter: none;
}

.xeno-feature-icon {
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.12),
    0 12px 28px rgba(5, 14, 24, 0.24);
}

.xeno-feature-tone--secure {
  background: linear-gradient(
    135deg,
    rgba(34, 211, 238, 0.96),
    rgba(14, 165, 233, 0.82)
  );
}

.xeno-feature-tone--analysis {
  background: linear-gradient(
    135deg,
    rgba(45, 212, 191, 0.94),
    rgba(59, 130, 246, 0.82)
  );
}

.xeno-feature-tone--ai {
  background: linear-gradient(
    135deg,
    rgba(20, 184, 166, 0.94),
    rgba(8, 145, 178, 0.82)
  );
}

.xeno-feature-title {
  color: var(--xeno-text-main);
}

.xeno-feature-copy {
  color: var(--xeno-text-secondary);
}

.xeno-import-shell {
  border: 1px solid var(--xeno-border-strong);
  border-radius: 1.5rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-stage-shell-bg);
  box-shadow: var(--xeno-shadow-panel);
  backdrop-filter: none;
}

.xeno-import-shell-head {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.xeno-import-shell-kicker {
  display: inline-flex;
  width: fit-content;
  align-items: center;
  border: 1px solid var(--xeno-border-soft);
  border-radius: 9999px;
  padding: 0.2rem 0.6rem;
  font-family: var(--xeno-font-mono);
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: #77d8f3;
  background: rgba(255, 255, 255, 0.06);
}

.xeno-import-shell-copy {
  max-width: 54rem;
  font-size: 0.92rem;
  line-height: 1.55;
  color: var(--xeno-text-secondary);
}

@media (min-width: 640px) {
  .xeno-hero-panel {
    padding: 2rem;
  }
}

@media (max-width: 900px) {
  .xeno-hero-layout {
    grid-template-columns: 1fr;
  }

  .xeno-hero-metrics {
    grid-template-columns: 1fr;
  }
}

.xeno-orb {
  position: absolute;
  border-radius: 9999px;
  filter: blur(34px);
  opacity: 0.2;
}

.xeno-orb-a {
  top: 3%;
  left: 4%;
  width: 12rem;
  height: 12rem;
  background: linear-gradient(135deg, #22d3ee, #0ea5e9);
  animation: xeno-float-a 15s ease-in-out infinite alternate;
}

.xeno-orb-b {
  top: 8%;
  right: 8%;
  width: 9rem;
  height: 9rem;
  background: linear-gradient(135deg, #fb923c, #f59e0b);
  animation: xeno-float-b 18s ease-in-out infinite alternate;
}

.xeno-orb-c {
  bottom: 18%;
  right: 28%;
  width: 8rem;
  height: 8rem;
  background: linear-gradient(135deg, #2dd4bf, #06b6d4);
  animation: xeno-float-c 16s ease-in-out infinite alternate;
}

.xeno-reveal {
  opacity: 0;
  transform: translate3d(0, 12px, 0) scale(0.995);
  animation: xeno-fade-up 560ms cubic-bezier(0.2, 0.8, 0.2, 1) forwards;
}

.xeno-reveal-1 {
  animation-delay: 40ms;
}

.xeno-reveal-2 {
  animation-delay: 160ms;
}

.xeno-card-reveal {
  opacity: 0;
  transform: translate3d(0, 14px, 0) scale(0.994);
  animation: xeno-fade-up 540ms cubic-bezier(0.2, 0.8, 0.2, 1) forwards;
  animation-delay: var(--xeno-card-delay, 120ms);
}

@keyframes xeno-fade-up {
  0% {
    opacity: 0;
    transform: translate3d(0, 12px, 0) scale(0.995);
  }
  100% {
    opacity: 1;
    transform: translate3d(0, 0, 0) scale(1);
  }
}

@keyframes xeno-sheen {
  0% {
    transform: translate3d(-22%, 0, 0) rotate(0.0001deg);
  }
  100% {
    transform: translate3d(24%, 0, 0) rotate(0.0001deg);
  }
}

@keyframes xeno-float-a {
  0% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  100% {
    transform: translate3d(16px, -12px, 0) scale(1.08);
  }
}

@keyframes xeno-float-b {
  0% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  100% {
    transform: translate3d(-18px, 14px, 0) scale(1.1);
  }
}

@keyframes xeno-float-c {
  0% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  100% {
    transform: translate3d(12px, -16px, 0) scale(1.09);
  }
}

@media (prefers-reduced-motion: reduce) {
  .xeno-orb-a,
  .xeno-orb-b,
  .xeno-orb-c,
  .xeno-hero-panel::after,
  .xeno-reveal,
  .xeno-card-reveal {
    animation: none !important;
    opacity: 1 !important;
    transform: none !important;
  }
}
</style>
