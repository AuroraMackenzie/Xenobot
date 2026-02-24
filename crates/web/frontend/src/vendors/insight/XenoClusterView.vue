<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { EChartGraph } from '@/components/charts'
import { EmptyState, LoadingState, SectionCard } from '@/components/UI'

interface TimeFilter {
  startTs?: number
  endTs?: number
  memberId?: number | null
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

const { t } = useI18n()
const isLoading = ref(false)
const clusterData = ref<any>(null)

async function loadData() {
  if (!props.sessionId) return
  isLoading.value = true
  try {
    clusterData.value = await window.chatApi.getClusterGraph(props.sessionId, props.timeFilter)
  } catch (error) {
    console.error('XenoClusterView load failed:', error)
    clusterData.value = null
  } finally {
    isLoading.value = false
  }
}

watch(
  () => [props.sessionId, props.timeFilter],
  () => loadData(),
  { immediate: true, deep: true }
)

const graphData = computed(() => {
  const data = clusterData.value || {}
  const nodes = (data.nodes || []).map((node: any) => ({
    id: node.id ?? node.name,
    name: node.name ?? '',
    value: Number(node.messageCount ?? node.value ?? 0),
    symbolSize: Number(node.symbolSize ?? 28),
  }))
  const links = (data.links || []).map((link: any) => ({
    source: link.source ?? '',
    target: link.target ?? '',
    value: Number(link.value ?? 0),
  }))
  const maxLinkValue = Number(data.maxLinkValue || 1)
  return { nodes, links, maxLinkValue }
})

const stats = computed(() => clusterData.value?.stats || {})
const communities = computed(() => (Array.isArray(clusterData.value?.communities) ? clusterData.value.communities : []))
const hasData = computed(() => graphData.value.nodes.length > 0 && graphData.value.links.length > 0)
</script>

<template>
  <div class="main-content space-y-5 p-6">
    <LoadingState v-if="isLoading" :text="t('common.loading')" />

    <template v-else-if="hasData">
      <div class="grid grid-cols-2 gap-3 lg:grid-cols-5">
        <div class="rounded-xl border border-cyan-100 bg-white/80 p-3 dark:border-cyan-500/20 dark:bg-slate-900/60">
          <p class="text-[11px] font-semibold text-slate-500">MEMBERS</p>
          <p class="mt-1 text-xl font-black text-cyan-600 dark:text-cyan-300">{{ stats.totalMembers || 0 }}</p>
        </div>
        <div class="rounded-xl border border-teal-100 bg-white/80 p-3 dark:border-teal-500/20 dark:bg-slate-900/60">
          <p class="text-[11px] font-semibold text-slate-500">INVOLVED</p>
          <p class="mt-1 text-xl font-black text-teal-600 dark:text-teal-300">{{ stats.involvedMembers || 0 }}</p>
        </div>
        <div class="rounded-xl border border-indigo-100 bg-white/80 p-3 dark:border-indigo-500/20 dark:bg-slate-900/60">
          <p class="text-[11px] font-semibold text-slate-500">EDGES</p>
          <p class="mt-1 text-xl font-black text-indigo-600 dark:text-indigo-300">{{ stats.edgeCount || 0 }}</p>
        </div>
        <div class="rounded-xl border border-amber-100 bg-white/80 p-3 dark:border-amber-500/20 dark:bg-slate-900/60">
          <p class="text-[11px] font-semibold text-slate-500">COMMUNITIES</p>
          <p class="mt-1 text-xl font-black text-amber-600 dark:text-amber-300">{{ stats.communityCount || 0 }}</p>
        </div>
        <div class="rounded-xl border border-sky-100 bg-white/80 p-3 dark:border-sky-500/20 dark:bg-slate-900/60">
          <p class="text-[11px] font-semibold text-slate-500">MESSAGES</p>
          <p class="mt-1 text-xl font-black text-sky-600 dark:text-sky-300">{{ stats.totalMessages || 0 }}</p>
        </div>
      </div>

      <SectionCard title="Community Graph">
        <div class="p-4">
          <EChartGraph :data="graphData" :height="500" layout="force" />
        </div>
      </SectionCard>

      <SectionCard title="Detected Communities">
        <div class="flex flex-wrap gap-2 p-4">
          <span
            v-for="community in communities"
            :key="community.id"
            class="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300"
          >
            <span class="inline-flex h-2 w-2 rounded-full bg-teal-500" />
            {{ community.name }} · {{ community.size }}
          </span>
        </div>
      </SectionCard>
    </template>

    <SectionCard v-else title="Cluster View">
      <EmptyState :text="t('common.noData')" />
    </SectionCard>
  </div>
</template>
