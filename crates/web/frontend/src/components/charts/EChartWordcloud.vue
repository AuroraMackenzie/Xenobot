<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

export interface WordcloudData {
  words: Array<{
    word: string;
    count: number;
    percentage?: number;
  }>;
}

interface Props {
  data: WordcloudData;
  height?: number | string;
  loading?: boolean;
  maxWords?: number;
  colorScheme?: "default" | "warm" | "cool" | "rainbow";
  sizeScale?: number;
}

interface PlacedWord {
  word: string;
  count: number;
  percentage?: number;
  color: string;
  fontSize: number;
  rotate: number;
  x: number;
  y: number;
  tooltip: string;
}

interface Box {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

const props = withDefaults(defineProps<Props>(), {
  height: 400,
  loading: false,
  maxWords: 100,
  colorScheme: "default",
  sizeScale: 1,
});

const emit = defineEmits<{
  wordClick: [word: string, count: number];
}>();

const rootRef = ref<HTMLDivElement | null>(null);
const viewportWidth = ref(960);
const viewportHeight = ref(540);
const prefersDark = ref(document.documentElement.classList.contains("dark"));

const heightStyle = computed(() => {
  if (typeof props.height === "number") {
    return `${props.height}px`;
  }
  return props.height;
});

const colorSchemes = {
  default: [
    "#93c5fd",
    "#a78bfa",
    "#f0abfc",
    "#fb7185",
    "#f59e0b",
    "#34d399",
    "#22d3ee",
    "#60a5fa",
  ],
  warm: [
    "#f97316",
    "#fb7185",
    "#facc15",
    "#fb923c",
    "#ef4444",
    "#f59e0b",
    "#f43f5e",
    "#fdba74",
  ],
  cool: [
    "#38bdf8",
    "#60a5fa",
    "#818cf8",
    "#a78bfa",
    "#22d3ee",
    "#2dd4bf",
    "#93c5fd",
    "#67e8f9",
  ],
  rainbow: [
    "#fb7185",
    "#f97316",
    "#facc15",
    "#4ade80",
    "#22d3ee",
    "#60a5fa",
    "#a78bfa",
    "#f472b6",
  ],
} as const;

function estimateBoundingBox(
  text: string,
  fontSize: number,
  rotate: number,
  x: number,
  y: number,
): Box {
  const width = Math.max(
    fontSize * Math.max(text.length, 2) * 0.58,
    fontSize * 1.8,
  );
  const height = fontSize * 1.05;
  const radians = (Math.abs(rotate) * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const rotatedWidth = Math.abs(width * cos) + Math.abs(height * sin);
  const rotatedHeight = Math.abs(width * sin) + Math.abs(height * cos);

  return {
    left: x - rotatedWidth / 2,
    right: x + rotatedWidth / 2,
    top: y - rotatedHeight / 2,
    bottom: y + rotatedHeight / 2,
  };
}

function overlaps(a: Box, b: Box) {
  return !(
    a.right < b.left ||
    a.left > b.right ||
    a.bottom < b.top ||
    a.top > b.bottom
  );
}

const placedWords = computed<PlacedWord[]>(() => {
  const words = props.data.words.slice(0, props.maxWords);
  if (words.length === 0) {
    return [];
  }

  const width = Math.max(viewportWidth.value, 360);
  const height = Math.max(viewportHeight.value, 240);
  const padding = 24;
  const centerX = width / 2;
  const centerY = height / 2;
  const maxCount = Math.max(...words.map((entry) => entry.count));
  const minCount = Math.min(...words.map((entry) => entry.count));
  const countRange = Math.max(maxCount - minCount, 1);
  const colors = colorSchemes[props.colorScheme];
  const baseMin = 14;
  const baseMax = Math.min(72, Math.max(48, height * 0.12));
  const sizeMin = Math.round(baseMin * props.sizeScale);
  const sizeMax = Math.round(baseMax * props.sizeScale);
  const sizeRange = Math.max(sizeMax - sizeMin, 1);
  const occupied: Box[] = [];
  const result: PlacedWord[] = [];

  words.forEach((entry, index) => {
    const normalized = (entry.count - minCount) / countRange;
    const fontSize = Math.round(sizeMin + normalized * sizeRange);
    const rotate = index % 5 === 0 ? -28 : index % 7 === 0 ? 24 : 0;
    const color = colors[index % colors.length];
    const tooltip = entry.percentage
      ? `${entry.word}: ${entry.count} (${entry.percentage}%)`
      : `${entry.word}: ${entry.count}`;

    let chosen: { x: number; y: number; box: Box } | null = null;
    for (let step = 0; step < 900; step += 1) {
      const angle = index * 0.47 + step * 0.36;
      const radius = 4 + step * 1.45;
      const x = centerX + Math.cos(angle) * radius;
      const y = centerY + Math.sin(angle) * radius * 0.72;
      const box = estimateBoundingBox(entry.word, fontSize, rotate, x, y);
      const withinBounds =
        box.left >= padding &&
        box.right <= width - padding &&
        box.top >= padding &&
        box.bottom <= height - padding;

      if (!withinBounds) {
        continue;
      }
      if (occupied.some((existing) => overlaps(existing, box))) {
        continue;
      }

      chosen = { x, y, box };
      break;
    }

    if (!chosen) {
      return;
    }

    occupied.push(chosen.box);
    result.push({
      word: entry.word,
      count: entry.count,
      percentage: entry.percentage,
      color,
      fontSize,
      rotate,
      x: chosen.x,
      y: chosen.y,
      tooltip,
    });
  });

  return result;
});

let resizeObserver: ResizeObserver | null = null;
let mutationObserver: MutationObserver | null = null;

function updateViewport() {
  if (!rootRef.value) {
    return;
  }
  const rect = rootRef.value.getBoundingClientRect();
  viewportWidth.value = Math.max(Math.round(rect.width), 360);
  viewportHeight.value = Math.max(Math.round(rect.height), 240);
}

function updateThemeFlag() {
  prefersDark.value = document.documentElement.classList.contains("dark");
}

watch(
  () => props.height,
  () => {
    requestAnimationFrame(updateViewport);
  },
);

watch(
  () => props.data,
  () => {
    requestAnimationFrame(updateViewport);
  },
  { deep: true },
);

onMounted(() => {
  updateViewport();
  updateThemeFlag();
  resizeObserver = new ResizeObserver(() => updateViewport());
  if (rootRef.value) {
    resizeObserver.observe(rootRef.value);
  }
  mutationObserver = new MutationObserver(() => updateThemeFlag());
  mutationObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class"],
  });
});

onUnmounted(() => {
  resizeObserver?.disconnect();
  mutationObserver?.disconnect();
});
</script>

<template>
  <div
    ref="rootRef"
    class="xeno-wordcloud relative w-full overflow-hidden rounded-2xl border border-white/10 bg-[radial-gradient(circle_at_50%_45%,rgba(148,163,184,0.15),rgba(15,23,42,0.04)_42%,transparent_72%)]"
    :style="{ height: heightStyle }"
  >
    <div
      v-if="loading"
      class="absolute inset-0 z-10 flex items-center justify-center bg-slate-950/35 text-sm font-medium text-slate-100 backdrop-blur-sm"
    >
      Rendering word cloud...
    </div>

    <svg
      class="h-full w-full"
      :viewBox="`0 0 ${viewportWidth} ${viewportHeight}`"
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label="Word cloud"
    >
      <defs>
        <radialGradient id="xenoWordcloudGlow" cx="50%" cy="45%" r="65%">
          <stop
            offset="0%"
            :stop-color="prefersDark ? '#1e293b' : '#e2e8f0'"
            stop-opacity="0.42"
          />
          <stop
            offset="55%"
            :stop-color="prefersDark ? '#0f172a' : '#cbd5e1'"
            stop-opacity="0.08"
          />
          <stop offset="100%" stop-color="transparent" stop-opacity="0" />
        </radialGradient>
      </defs>

      <rect
        x="0"
        y="0"
        :width="viewportWidth"
        :height="viewportHeight"
        fill="url(#xenoWordcloudGlow)"
      />

      <g
        v-for="item in placedWords"
        :key="`${item.word}-${item.count}`"
        class="cursor-pointer"
      >
        <title>{{ item.tooltip }}</title>
        <text
          :x="item.x"
          :y="item.y"
          text-anchor="middle"
          dominant-baseline="middle"
          :fill="item.color"
          :font-size="item.fontSize"
          font-family="'Spline Sans', 'Inter', 'PingFang SC', sans-serif"
          font-weight="700"
          :transform="`rotate(${item.rotate} ${item.x} ${item.y})`"
          class="transition-opacity duration-150 hover:opacity-80"
          @click="emit('wordClick', item.word, item.count)"
        >
          {{ item.word }}
        </text>
      </g>
    </svg>
  </div>
</template>
