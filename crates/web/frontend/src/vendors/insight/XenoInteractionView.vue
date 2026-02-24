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

interface MentionRank {
  memberId: number
  name: string
  count: number
}

interface MentionPair {
  fromName: string
  toName: string
  fromToCount?: number
  member1Name?: string
  member2Name?: string
  member1To2?: number
  member2To1?: number
}

const props = defineProps<{
  sessionId: string
  timeFilter?: TimeFilter
}>()

const { t } = useI18n()
const isLoading = ref(false)
const mentionData = ref<any>(null)

async function loadData() {
  if (!props.sessionId) return
  isLoading.value = true
  try {
    mentionData.value = await window.chatApi.getMentionAnalysis(props.sessionId, props.timeFilter)
  } catch (error) {
    console.error('XenoInteractionView load failed:', error)
    mentionData.value = null
  } finally {
    isLoading.value = false
  }
}

watch(
  () => [props.sessionId, props.timeFilter],
  () => loadData(),
  { immediate: true, deep: true }
)

const topMentioners = computed<MentionRank[]>(() => (mentionData.value?.topMentioners || []).slice(0, 10))
const topMentioned = computed<MentionRank[]>(() => (mentionData.value?.topMentioned || []).slice(0, 10))

const graphData = computed(() => {
  const nodeMap = new Map<string, { id: string; name: string; value: number; symbolSize: number }>()
  const linkMap = new Map<string, { source: string; target: string; value: number }>()

  const addNode = (name: string, weight = 1) => {
    if (!name) return
    const existing = nodeMap.get(name)
    if (existing) {
      existing.value += weight
      existing.symbolSize = Math.min(72, 24 + Math.sqrt(existing.value) * 2.5)
      return
    }
    nodeMap.set(name, {
      id: name,
      name,
      value: weight,
      symbolSize: Math.min(72, 24 + Math.sqrt(weight) * 2.5),
    })
  }

  const addLink = (source: string, target: string, value: number) => {
    if (!source || !target || source === target || value <= 0) return
    const key = `${source}->${target}`
    const existing = linkMap.get(key)
    if (existing) {
      existing.value += value
    } else {
      linkMap.set(key, { source, target, value })
    }
  }

  ;(mentionData.value?.oneWay || []).forEach((pair: MentionPair) => {
    const source = pair.fromName || ''
    const target = pair.toName || ''
    const count = Number(pair.fromToCount || 0)
    addNode(source, count)
    addNode(target, count)
    addLink(source, target, count)
  })

  ;(mentionData.value?.twoWay || []).forEach((pair: MentionPair) => {
    const a = pair.member1Name || ''
    const b = pair.member2Name || ''
    const ab = Number(pair.member1To2 || 0)
    const ba = Number(pair.member2To1 || 0)
    addNode(a, ab + ba)
    addNode(b, ab + ba)
    addLink(a, b, ab)
    addLink(b, a, ba)
  })

  const nodes = Array.from(nodeMap.values())
  const links = Array.from(linkMap.values())
  const maxLinkValue = Math.max(1, ...links.map((l) => l.value))
  return { nodes, links, maxLinkValue }
})

const hasData = computed(() => graphData.value.nodes.length > 0 || topMentioners.value.length > 0)
</script>

<template>
  <div class="main-content space-y-5 p-6">
    <LoadingState v-if="isLoading" :text="t('common.loading')" />

    <template v-else-if="hasData">
      <SectionCard title="Mention Network">
        <div class="p-4">
          <EChartGraph :data="graphData" :height="440" layout="force" :directed="true" />
        </div>
      </SectionCard>

      <div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <SectionCard title="Top Mentioners">
          <ul class="space-y-2 p-4">
            <li
              v-for="(item, idx) in topMentioners"
              :key="`out-${item.memberId}`"
              class="flex items-center justify-between rounded-lg border border-slate-200/80 bg-white/70 px-3 py-2 dark:border-slate-700/60 dark:bg-slate-900/60"
            >
              <div class="flex items-center gap-2">
                <span class="inline-flex h-6 w-6 items-center justify-center rounded-full bg-cyan-500 text-xs font-bold text-white">
                  {{ idx + 1 }}
                </span>
                <span class="text-sm font-medium text-slate-800 dark:text-slate-200">{{ item.name }}</span>
              </div>
              <span class="text-sm font-semibold text-cyan-600 dark:text-cyan-300">{{ item.count }}</span>
            </li>
          </ul>
        </SectionCard>

        <SectionCard title="Top Mentioned">
          <ul class="space-y-2 p-4">
            <li
              v-for="(item, idx) in topMentioned"
              :key="`in-${item.memberId}`"
              class="flex items-center justify-between rounded-lg border border-slate-200/80 bg-white/70 px-3 py-2 dark:border-slate-700/60 dark:bg-slate-900/60"
            >
              <div class="flex items-center gap-2">
                <span class="inline-flex h-6 w-6 items-center justify-center rounded-full bg-amber-500 text-xs font-bold text-white">
                  {{ idx + 1 }}
                </span>
                <span class="text-sm font-medium text-slate-800 dark:text-slate-200">{{ item.name }}</span>
              </div>
              <span class="text-sm font-semibold text-amber-600 dark:text-amber-300">{{ item.count }}</span>
            </li>
          </ul>
        </SectionCard>
      </div>
    </template>

    <SectionCard v-else title="Interaction View">
      <EmptyState :text="t('common.noData')" />
    </SectionCard>
  </div>
</template>
