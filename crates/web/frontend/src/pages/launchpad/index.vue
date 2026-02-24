<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import AgreementModal from './components/AgreementModal.vue'
import MigrationModal from './components/MigrationModal.vue'
import ImportArea from './components/ImportArea.vue'
import ChangelogModal from './components/ChangelogModal.vue'
import HomeFooter from './components/HomeFooter.vue'

const { t } = useI18n()

// 弹窗引用
const changelogModalRef = ref<InstanceType<typeof ChangelogModal> | null>(null)
const agreementModalRef = ref<InstanceType<typeof AgreementModal> | null>(null)

// 打开版本日志弹窗（手动点击时调用）
async function openChangelog() {
  changelogModalRef.value?.open()
}

// 打开使用条款弹窗
function openTerms() {
  agreementModalRef.value?.open()
}

const features = computed(() => [
  {
    icon: 'i-heroicons-shield-check',
    title: t('home.features.privacy.title'),
    description: t('home.features.privacy.description'),
    color: 'from-cyan-500 to-sky-500',
  },
  {
    icon: 'i-heroicons-chart-bar',
    title: t('home.features.analysis.title'),
    description: t('home.features.analysis.description'),
    color: 'from-orange-400 to-amber-500',
  },
  {
    icon: 'i-heroicons-sparkles',
    title: t('home.features.ai.title'),
    description: t('home.features.ai.description'),
    color: 'from-teal-500 to-cyan-600',
  },
])
</script>

<template>
  <div class="relative flex h-full w-full overflow-hidden pt-4">
    <div class="absolute inset-0 pointer-events-none">
      <div class="xeno-orb xeno-orb-a" />
      <div class="xeno-orb xeno-orb-b" />
      <div class="xeno-orb xeno-orb-c" />
    </div>

    <div class="relative h-full w-full overflow-y-auto">
      <div class="flex min-h-full w-full flex-col items-center px-4 py-10 md:py-14">
        <div class="absolute -top-32 left-0 right-0 h-32" style="-webkit-app-region: drag" />

        <div class="w-full max-w-6xl">
          <div class="xeno-hero-panel">
            <div class="mb-4 inline-flex items-center gap-2 rounded-full border border-cyan-200/70 bg-white/80 px-4 py-1.5 text-xs font-semibold text-cyan-700 backdrop-blur-sm dark:border-cyan-500/30 dark:bg-slate-900/50 dark:text-cyan-300">
              <UIcon name="i-heroicons-bolt" class="h-3.5 w-3.5" />
              <span>LOCAL-FIRST • AI READY • MULTI-PLATFORM</span>
            </div>

            <h1 class="select-none text-4xl font-black tracking-tight text-slate-900 sm:text-5xl lg:text-6xl dark:text-slate-100">
              {{ t('home.title') }}
            </h1>
            <p class="mt-3 max-w-2xl text-base text-slate-600 sm:text-lg dark:text-slate-300">
              {{ t('home.subtitle') }}
            </p>

            <div class="mt-8 grid grid-cols-1 gap-3 sm:grid-cols-3">
              <article
                v-for="feature in features"
                :key="feature.title"
                class="group rounded-2xl border border-white/60 bg-white/70 p-4 shadow-sm backdrop-blur-sm transition-all duration-200 hover:-translate-y-0.5 hover:shadow-md dark:border-slate-700/60 dark:bg-slate-900/60"
              >
                <div class="mb-3 inline-flex h-9 w-9 items-center justify-center rounded-xl bg-linear-to-br text-white" :class="feature.color">
                  <UIcon :name="feature.icon" class="h-4.5 w-4.5" />
                </div>
                <h3 class="text-sm font-semibold text-slate-800 dark:text-slate-100">{{ feature.title }}</h3>
                <p class="mt-1 text-xs leading-relaxed text-slate-600 dark:text-slate-400">{{ feature.description }}</p>
              </article>
            </div>
          </div>
        </div>

        <div class="mt-8 w-full max-w-6xl rounded-3xl border border-white/60 bg-white/72 px-4 py-6 shadow-lg backdrop-blur-sm dark:border-slate-700/60 dark:bg-slate-900/62 sm:px-6">
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
  border: 1px solid rgba(255, 255, 255, 0.72);
  background:
    linear-gradient(135deg, rgba(255, 255, 255, 0.84), rgba(255, 255, 255, 0.58));
  border-radius: 1.5rem;
  padding: 1.25rem;
  box-shadow:
    0 22px 60px -38px rgba(15, 23, 42, 0.45),
    inset 0 1px 0 rgba(255, 255, 255, 0.72);
}

@media (min-width: 640px) {
  .xeno-hero-panel {
    padding: 2rem;
  }
}

:root.dark .xeno-hero-panel {
  border-color: rgba(71, 85, 105, 0.62);
  background:
    linear-gradient(135deg, rgba(15, 23, 42, 0.82), rgba(15, 23, 42, 0.62));
  box-shadow:
    0 30px 70px -42px rgba(2, 6, 23, 0.72),
    inset 0 1px 0 rgba(148, 163, 184, 0.18);
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
}

.xeno-orb-b {
  top: 8%;
  right: 8%;
  width: 9rem;
  height: 9rem;
  background: linear-gradient(135deg, #fb923c, #f59e0b);
}

.xeno-orb-c {
  bottom: 18%;
  right: 28%;
  width: 8rem;
  height: 8rem;
  background: linear-gradient(135deg, #2dd4bf, #06b6d4);
}
</style>
