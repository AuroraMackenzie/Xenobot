<script setup lang="ts">
import { ref, onMounted } from 'vue'

// English engineering note.
const isMac = ref(false)
const isMaximized = ref(false)

// English engineering note.
const isWindows = ref(false)

onMounted(() => {
  isMac.value = navigator.platform.toLowerCase().includes('mac')
  isWindows.value = navigator.platform.toLowerCase().includes('win')

  // English engineering note.
  window.electron?.ipcRenderer?.on('windowState', (_: unknown, maximized: boolean) => {
    isMaximized.value = maximized
  })
})

// English engineering note.
function minimize() {
  window.electron?.ipcRenderer?.send('window-min')
}

function maximize() {
  window.electron?.ipcRenderer?.send('window-maxOrRestore')
}

function close() {
  window.electron?.ipcRenderer?.send('window-close')
}
</script>

<template>
  <div class="title-bar">
    <!-- English UI note -->
    <div v-if="isMac" class="traffic-light-spacer" />

    <!-- English UI note -->
    <div class="drag-region" />

    <!-- English UI note -->
    <!-- English UI note -->
    <div v-if="!isMac && !isWindows" class="window-controls">
      <!-- English UI note -->
      <button class="control-btn" @click="minimize">
        <svg width="10" height="1" viewBox="0 0 10 1">
          <path d="M0 0h10v1H0z" fill="currentColor" />
        </svg>
      </button>

      <!-- English UI note -->
      <button class="control-btn" @click="maximize">
        <svg v-if="!isMaximized" width="10" height="10" viewBox="0 0 10 10">
          <path d="M0 0v10h10V0H0zm1 1h8v8H1V1z" fill="currentColor" />
        </svg>
        <svg v-else width="10" height="10" viewBox="0 0 10 10">
          <path d="M2 0v2H0v8h8V8h2V0H2zm1 3h5v5H1V3h2z" fill="currentColor" />
        </svg>
      </button>

      <!-- English UI note -->
      <button class="control-btn control-btn-close" @click="close">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <path
            d="M1.41 0L0 1.41 3.59 5 0 8.59 1.41 10 5 6.41 8.59 10 10 8.59 6.41 5 10 1.41 8.59 0 5 3.59z"
            fill="currentColor"
          />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.title-bar {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 32px;
  display: flex;
  align-items: center;
  z-index: 9999;
  -webkit-app-region: drag;
}

/* Platform spacer reserved for native traffic lights. */
.traffic-light-spacer {
  width: 70px;
  height: 100%;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
}

/* Stretch region keeps the window draggable outside controls. */
.drag-region {
  flex: 1;
  height: 100%;
}

/* Desktop window controls are shown on non-mac, non-windows shells. */
.window-controls {
  display: flex;
  height: 100%;
  -webkit-app-region: no-drag;
}

/* Shared control button surface. */
.control-btn {
  width: 46px;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--color-gray-600);
  cursor: pointer;
  transition: background-color 0.15s;
}

.control-btn:hover {
  background-color: rgba(0, 0, 0, 0.1);
}

/* Dark mode keeps contrast without adding noisy chrome. */
:global(.dark) .control-btn {
  color: var(--color-gray-400);
}

:global(.dark) .control-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

/* Close action uses the destructive hover color. */
.control-btn-close:hover {
  background-color: #e81123;
  color: white;
}

:global(.dark) .control-btn-close:hover {
  background-color: #e81123;
  color: white;
}
</style>
