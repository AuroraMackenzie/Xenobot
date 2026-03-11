<script setup lang="ts">
/**
 * English note.
 */
import { computed, ref, watch, onMounted, onUnmounted } from "vue";
import * as echarts from "echarts/core";
import { GraphChart } from "echarts/charts";
import { TooltipComponent, LegendComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import { useDark } from "@vueuse/core";
import type { EChartsOption } from "echarts";

// English engineering note.
echarts.use([GraphChart, TooltipComponent, LegendComponent, CanvasRenderer]);

type ECOption = EChartsOption;

export interface GraphNode {
  id: number | string;
  name: string;
  value?: number;
  symbolSize?: number;
  category?: number;
}

export interface GraphLink {
  source: string;
  target: string;
  value?: number;
}

export interface GraphData {
  nodes: GraphNode[];
  links: GraphLink[];
  maxLinkValue?: number;
}

interface Props {
  data: GraphData;
  height?: number | string;
  layout?: "circular" | "force"; // English engineering note.
  directed?: boolean; // English engineering note.
}

const props = withDefaults(defineProps<Props>(), {
  height: 400,
  layout: "circular",
  directed: false,
});

// English engineering note.
const heightStyle = computed(() => {
  if (typeof props.height === "number") {
    return `${props.height}px`;
  }
  return props.height;
});

const isDark = useDark();
const chartRef = ref<HTMLElement | null>(null);
let chartInstance: echarts.ECharts | null = null;

// English engineering note.
const colorPalette = [
  "#0ea5c9", // English engineering note.
  "#5470c6", // English engineering note.
  "#91cc75", // English engineering note.
  "#fac858", // English engineering note.
  "#ee6666", // English engineering note.
  "#73c0de", // English engineering note.
  "#9a60b4", // English engineering note.
  "#fc8452", // English engineering note.
  "#3ba272", // English engineering note.
  "#ea7ccc", // English engineering note.
  "#6e7074", // English engineering note.
  "#546570", // English engineering note.
];

// English engineering note.
const uniqueNodes = computed(() => {
  const seen = new Set<string>();
  return props.data.nodes.filter((node) => {
    if (seen.has(node.name)) {
      return false;
    }
    seen.add(node.name);
    return true;
  });
});

// English engineering note.
const nodeColorMap = computed(() => {
  const map = new Map<string, string>();
  uniqueNodes.value.forEach((node, index) => {
    map.set(node.name, colorPalette[index % colorPalette.length]);
  });
  return map;
});

// English engineering note.
function getLinkWidth(value: number, maxValue: number): number {
  if (maxValue <= 0) return 1;
  // English engineering note.
  return 1 + (value / maxValue) * 5;
}

const option = computed<ECOption>(() => {
  const maxLinkValue =
    props.data.maxLinkValue ||
    Math.max(...props.data.links.map((l) => l.value || 1), 1);

  return {
    tooltip: {
      trigger: "item",
      backgroundColor: isDark.value
        ? "rgba(30, 30, 30, 0.9)"
        : "rgba(255, 255, 255, 0.95)",
      borderColor: isDark.value
        ? "rgba(255, 255, 255, 0.1)"
        : "rgba(0, 0, 0, 0.1)",
      textStyle: {
        color: isDark.value ? "#e5e7eb" : "#374151",
      },
      formatter: (params: any) => {
        if (params.dataType === "node") {
          return `<b>${params.data.name}</b><br/>消息数: ${params.data.value || 0}`;
        } else if (params.dataType === "edge") {
          return `${params.data.source} → ${params.data.target}<br/>艾特次数: ${params.data.value || 0}`;
        }
        return "";
      },
    },
    // English engineering note.
    animationDuration: 1000,
    animationDurationUpdate: 500,
    animationEasingUpdate: "quinticInOut",
    series: [
      {
        type: "graph",
        layout: props.layout,
        circular:
          props.layout === "circular" ? { rotateLabel: true } : undefined,
        force:
          props.layout === "force"
            ? {
                repulsion: 300,
                gravity: 0.1,
                edgeLength: [80, 200],
                friction: 0.6,
              }
            : undefined,
        roam: true,
        scaleLimit: {
          min: 0.3, // English engineering note.
          max: 3, // English engineering note.
        },
        draggable: true,
        label: {
          show: true,
          position: "right",
          formatter: "{b}",
          color: isDark.value ? "#e5e7eb" : "#374151",
          fontSize: 11,
          fontWeight: 500,
        },
        edgeSymbol: props.directed ? ["none", "arrow"] : ["none", "none"],
        edgeSymbolSize: props.directed ? [0, 10] : [0, 0],
        lineStyle: {
          curveness: 0.3, // English engineering note.
          opacity: 0.5,
        },
        emphasis: {
          focus: "adjacency",
          label: {
            show: true,
            fontSize: 13,
            fontWeight: 600,
          },
          lineStyle: {
            width: 4,
            opacity: 0.9,
          },
          itemStyle: {
            shadowBlur: 15,
            shadowColor: "rgba(0, 0, 0, 0.3)",
          },
        },
        // English engineering note.
        data: uniqueNodes.value.map((node) => {
          const color = nodeColorMap.value.get(node.name) || colorPalette[0];
          return {
            name: node.name,
            value: node.value,
            symbolSize: node.symbolSize || 30,
            // English engineering note.
            label: {
              show:
                props.layout === "circular"
                  ? true
                  : (node.symbolSize || 30) > 30,
            },
            itemStyle: {
              color: color,
              borderColor: "#fff",
              borderWidth: 2,
              shadowBlur: 5,
              shadowColor: `${color}66`, // English engineering note.
            },
          };
        }),
        // English engineering note.
        links: props.data.links
          .filter(
            (link) =>
              nodeColorMap.value.has(link.source) &&
              nodeColorMap.value.has(link.target),
          )
          .map((link) => {
            const sourceColor =
              nodeColorMap.value.get(link.source) || colorPalette[0];
            return {
              source: link.source,
              target: link.target,
              value: link.value,
              lineStyle: {
                color: sourceColor,
                width: getLinkWidth(link.value || 1, maxLinkValue),
              },
            };
          }),
      },
    ],
  };
});

// English engineering note.
function initChart() {
  if (!chartRef.value) return;

  chartInstance = echarts.init(
    chartRef.value,
    isDark.value ? "dark" : undefined,
    {
      renderer: "canvas",
    },
  );
  chartInstance.setOption(option.value);
}

// English engineering note.
function updateChart() {
  if (!chartInstance) return;
  chartInstance.setOption(option.value, { notMerge: true });
}

// English engineering note.
function handleResize() {
  chartInstance?.resize();
}

// English engineering note.
function resetView() {
  if (!chartInstance) return;
  chartInstance.dispatchAction({
    type: "restore",
  });
}

// English engineering note.
defineExpose({
  resetView,
});

// English engineering note.
watch(
  [() => props.data, () => props.layout, () => props.directed, isDark],
  () => {
    if (chartInstance) {
      updateChart();
    } else {
      initChart();
    }
  },
  { deep: true },
);

onMounted(() => {
  initChart();
  window.addEventListener("resize", handleResize);
});

onUnmounted(() => {
  window.removeEventListener("resize", handleResize);
  chartInstance?.dispose();
});
</script>

<template>
  <div ref="chartRef" :style="{ height: heightStyle, width: '100%' }" />
</template>
