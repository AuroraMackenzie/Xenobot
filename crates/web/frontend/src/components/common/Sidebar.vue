<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { ref, computed, onMounted, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import type { AnalysisSession } from '@/types/base'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import 'dayjs/locale/zh-cn'
import 'dayjs/locale/en'
import SidebarButton from './sidebar/SidebarButton.vue'
import SidebarFooter from './sidebar/SidebarFooter.vue'
import { useSessionStore } from '@/stores/session'
import { useLayoutStore } from '@/stores/layout'

dayjs.extend(relativeTime)
const { t } = useI18n()

const sessionStore = useSessionStore()
const layoutStore = useLayoutStore()
const { sessions, sortedSessions } = storeToRefs(sessionStore)
const { isSidebarCollapsed: isCollapsed } = storeToRefs(layoutStore)
const { toggleSidebar } = layoutStore
const router = useRouter()
const route = useRoute()

// English engineering note.
const isHomePage = computed(() => route.path === '/')

// English engineering note.
const showRenameModal = ref(false)
const renameTarget = ref<AnalysisSession | null>(null)
const newName = ref('')
const renameInputRef = ref<HTMLInputElement | null>(null)

// English engineering note.
const showDeleteModal = ref(false)
const deleteTarget = ref<AnalysisSession | null>(null)

// English engineering note.
const version = ref('')

// English engineering note.
const showSearch = ref(false)
const searchQuery = ref('')

// English engineering note.
const filteredSortedSessions = computed(() => {
  if (!searchQuery.value.trim()) {
    return sortedSessions.value
  }
  const query = searchQuery.value.toLowerCase().trim()
  return sortedSessions.value.filter((s) => s.name.toLowerCase().includes(query))
})

// English engineering note.
function toggleSearch() {
  showSearch.value = !showSearch.value
  if (!showSearch.value) {
    searchQuery.value = ''
  }
}

// English engineering note.
onMounted(async () => {
  sessionStore.loadSessions()
  try {
    version.value = await window.api.app.getVersion()
  } catch (e) {
    console.error('Failed to get version', e)
  }
})

function handleImport() {
  // Navigate to home (Welcome Guide)
  router.push('/')
}

function formatTime(timestamp: number): string {
  return dayjs.unix(timestamp).fromNow()
}

// English engineering note.
function openRenameModal(session: AnalysisSession) {
  renameTarget.value = session
  newName.value = session.name
  showRenameModal.value = true
  // English engineering note.
  nextTick(() => {
    renameInputRef.value?.focus()
    renameInputRef.value?.select()
  })
}

// English engineering note.
async function handleRename() {
  if (!renameTarget.value || !newName.value.trim()) return

  const success = await sessionStore.renameSession(renameTarget.value.id, newName.value.trim())
  if (success) {
    showRenameModal.value = false
    renameTarget.value = null
    newName.value = ''
  }
}

// English engineering note.
function closeRenameModal() {
  showRenameModal.value = false
  renameTarget.value = null
  newName.value = ''
}

// English engineering note.
function openDeleteModal(session: AnalysisSession) {
  deleteTarget.value = session
  showDeleteModal.value = true
}

// English engineering note.
async function confirmDelete() {
  if (!deleteTarget.value) return

  await sessionStore.deleteSession(deleteTarget.value.id)
  showDeleteModal.value = false
  deleteTarget.value = null
}

// English engineering note.
function closeDeleteModal() {
  showDeleteModal.value = false
  deleteTarget.value = null
}

// English engineering note.
function getContextMenuItems(session: AnalysisSession) {
  const isPinned = sessionStore.isPinned(session.id)
  return [
    [
      {
        label: isPinned ? t('layout.contextMenu.unpin') : t('layout.contextMenu.pin'),
        class: 'p-2',
        onSelect: () => sessionStore.togglePinSession(session.id),
      },
      {
        label: t('layout.contextMenu.rename'),
        class: 'p-2',
        onSelect: () => openRenameModal(session),
      },
      {
        label: t('layout.contextMenu.delete'),
        color: 'error' as const,
        class: 'p-2',
        onSelect: () => openDeleteModal(session),
      },
    ],
  ]
}

// English engineering note.
function getSessionRouteName(session: AnalysisSession): string {
  return session.type === 'private' ? 'direct-room' : 'circle-room'
}

// English engineering note.
function isPrivateChat(session: AnalysisSession): boolean {
  return session.type === 'private'
}

// English engineering note.
function getSessionAvatarText(session: AnalysisSession): string {
  const name = session.name || ''
  if (!name) return '?'
  if (isPrivateChat(session)) {
    // English engineering note.
    return name.length <= 2 ? name : name.slice(-2)
  } else {
    // English engineering note.
    return name.length <= 2 ? name : name.slice(0, 2)
  }
}

// English engineering note.
function getSessionAvatar(session: AnalysisSession): string | null {
  if (isPrivateChat(session)) {
    return session.memberAvatar || null
  }
  return session.groupAvatar || null
}
</script>

<template>
  <div
    class="xeno-sidebar-shell flex h-full flex-col transition-all duration-300 ease-in-out"
    :class="[isCollapsed ? 'w-20' : 'w-72', isHomePage ? 'xeno-sidebar-home' : 'xeno-sidebar-default']"
  >
    <div class="flex flex-col p-4 pt-5">
      <!-- Header -->
      <div
        class="mb-2 flex items-center"
        :class="[isCollapsed ? 'justify-center' : 'justify-between']"
        style="-webkit-app-region: drag"
      >
        <div v-if="!isCollapsed" class="xeno-sidebar-brand ml-2">
          <div class="xeno-sidebar-brand-mark">
            <span class="xeno-sidebar-brand-dot" />
            <div class="text-2xl font-black tracking-tight text-cyan-600 dark:text-cyan-400">
              {{ t('layout.brand') }}
            </div>
          </div>
          <div class="xeno-sidebar-brand-meta">
            <span class="xeno-sidebar-version">v{{ version }}</span>
            <span v-if="sessions.length > 0" class="xeno-sidebar-count">{{ sessions.length }}</span>
          </div>
        </div>
        <UTooltip
          :text="isCollapsed ? t('layout.tooltip.expand') : t('layout.tooltip.collapse')"
          :popper="{ placement: 'right' }"
          style="-webkit-app-region: no-drag"
        >
          <UButton
            icon="i-heroicons-bars-3"
            color="gray"
            variant="ghost"
            size="md"
            class="flex h-12 w-12 cursor-pointer items-center justify-center rounded-full hover:bg-gray-200/60 dark:hover:bg-gray-800"
            @click="toggleSidebar"
          />
        </UTooltip>
      </div>

      <!-- English UI note -->
      <SidebarButton icon="i-heroicons-plus" :title="t('layout.newAnalysis')" @click="handleImport" />
    </div>

    <!-- Session List -->
    <div class="flex-1 relative min-h-0 flex flex-col">
      <!-- English UI note -->
      <div v-if="!isCollapsed && sessions.length > 0" class="px-4 mb-2">
        <div class="xeno-sidebar-section-head flex items-center justify-between">
          <UTooltip :text="t('layout.tooltip.hint')" :popper="{ placement: 'right' }">
            <div class="flex items-center gap-1 pl-3">
              <div class="text-sm font-medium text-gray-500">{{ t('layout.chatHistory') }}</div>
              <UIcon name="i-heroicons-question-mark-circle" class="size-3.5 text-gray-400" />
            </div>
          </UTooltip>
          <UTooltip :text="t('layout.tooltip.search')" :popper="{ placement: 'right' }">
            <UButton
              :icon="showSearch ? 'i-heroicons-x-mark' : 'i-heroicons-magnifying-glass'"
              color="neutral"
              variant="ghost"
              size="xs"
              @click="toggleSearch"
            />
          </UTooltip>
        </div>
        <!-- English UI note -->
        <div v-if="showSearch" class="xeno-sidebar-search mt-2">
          <UInput
            v-model="searchQuery"
            :placeholder="t('layout.searchPlaceholder')"
            icon="i-heroicons-magnifying-glass"
            size="sm"
            autofocus
          />
        </div>
      </div>

      <!-- English UI note -->
      <div class="xeno-sidebar-scroll flex-1 overflow-y-auto">
        <div v-if="sessions.length === 0 && !isCollapsed" class="py-8 text-center text-sm text-gray-500">
          {{ t('layout.noRecords') }}
        </div>

        <!-- English UI note -->
        <div
          v-else-if="filteredSortedSessions.length === 0 && searchQuery.trim() && !isCollapsed"
          class="py-8 text-center text-sm text-gray-500"
        >
          {{ t('layout.noSearchResult') }}
        </div>

        <div class="space-y-1 pb-8" :class="[isCollapsed ? '' : 'px-4']">
          <UContextMenu
            v-for="session in filteredSortedSessions"
            :key="session.id"
            :items="getContextMenuItems(session)"
          >
            <!-- English UI note -->
            <UTooltip :text="session.name" :disabled="!isCollapsed || !session.name" :popper="{ placement: 'right' }">
              <div
                class="xeno-session-item group relative flex items-center p-2 text-left transition-colors"
                :class="[
                  route.params.id === session.id && !isCollapsed
                    ? 'xeno-session-item-active text-gray-900 dark:text-primary-100'
                    : 'xeno-session-item-idle text-gray-700 dark:text-gray-200',
                  isCollapsed
                    ? 'justify-center cursor-pointer h-13 w-13 rounded-[1.35rem] ml-3.5'
                    : 'cursor-pointer w-full rounded-2xl',
                ]"
                @click="router.push({ name: getSessionRouteName(session), params: { id: session.id } })"
              >
                <span class="xeno-session-item-rail" aria-hidden="true" />
                <!-- English UI note -->
                <!-- English UI note -->
                <img
                  v-if="getSessionAvatar(session)"
                  :src="getSessionAvatar(session)!"
                  :alt="session.name"
                  class="h-9 w-9 min-w-9 shrink-0 rounded-full object-cover"
                  :class="[isCollapsed ? '' : 'mr-3']"
                />
                <!-- English UI note -->
                <div
                  v-else
                  class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-[10px] font-bold"
                  :class="[
                    route.params.id === session.id
                      ? isPrivateChat(session)
                        ? 'bg-pink-600 text-white dark:bg-pink-500 dark:text-white'
                        : 'bg-primary-600 text-white dark:bg-primary-500 dark:text-white'
                      : 'bg-gray-400 text-white dark:bg-gray-600 dark:text-white',
                    isCollapsed ? '' : 'mr-3',
                  ]"
                >
                  <!-- English UI note -->
                  <template v-if="isCollapsed">
                    {{ getSessionAvatarText(session) }}
                  </template>
                  <template v-else>
                    <UIcon
                      :name="isPrivateChat(session) ? 'i-heroicons-user' : 'i-heroicons-chat-bubble-left-right'"
                      class="h-4 w-4"
                    />
                  </template>
                </div>

                <!-- Session Info -->
                <div v-if="!isCollapsed" class="min-w-0 flex-1">
                  <div class="flex items-center justify-between gap-2">
                    <p class="truncate text-sm font-medium">
                      {{ session.name }}
                    </p>
                    <UIcon
                      v-if="sessionStore.isPinned(session.id)"
                      name="i-lucide-pin"
                      class="h-3.5 w-3.5 shrink-0 text-gray-400 rotate-45"
                    />
                  </div>
                  <p class="truncate text-xs text-gray-500 dark:text-gray-400">
                    {{ t('layout.sessionInfo', { count: session.messageCount, time: formatTime(session.importedAt) }) }}
                  </p>
                </div>
              </div>
            </UTooltip>
          </UContextMenu>
        </div>
      </div>
      <!-- English UI note -->
      <div class="xeno-sidebar-fade pointer-events-none absolute bottom-0 left-0 right-0 h-12" />
    </div>

    <!-- Rename Modal -->
    <UModal v-model:open="showRenameModal">
      <template #content>
        <div class="p-4">
          <h3 class="mb-3 font-semibold text-gray-900 dark:text-white">{{ t('layout.renameModal.title') }}</h3>
          <UInput
            ref="renameInputRef"
            v-model="newName"
            :placeholder="t('layout.renameModal.placeholder')"
            class="mb-4 w-100"
            @keydown.enter="handleRename"
          />
          <div class="flex justify-end gap-2">
            <UButton variant="soft" @click="closeRenameModal">{{ t('common.cancel') }}</UButton>
            <UButton color="primary" :disabled="!newName.trim()" @click="handleRename">
              {{ t('common.confirm') }}
            </UButton>
          </div>
        </div>
      </template>
    </UModal>

    <!-- Delete Confirmation Modal -->
    <UModal v-model:open="showDeleteModal">
      <template #content>
        <div class="p-4">
          <h3 class="mb-3 font-semibold text-gray-900 dark:text-white">{{ t('layout.deleteModal.title') }}</h3>
          <p class="mb-4 text-sm text-gray-600 dark:text-gray-400">
            {{ t('layout.deleteModal.message', { name: deleteTarget?.name }) }}
          </p>
          <div class="flex justify-end gap-2">
            <UButton variant="soft" @click="closeDeleteModal">{{ t('common.cancel') }}</UButton>
            <UButton color="error" @click="confirmDelete">{{ t('common.delete') }}</UButton>
          </div>
        </div>
      </template>
    </UModal>

    <!-- Footer -->
    <SidebarFooter />
  </div>
</template>

<style scoped>
.xeno-sidebar-shell {
  position: relative;
  border-right: 1px solid var(--xeno-border-strong);
  background: var(--xeno-sidebar-bg);
  backdrop-filter: blur(16px) saturate(130%);
  box-shadow:
    inset -1px 0 0 rgba(255, 255, 255, 0.04),
    18px 0 42px -34px rgba(2, 6, 23, 0.38);
  overflow: hidden;
}

.xeno-sidebar-shell::before {
  content: '';
  position: absolute;
  inset: 0 0 auto 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(56, 189, 248, 0.34), transparent);
  opacity: 0.78;
}

.xeno-sidebar-shell::after {
  content: '';
  position: absolute;
  top: -7rem;
  right: -7rem;
  width: 16rem;
  height: 16rem;
  border-radius: 9999px;
  background: radial-gradient(circle, rgba(56, 189, 248, 0.12), transparent 72%);
  filter: blur(16px);
  opacity: 0.8;
  pointer-events: none;
}

.xeno-sidebar-home {
  background: var(--xeno-surface-muted);
}

.xeno-sidebar-default {
  background: var(--xeno-sidebar-bg);
}

.xeno-session-item {
  overflow: hidden;
  border: 1px solid transparent;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 130%),
    transparent;
  backdrop-filter: blur(10px) saturate(122%);
}

.xeno-sidebar-brand {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.xeno-sidebar-brand-mark {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.xeno-sidebar-brand-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 9999px;
  background: linear-gradient(135deg, rgba(45, 212, 191, 0.92), rgba(14, 165, 233, 0.9));
  box-shadow: 0 0 0 4px rgba(34, 211, 238, 0.1);
}

.xeno-sidebar-brand-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding-left: 1.1rem;
}

.xeno-sidebar-version,
.xeno-sidebar-count {
  border: 1px solid var(--xeno-border-soft);
  background: rgba(255, 255, 255, 0.03);
  border-radius: 9999px;
  padding: 0.1rem 0.48rem;
  font-size: 0.66rem;
  line-height: 1rem;
  color: var(--xeno-text-secondary);
}

.xeno-sidebar-section-head {
  min-height: 2rem;
}

.xeno-sidebar-search :deep(input) {
  border-radius: 1rem;
}

.xeno-sidebar-scroll {
  position: relative;
}

.xeno-session-item-rail {
  position: absolute;
  left: 0;
  top: 0.55rem;
  bottom: 0.55rem;
  width: 2px;
  border-radius: 9999px;
  background: linear-gradient(180deg, rgba(45, 212, 191, 0.88), rgba(56, 189, 248, 0.92));
  opacity: 0;
  transform: translateX(-4px);
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.xeno-session-item::after {
  content: '';
  position: absolute;
  inset: 0 auto auto 0;
  width: 100%;
  height: 1px;
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.12), transparent 56%);
  opacity: 0.5;
}

.xeno-session-item-idle:hover {
  border-color: var(--xeno-border-soft);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.04), transparent 120%),
    var(--xeno-hover-bg);
}

.xeno-session-item-active {
  border-color: var(--xeno-active-border);
  background:
    linear-gradient(180deg, rgba(56, 189, 248, 0.08), transparent 120%),
    var(--xeno-active-bg);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.06),
    0 14px 32px -26px rgba(14, 165, 233, 0.52);
}

.xeno-session-item:hover .xeno-session-item-rail,
.xeno-session-item-active .xeno-session-item-rail {
  opacity: 1;
  transform: translateX(0);
}

.xeno-sidebar-fade {
  background: linear-gradient(180deg, transparent, var(--xeno-sidebar-bg));
}
</style>
