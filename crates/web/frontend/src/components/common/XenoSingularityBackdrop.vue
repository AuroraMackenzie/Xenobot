<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

interface StarParticle {
  x: number
  y: number
  radius: number
  brightness: number
  depth: number
  twinkle: number
  drift: number
  hueBias: number
}

interface LensSample {
  x: number
  y: number
  distance: number
  ringFactor: number
  tangentX: number
  tangentY: number
}

const canvasRef = ref<HTMLCanvasElement | null>(null)

let ctx: CanvasRenderingContext2D | null = null
let animationFrame = 0
let lastFrame = 0
let elapsed = 0
let width = 1
let height = 1
let deviceScale = 1
let stars: StarParticle[] = []
let darkMode = true
let motionScale = 1
let rootObserver: MutationObserver | null = null
let reduceMotionMedia: MediaQueryList | null = null

const singularity = {
  x: 0.74,
  y: 0.23,
  radius: 84,
}

function updateThemeState() {
  darkMode = document.documentElement.classList.contains('dark')
}

function updateMotionState() {
  motionScale = reduceMotionMedia?.matches ? 0.18 : 1
}

function createStars(count: number) {
  stars = Array.from({ length: count }, () => ({
    x: Math.random(),
    y: Math.random(),
    radius: 0.45 + Math.random() * 1.6,
    brightness: 0.35 + Math.random() * 0.65,
    depth: 0.35 + Math.random() * 0.9,
    twinkle: Math.random() * Math.PI * 2,
    drift: 0.2 + Math.random() * 0.8,
    hueBias: Math.random(),
  }))
}

function resizeCanvas() {
  const canvas = canvasRef.value
  if (!canvas) return

  deviceScale = Math.min(window.devicePixelRatio || 1, 2)
  width = Math.max(1, Math.floor(canvas.clientWidth))
  height = Math.max(1, Math.floor(canvas.clientHeight))

  canvas.width = Math.floor(width * deviceScale)
  canvas.height = Math.floor(height * deviceScale)

  ctx = canvas.getContext('2d')
  if (!ctx) return

  ctx.setTransform(deviceScale, 0, 0, deviceScale, 0, 0)
  singularity.radius = Math.max(78, Math.min(132, Math.min(width, height) * 0.102))

  const area = width * height
  createStars(Math.max(110, Math.min(260, Math.floor(area / 9000))))
}

function lensPoint(x: number, y: number): LensSample {
  const centerX = width * singularity.x
  const centerY = height * singularity.y
  const dx = x - centerX
  const dy = y - centerY
  const distance = Math.hypot(dx, dy)

  if (distance < 0.0001) {
    return {
      x,
      y,
      distance: 0.0001,
      ringFactor: 0,
      tangentX: 0,
      tangentY: 0,
    }
  }

  const unitX = dx / distance
  const unitY = dy / distance
  const tangentX = -unitY
  const tangentY = unitX

  const ringRadius = singularity.radius * 2.22
  const ringSigma = singularity.radius * 0.72
  const ringFactor = Math.exp(-Math.pow((distance - ringRadius) / ringSigma, 2))
  const gravitationalTerm = (singularity.radius * singularity.radius * 0.09) / (distance + singularity.radius * 0.6)
  const ringLift = ringFactor * singularity.radius * 0.58
  const tangentialShear = ringFactor * singularity.radius * 0.06

  return {
    x: x + unitX * (gravitationalTerm + ringLift) + tangentX * tangentialShear,
    y: y + unitY * (gravitationalTerm + ringLift) + tangentY * tangentialShear,
    distance,
    ringFactor,
    tangentX,
    tangentY,
  }
}

function starColor(sample: LensSample, star: StarParticle, pulse: number) {
  const lensBlue = 170 + sample.ringFactor * 45
  const lensWarm = 190 + star.hueBias * 35

  if (star.hueBias > 0.78) {
    return `rgba(${lensWarm}, ${140 + pulse * 16}, ${90 + pulse * 10}, ${0.55 + sample.ringFactor * 0.18})`
  }

  return `rgba(${200 + pulse * 10}, ${220 + pulse * 16}, ${lensBlue}, ${0.52 + sample.ringFactor * 0.22})`
}

function drawStars(timeSeconds: number) {
  if (!ctx) return

  const centerX = width * singularity.x
  const centerY = height * singularity.y
  const eventHorizon = singularity.radius * 0.96

  for (const star of stars) {
    const driftX = Math.sin(timeSeconds * 0.035 * star.drift + star.twinkle) * 18 * star.depth * motionScale
    const driftY = Math.cos(timeSeconds * 0.028 * star.drift + star.twinkle * 0.7) * 12 * star.depth * motionScale

    const baseX = star.x * width + driftX
    const baseY = star.y * height + driftY
    const sample = lensPoint(baseX, baseY)

    if (sample.distance < eventHorizon) {
      continue
    }

    const pulse = 0.5 + 0.5 * Math.sin(timeSeconds * (0.8 + star.depth * 0.7) + star.twinkle)
    const alphaScale = darkMode ? 0.82 : 0.34
    const radius = star.radius * (0.8 + pulse * 0.28) * (0.94 + sample.ringFactor * 0.28)
    const color = starColor(sample, star, pulse)

    if (sample.ringFactor > 0.12) {
      ctx.beginPath()
      ctx.moveTo(sample.x - sample.tangentX * radius * 2.6, sample.y - sample.tangentY * radius * 2.6)
      ctx.lineTo(sample.x + sample.tangentX * radius * 2.6, sample.y + sample.tangentY * radius * 2.6)
      ctx.strokeStyle = color.replace(/[\d.]+\)$/, `${(0.18 + sample.ringFactor * 0.18) * alphaScale})`)
      ctx.lineWidth = Math.max(0.4, radius * 0.8)
      ctx.stroke()
    }

    const halo = ctx.createRadialGradient(sample.x, sample.y, 0, sample.x, sample.y, radius * 4.6)
    halo.addColorStop(0, color.replace(/[\d.]+\)$/, `${(0.34 + sample.ringFactor * 0.26) * alphaScale})`))
    halo.addColorStop(1, color.replace(/[\d.]+\)$/, '0)'))
    ctx.fillStyle = halo
    ctx.beginPath()
    ctx.arc(sample.x, sample.y, radius * 4.6, 0, Math.PI * 2)
    ctx.fill()

    ctx.fillStyle = color.replace(/[\d.]+\)$/, `${(0.62 + pulse * 0.18) * alphaScale})`)
    ctx.beginPath()
    ctx.arc(sample.x, sample.y, radius, 0, Math.PI * 2)
    ctx.fill()
  }

  const vignette = ctx.createRadialGradient(centerX, centerY, singularity.radius * 1.4, centerX, centerY, singularity.radius * 6.2)
  vignette.addColorStop(0, darkMode ? 'rgba(0, 0, 0, 0.02)' : 'rgba(255, 255, 255, 0.01)')
  vignette.addColorStop(1, darkMode ? 'rgba(0, 0, 0, 0.24)' : 'rgba(255, 255, 255, 0.08)')
  ctx.fillStyle = vignette
  ctx.fillRect(0, 0, width, height)
}

function drawAccretionDisc(timeSeconds: number) {
  if (!ctx) return

  const centerX = width * singularity.x
  const centerY = height * singularity.y
  const outerRadius = singularity.radius * 2.95
  const innerRadius = singularity.radius * 1.18
  const verticalScale = 0.38
  const spin = timeSeconds * 0.32 * motionScale
  const steps = 220

  for (let pass = 0; pass < 2; pass += 1) {
    const frontPass = pass === 1

    for (let i = 0; i < steps; i += 1) {
      const t = i / steps
      const angle = t * Math.PI * 2 + spin
      const radialNoise = 0.72 + 0.28 * Math.sin(angle * 4.0 - spin * 1.1)
      const radius = innerRadius + (outerRadius - innerRadius) * (0.16 + 0.84 * radialNoise)

      let x = Math.cos(angle) * radius
      let y = Math.sin(angle) * radius * verticalScale

      const visibleInPass = frontPass ? y >= 0 : y < 0
      if (!visibleInPass) continue

      const lensLift = !frontPass ? Math.exp(-Math.pow(x / (singularity.radius * 2.6), 2)) * singularity.radius * 0.56 : 0
      y -= lensLift

      const normalizedOrbit = Math.cos(angle)
      const approaching = 0.5 + 0.5 * normalizedOrbit
      const receding = 1 - approaching
      const redShift = receding * 0.42

      const arcWidth = Math.max(1, (outerRadius - radius) * 0.04 + 1.6)
      const alpha = (frontPass ? 0.13 : 0.065) * (0.45 + radialNoise * 0.9)
      const cyan = 190 + approaching * 55
      const amber = 110 + receding * 70

      ctx.strokeStyle = `rgba(${80 + redShift * 65}, ${amber + redShift * 35}, ${cyan}, ${alpha * (darkMode ? 1 : 0.45)})`
      ctx.lineWidth = arcWidth
      ctx.beginPath()
      ctx.moveTo(centerX + x * 0.994, centerY + y * 0.994)
      ctx.lineTo(centerX + x, centerY + y)
      ctx.stroke()

      if (frontPass && approaching > 0.72) {
        ctx.strokeStyle = `rgba(120, ${210 + approaching * 30}, 255, ${(0.08 + approaching * 0.07) * (darkMode ? 1 : 0.4)})`
        ctx.lineWidth = arcWidth * 1.8
        ctx.beginPath()
        ctx.moveTo(centerX + x * 0.98, centerY + y * 0.98)
        ctx.lineTo(centerX + x * 1.02, centerY + y * 1.02)
        ctx.stroke()
      }
    }
  }
}

function drawSingularityCore() {
  if (!ctx) return

  const centerX = width * singularity.x
  const centerY = height * singularity.y
  const outerGlow = singularity.radius * 1.68
  const photonRing = singularity.radius * 1.08

  const ring = ctx.createRadialGradient(centerX, centerY, singularity.radius * 0.68, centerX, centerY, outerGlow)
  ring.addColorStop(0, 'rgba(0, 0, 0, 0.98)')
  ring.addColorStop(0.58, darkMode ? 'rgba(4, 10, 18, 0.94)' : 'rgba(8, 16, 26, 0.8)')
  ring.addColorStop(0.74, darkMode ? 'rgba(55, 200, 255, 0.2)' : 'rgba(55, 200, 255, 0.08)')
  ring.addColorStop(0.86, darkMode ? 'rgba(255, 190, 110, 0.12)' : 'rgba(255, 190, 110, 0.05)')
  ring.addColorStop(1, 'rgba(0, 0, 0, 0)')
  ctx.fillStyle = ring
  ctx.beginPath()
  ctx.arc(centerX, centerY, outerGlow, 0, Math.PI * 2)
  ctx.fill()

  ctx.strokeStyle = darkMode ? 'rgba(120, 226, 255, 0.42)' : 'rgba(90, 180, 220, 0.18)'
  ctx.lineWidth = 2.2
  ctx.beginPath()
  ctx.ellipse(centerX, centerY - singularity.radius * 0.05, photonRing, photonRing * 0.86, -0.12, 0, Math.PI * 2)
  ctx.stroke()

  const core = ctx.createRadialGradient(centerX, centerY, 0, centerX, centerY, singularity.radius)
  core.addColorStop(0, 'rgba(0, 0, 0, 1)')
  core.addColorStop(0.68, 'rgba(1, 3, 8, 0.98)')
  core.addColorStop(1, 'rgba(0, 0, 0, 0)')
  ctx.fillStyle = core
  ctx.beginPath()
  ctx.arc(centerX, centerY, singularity.radius, 0, Math.PI * 2)
  ctx.fill()
}

function renderFrame(timestamp: number) {
  if (!ctx) return

  if (lastFrame === 0) {
    lastFrame = timestamp
  }

  const delta = Math.min((timestamp - lastFrame) / 1000, 0.032)
  lastFrame = timestamp
  elapsed += delta

  ctx.clearRect(0, 0, width, height)
  drawStars(elapsed)
  drawAccretionDisc(elapsed)
  drawSingularityCore()

  animationFrame = window.requestAnimationFrame(renderFrame)
}

onMounted(() => {
  updateThemeState()
  reduceMotionMedia = window.matchMedia('(prefers-reduced-motion: reduce)')
  updateMotionState()
  resizeCanvas()

  rootObserver = new MutationObserver(() => {
    updateThemeState()
  })

  rootObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class'],
  })

  reduceMotionMedia.addEventListener('change', updateMotionState)
  window.addEventListener('resize', resizeCanvas)
  animationFrame = window.requestAnimationFrame(renderFrame)
})

onBeforeUnmount(() => {
  window.cancelAnimationFrame(animationFrame)
  window.removeEventListener('resize', resizeCanvas)
  reduceMotionMedia?.removeEventListener('change', updateMotionState)
  rootObserver?.disconnect()
})
</script>

<template>
  <canvas ref="canvasRef" class="xeno-singularity-backdrop" aria-hidden="true" />
</template>

<style scoped>
.xeno-singularity-backdrop {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  opacity: 0.96;
  mix-blend-mode: normal;
  filter: saturate(1.08) brightness(1.02);
}

:global(.dark) .xeno-singularity-backdrop {
  opacity: 1;
}

@media (prefers-reduced-motion: reduce) {
  .xeno-singularity-backdrop {
    opacity: 0.82;
  }
}
</style>
