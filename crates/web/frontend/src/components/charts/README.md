# 图表组件库

这是一个基于 `vue-chartjs` 和 `Chart.js` 的可复用图表组件库，专为 Xenobot 项目设计。所有组件都已经过封装，只需传入数据和简单配置即可使用。

## 📦 组件列表

### 1. DoughnutChart - 环形图

用于展示占比数据，如消息类型分布。

**Props:**

```typescript
interface DoughnutChartData {
  labels: string[] // label buckets for each slice
  values: number[] // numeric payload for slices
  colors?: string[] // optional custom color palette
}

interface Props {
  data: DoughnutChartData
  cutout?: number | string // inner radius size, default '60%'
  height?: number // component height in px, default 256
  showLegend?: boolean // toggle legend, default true
  legendPosition?: 'top' | 'bottom' | 'left' | 'right' // legend anchor, default 'bottom'
}
```

**使用示例:**

```vue
<script setup lang="ts">
import { DoughnutChart } from '@/components/charts'
import type { DoughnutChartData } from '@/components/charts'

const chartData: DoughnutChartData = {
  labels: ['文字', '图片', '语音', '视频'],
  values: [1500, 300, 200, 100],
}
</script>

<template>
  <DoughnutChart :data="chartData" :height="300" />
</template>
```

---

### 2. HorizontalBarChart - 横向柱状图

用于展示排名数据，如 Top 10 活跃成员。

**Props:**

```typescript
interface HorizontalBarChartData {
  labels: string[] // axis labels for ranked bars
  values: number[] // metric values for ranking
  colors?: string[] // optional custom color palette
}

interface Props {
  data: HorizontalBarChartData
  height?: number // component height in px, default 320
  showLegend?: boolean // toggle legend, default false
  borderRadius?: number // bar corner radius, default 8
}
```

**使用示例:**

```vue
<script setup lang="ts">
import { HorizontalBarChart } from '@/components/charts'
import type { HorizontalBarChartData } from '@/components/charts'

const chartData: HorizontalBarChartData = {
  labels: ['张三', '李四', '王五'],
  values: [500, 400, 300],
}
</script>

<template>
  <HorizontalBarChart :data="chartData" />
</template>
```

---

### 3. LineChart - 折线图

用于展示趋势数据，如每日消息趋势。

**Props:**

```typescript
interface LineChartData {
  labels: string[] // x-axis labels
  values: number[] // y-axis measurements
}

interface Props {
  data: LineChartData
  height?: number // component height in px, default 288
  fill?: boolean // draw filled area under curve, default true
  lineColor?: string // stroke color, default '#6366f1'
  fillColor?: string // area fill color, default 'rgba(99, 102, 241, 0.1)'
  tension?: number // curve smoothing factor, default 0.4
  showLegend?: boolean // toggle legend, default false
  xAxisRotation?: number // label rotation angle for x-axis, default 45
}
```

**使用示例:**

```vue
<script setup lang="ts">
import { LineChart } from '@/components/charts'
import type { LineChartData } from '@/components/charts'

const chartData: LineChartData = {
  labels: ['01/01', '01/02', '01/03', '01/04', '01/05'],
  values: [120, 150, 180, 140, 200],
}
</script>

<template>
  <LineChart :data="chartData" :height="300" line-color="#ec4899" fill-color="rgba(236, 72, 153, 0.1)" />
</template>
```

---

### 4. BarChart - 垂直柱状图

用于展示分布数据，如 24 小时活跃分布。

**Props:**

```typescript
interface BarChartData {
  labels: string[] // x-axis labels
  values: number[] // y-axis values
  colors?: string[] // optional custom color palette
}

interface Props {
  data: BarChartData
  height?: number // component height in px, default 256
  showLegend?: boolean // toggle legend, default false
  borderRadius?: number // bar corner radius, default 4
  colorMode?: 'static' | 'gradient' // color rendering mode, default 'gradient'
  xLabelFilter?: (label: string, index: number) => string // x-axis label filter callback
}
```

**使用示例:**

```vue
<script setup lang="ts">
import { BarChart } from '@/components/charts'
import type { BarChartData } from '@/components/charts'

const hourlyData: BarChartData = {
  labels: Array.from({ length: 24 }, (_, i) => `${i}:00`),
  values: [
    50, 30, 20, 15, 10, 20, 40, 60, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 180, 150, 100,
  ],
}
</script>

<template>
  <BarChart :data="hourlyData" :x-label-filter="(_, index) => (index % 3 === 0 ? `${index}:00` : '')" />
</template>
```

---

### 5. MemberRankList - 成员排行列表

用于展示成员排行榜，带排名徽章和进度条。

**Props:**

```typescript
interface MemberRankItem {
  id: string // stable unique id
  name: string // member display name
  value: number // measured value (for example, message count)
  percentage: number // normalized ratio (0-100)
}

interface Props {
  members: MemberRankItem[]
  showAvatar?: boolean // toggle avatar rendering, default true
  rankLimit?: number // number of rows to show, 0 means no cap, default 0
}
```

**使用示例:**

```vue
<script setup lang="ts">
import { MemberRankList } from '@/components/charts'
import type { MemberRankItem } from '@/components/charts'

const members: MemberRankItem[] = [
  { id: '1', name: '张三', value: 500, percentage: 45 },
  { id: '2', name: '李四', value: 400, percentage: 36 },
  { id: '3', name: '王五', value: 300, percentage: 27 },
]
</script>

<template>
  <!-- render the full member list -->
  <MemberRankList :members="members" />

  <!-- render only top five members -->
  <MemberRankList :members="members" :rank-limit="5" />
</template>
```

---

### 6. ProgressBar - 进度条

通用进度条组件，支持自定义颜色和动画。

**Props:**

```typescript
interface Props {
  percentage: number // normalized ratio (0-100)
  color?: string // gradient utility classes, default 'from-indigo-500 to-purple-500'
  height?: number // component height in px, default 8
  showLabel?: boolean // toggle percent text, default false
  animated?: boolean // enable transition animation, default true
}
```

**使用示例:**

```vue
<script setup lang="ts">
import { ProgressBar } from '@/components/charts'
</script>

<template>
  <!-- basic usage -->
  <ProgressBar :percentage="75" />

  <!-- custom gradient with value label -->
  <ProgressBar :percentage="85" color="from-amber-400 to-orange-500" :show-label="true" />
</template>
```

---

## 🎨 设计特性

### 颜色方案

所有图表组件使用统一的配色方案，与 Xenobot 的设计语言保持一致：

- 主色调：Indigo (#6366f1)
- 辅助色：Violet, Purple, Pink, Rose
- 灰度色：Gray 系列

### 响应式设计

- 所有图表组件都支持响应式布局
- 图表尺寸根据容器自动调整
- 支持暗色模式（通过 Tailwind CSS dark: 前缀）

### 交互体验

- 鼠标悬停时显示详细数据
- 平滑的动画过渡效果
- 优化的 Tooltip 样式

---

## 📚 完整导入示例

```typescript
// direct component imports
import { DoughnutChart, LineChart, MemberRankList } from '@/components/charts'

// type-only imports
import type { DoughnutChartData, LineChartData, MemberRankItem } from '@/components/charts'
```

---

## 🔧 技术栈

- **Chart.js**: 强大的图表库
- **vue-chartjs**: Vue 3 的 Chart.js 包装器
- **TypeScript**: 完整的类型支持
- **Tailwind CSS**: 统一的样式系统

---

## 📝 注意事项

1. **数据格式**: 确保传入的数据格式正确，`labels` 和 `values` 数组长度必须一致
2. **性能优化**: 大数据量时，考虑对数据进行分页或限制显示数量
3. **颜色自定义**: 如果提供自定义颜色，确保颜色数量与数据点数量匹配
4. **响应式**: 图表会自动响应容器尺寸变化，无需手动调整

---

## 🚀 扩展建议

如需添加新的图表类型，建议遵循以下原则：

1. **统一接口**: 使用相似的 Props 结构
2. **类型安全**: 导出完整的 TypeScript 类型定义
3. **可配置**: 提供合理的默认值和可选配置项
4. **文档完善**: 在本文档中添加使用说明和示例

---

## 📖 更多资源

- [Chart.js 官方文档](https://www.chartjs.org/docs/latest/)
- [vue-chartjs 文档](https://vue-chartjs.org/)
