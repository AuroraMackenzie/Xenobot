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
    color: "from-cyan-500 to-sky-500",
  },
  {
    icon: "i-heroicons-chart-bar",
    title: t("home.features.analysis.title"),
    description: t("home.features.analysis.description"),
    color: "from-orange-400 to-amber-500",
  },
  {
    icon: "i-heroicons-sparkles",
    title: t("home.features.ai.title"),
    description: t("home.features.ai.description"),
    color: "from-teal-500 to-cyan-600",
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
          <div class="xeno-hero-panel xeno-reveal xeno-reveal-1">
            <div
              class="xeno-hero-kicker mb-4 inline-flex items-center gap-2 rounded-full px-4 py-1.5 text-xs font-semibold backdrop-blur-sm"
            >
              <UIcon name="i-heroicons-bolt" class="h-3.5 w-3.5" />
              <span>AUTHORIZED INPUTS • SANDBOX-AWARE • OPERATIONAL AI</span>
            </div>

            <div class="xeno-hero-layout">
              <div class="xeno-hero-copy">
                <h1
                  class="select-none text-4xl font-black tracking-tight text-slate-900 sm:text-5xl lg:text-6xl dark:text-slate-100"
                >
                  {{ t("home.title") }}
                </h1>
                <p
                  class="mt-3 max-w-2xl text-base text-slate-600 sm:text-lg dark:text-slate-300"
                >
                  {{ t("home.subtitle") }}
                </p>
              </div>

              <div class="xeno-hero-signal">
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

            <div class="mt-8 grid grid-cols-1 gap-3 sm:grid-cols-3">
              <article
                v-for="(feature, idx) in features"
                :key="feature.title"
                class="group xeno-card-reveal xeno-feature-card p-4 transition-all duration-200 hover:-translate-y-0.5"
                :style="{ '--xeno-card-delay': `${120 + idx * 80}ms` }"
              >
                <div
                  class="mb-3 inline-flex h-9 w-9 items-center justify-center rounded-xl bg-linear-to-br text-white"
                  :class="feature.color"
                >
                  <UIcon :name="feature.icon" class="h-4.5 w-4.5" />
                </div>
                <h3
                  class="text-sm font-semibold text-slate-800 dark:text-slate-100"
                >
                  {{ feature.title }}
                </h3>
                <p
                  class="mt-1 text-xs leading-relaxed text-slate-600 dark:text-slate-400"
                >
                  {{ feature.description }}
                </p>
              </article>
            </div>
          </div>
        </div>

        <div
          class="xeno-import-shell mt-8 xeno-reveal xeno-reveal-2 w-full max-w-6xl px-4 py-6 sm:px-6"
        >
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
  background: linear-gradient(
    135deg,
    rgba(255, 255, 255, 0.84),
    rgba(255, 255, 255, 0.58)
  );
  border-radius: 1.5rem;
  padding: 1.25rem;
  box-shadow:
    0 22px 60px -38px rgba(15, 23, 42, 0.45),
    inset 0 1px 0 rgba(255, 255, 255, 0.72);
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
    rgba(9, 27, 40, 0.08);
  color: #0f6f8d;
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

.xeno-hero-signal {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.25rem;
  padding: 1rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-surface-main);
  box-shadow: var(--xeno-shadow-soft);
}

.xeno-hero-signal-label {
  font-family: "JetBrains Mono", monospace;
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: #2b89a8;
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

.xeno-feature-card {
  border: 1px solid var(--xeno-border-soft);
  border-radius: 1.25rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-surface-main);
  box-shadow: var(--xeno-shadow-soft);
  backdrop-filter: blur(12px) saturate(124%);
}

.xeno-import-shell {
  border: 1px solid var(--xeno-border-strong);
  border-radius: 1.5rem;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), transparent 120%),
    var(--xeno-surface-main);
  box-shadow: var(--xeno-shadow-panel);
  backdrop-filter: blur(14px) saturate(126%);
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
}

:root.dark .xeno-hero-panel {
  border-color: rgba(71, 85, 105, 0.62);
  background: linear-gradient(
    135deg,
    rgba(15, 23, 42, 0.82),
    rgba(15, 23, 42, 0.62)
  );
  box-shadow:
    0 30px 70px -42px rgba(2, 6, 23, 0.72),
    inset 0 1px 0 rgba(148, 163, 184, 0.18);
}

:root.dark .xeno-hero-kicker {
  color: #7edcff;
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
