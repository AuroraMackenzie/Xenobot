<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MemberWithStats } from '@/types/analysis'
import OwnerSelector from '@/components/analysis/member/OwnerSelector.vue'

const { t } = useI18n()

// Props
const props = defineProps<{
  sessionId: string
}>()

// Emits
const emit = defineEmits<{
  'data-changed': []
}>()

// English engineering note.
const members = ref<MemberWithStats[]>([])
const allMembers = ref<MemberWithStats[]>([]) // English engineering note.
const isLoading = ref(false)
const searchQuery = ref('')

// English engineering note.
const deletingMember = ref<MemberWithStats | null>(null)
const isDeleting = ref(false)

// English engineering note.
const pageSize = 20
const currentPage = ref(1)
const total = ref(0)
const totalPages = ref(0)

// English engineering note.
const sortOrder = ref<'desc' | 'asc'>('desc') // English engineering note.

// English engineering note.
const savingAliasesId = ref<number | null>(null)

// English engineering note.
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null

// English engineering note.
function getDisplayName(member: MemberWithStats): string {
  return member.groupNickname || member.accountName || member.platformId
}

// English engineering note.
function getFirstChar(member: MemberWithStats): string {
  const name = getDisplayName(member)
  return name.slice(0, 1)
}

// English engineering note.
function toggleSort() {
  sortOrder.value = sortOrder.value === 'desc' ? 'asc' : 'desc'
  currentPage.value = 1
  loadMembers()
}

// English engineering note.
async function loadMembers() {
  if (!props.sessionId) return
  isLoading.value = true
  try {
    const result = await window.chatApi.getMembersPaginated(props.sessionId, {
      page: currentPage.value,
      pageSize,
      search: searchQuery.value.trim(),
      sortOrder: sortOrder.value,
    })
    members.value = result.members
    total.value = result.total
    totalPages.value = result.totalPages
  } catch (error) {
    console.error('加载成员列表失败:', error)
  } finally {
    isLoading.value = false
  }
}

// English engineering note.
async function loadAllMembers() {
  if (!props.sessionId) return
  try {
    allMembers.value = await window.chatApi.getMembers(props.sessionId)
  } catch (error) {
    console.error('加载所有成员失败:', error)
  }
}

// English engineering note.
async function updateAliases(member: MemberWithStats, newAliases: string[]) {
  // English engineering note.
  const aliasesToSave = JSON.parse(JSON.stringify(newAliases)) as string[]

  // English engineering note.
  const currentAliases = JSON.stringify(member.aliases)
  const newAliasesStr = JSON.stringify(aliasesToSave)
  if (currentAliases === newAliasesStr) return

  savingAliasesId.value = member.id
  try {
    const success = await window.chatApi.updateMemberAliases(props.sessionId, member.id, aliasesToSave)
    if (success) {
      // English engineering note.
      const idx = members.value.findIndex((m) => m.id === member.id)
      if (idx !== -1) {
        members.value[idx] = {
          ...members.value[idx],
          aliases: aliasesToSave,
        }
      }
    }
  } catch (error) {
    console.error('保存别名失败:', error)
  } finally {
    savingAliasesId.value = null
  }
}

// English engineering note.
function showDeleteConfirm(member: MemberWithStats) {
  deletingMember.value = member
}

// English engineering note.
function cancelDelete() {
  deletingMember.value = null
}

// English engineering note.
async function confirmDelete() {
  if (!deletingMember.value) return
  isDeleting.value = true
  try {
    const success = await window.chatApi.deleteMember(props.sessionId, deletingMember.value.id)
    if (success) {
      // English engineering note.
      await loadMembers()
      // English engineering note.
      await loadAllMembers()
      // English engineering note.
      emit('data-changed')
    }
  } catch (error) {
    console.error('删除成员失败:', error)
  } finally {
    isDeleting.value = false
    deletingMember.value = null
  }
}

// English engineering note.
watch(searchQuery, () => {
  currentPage.value = 1
  // English engineering note.
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
  searchDebounceTimer = setTimeout(() => {
    loadMembers()
  }, 300)
})

// English engineering note.
watch(currentPage, () => {
  loadMembers()
})

// English engineering note.
watch(
  () => props.sessionId,
  () => {
    searchQuery.value = ''
    currentPage.value = 1
    loadMembers()
    loadAllMembers()
  },
  { immediate: true }
)

onMounted(() => {
  loadMembers()
  loadAllMembers()
})
</script>

<template>
  <div class="main-content max-w-5xl p-6">
    <!-- English UI note -->
    <div class="mb-6">
      <div class="flex items-center gap-3">
        <div>
          <h2 class="text-xl font-bold text-gray-900 dark:text-white">{{ t('members.list.title') }}</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {{ t('members.list.description', { count: total }) }}
          </p>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <OwnerSelector
      class="mb-6"
      :session-id="sessionId"
      :members="allMembers"
      :is-loading="isLoading"
      chat-type="group"
    />

    <!-- English UI note -->
    <div class="mb-4">
      <UInput
        v-model="searchQuery"
        :placeholder="t('members.list.searchPlaceholder')"
        icon="i-heroicons-magnifying-glass"
        class="w-100"
      >
        <template v-if="searchQuery" #trailing>
          <UButton icon="i-heroicons-x-mark" variant="link" color="neutral" size="xs" @click="searchQuery = ''" />
        </template>
      </UInput>
    </div>

    <!-- English UI note -->
    <div class="rounded-xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-900">
      <!-- English UI note -->
      <div v-if="isLoading" class="flex h-60 items-center justify-center">
        <UIcon name="i-heroicons-arrow-path" class="h-8 w-8 animate-spin text-pink-500" />
      </div>

      <!-- English UI note -->
      <div v-else-if="members.length === 0" class="flex h-60 flex-col items-center justify-center">
        <UIcon name="i-heroicons-user-group" class="mb-3 h-12 w-12 text-gray-300 dark:text-gray-600" />
        <p class="text-gray-500 dark:text-gray-400">
          {{ searchQuery ? t('members.list.noMatch') : t('members.list.empty') }}
        </p>
      </div>

      <!-- English UI note -->
      <div v-else>
        <div class="max-h-[500px] overflow-y-auto">
          <table class="w-full">
            <thead class="sticky top-0 bg-gray-50 dark:bg-gray-800">
              <tr class="text-left text-xs font-medium uppercase text-gray-500 dark:text-gray-400">
                <th class="px-4 py-4">{{ t('members.list.table.accountName') }}</th>
                <th class="px-4 py-4">{{ t('members.list.table.groupNickname') }}</th>
                <th class="px-4 py-4">
                  <button
                    class="flex items-center gap-1.5 hover:text-gray-700 dark:hover:text-gray-200"
                    @click="toggleSort"
                  >
                    {{ t('members.list.table.messageCount') }}
                    <UIcon
                      :name="sortOrder === 'desc' ? 'i-heroicons-arrow-down' : 'i-heroicons-arrow-up'"
                      class="h-3.5 w-3.5"
                    />
                  </button>
                </th>
                <th class="px-4 py-4 w-64">{{ t('members.list.table.customAlias') }}</th>
                <th class="px-4 py-4 text-right">{{ t('members.list.table.actions') }}</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
              <tr v-for="member in members" :key="member.id" class="hover:bg-gray-50 dark:hover:bg-gray-800/50">
                <!-- English UI note -->
                <td class="px-4 py-4">
                  <div class="flex items-center gap-2">
                    <!-- English UI note -->
                    <img
                      v-if="member.avatar"
                      :src="member.avatar"
                      :alt="getDisplayName(member)"
                      class="h-8 w-8 shrink-0 rounded-full object-cover"
                    />
                    <div
                      v-else
                      class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-pink-400 to-pink-600 text-xs font-medium text-white"
                    >
                      {{ getFirstChar(member) }}
                    </div>
                    <div>
                      <span class="text-sm font-medium text-gray-900 dark:text-white">
                        {{ member.accountName || '-' }}
                      </span>
                      <span class="ml-1 text-sm text-gray-500 dark:text-gray-400">({{ member.platformId }})</span>
                    </div>
                  </div>
                </td>

                <!-- English UI note -->
                <td class="px-4 py-4">
                  <span v-if="member.groupNickname" class="text-sm font-medium text-gray-900 dark:text-white">
                    {{ member.groupNickname }}
                  </span>
                  <span v-else class="text-sm text-gray-400 dark:text-gray-500">-</span>
                </td>

                <!-- English UI note -->
                <td class="px-4 py-4">
                  <span class="text-sm font-semibold text-gray-900 dark:text-white">
                    {{ member.messageCount.toLocaleString() }}
                  </span>
                </td>

                <!-- English UI note -->
                <td class="px-4 py-4">
                  <div class="max-w-xs">
                    <UInputTags
                      :model-value="member.aliases"
                      :placeholder="t('members.list.aliasPlaceholder')"
                      class="w-80"
                      @update:model-value="(val) => updateAliases(member, val)"
                    />
                    <!-- English UI note -->
                    <div v-if="savingAliasesId === member.id" class="absolute right-2 top-1/2 -translate-y-1/2">
                      <UIcon name="i-heroicons-arrow-path" class="h-4 w-4 animate-spin text-pink-500" />
                    </div>
                  </div>
                </td>

                <!-- English UI note -->
                <td class="px-4 py-4 text-right">
                  <UButton :label="t('members.list.delete')" size="xs" @click="showDeleteConfirm(member)" />
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- English UI note -->
        <div
          v-if="totalPages > 1"
          class="flex items-center justify-between border-t border-gray-200 px-6 py-4 dark:border-gray-700"
        >
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {{
              t('members.list.pagination', {
                start: (currentPage - 1) * pageSize + 1,
                end: Math.min(currentPage * pageSize, total),
                total: total,
              })
            }}
          </p>
          <div class="flex items-center gap-2">
            <UButton
              icon="i-heroicons-chevron-left"
              variant="outline"
              size="sm"
              :disabled="currentPage === 1 || isLoading"
              @click="currentPage--"
            />
            <span class="text-sm font-medium text-gray-600 dark:text-gray-300">
              {{ currentPage }} / {{ totalPages }}
            </span>
            <UButton
              icon="i-heroicons-chevron-right"
              variant="outline"
              size="sm"
              :disabled="currentPage >= totalPages || isLoading"
              @click="currentPage++"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- English UI note -->
    <div class="mt-4 flex items-start gap-3 rounded-xl bg-amber-50 p-4 dark:bg-amber-900/20">
      <UIcon name="i-heroicons-exclamation-triangle" class="mt-0.5 h-5 w-5 shrink-0 text-amber-500" />
      <div>
        <p class="text-sm font-medium text-amber-800 dark:text-amber-200">
          {{ t('members.list.tip') }}
        </p>
      </div>
    </div>

    <!-- English UI note -->
    <UModal :open="!!deletingMember" :ui="{ content: 'max-w-sm' }" @update:open="deletingMember = null">
      <template #content>
        <div class="p-6 text-center">
          <div
            class="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-full bg-red-100 dark:bg-red-900/30"
          >
            <UIcon name="i-heroicons-exclamation-triangle" class="h-7 w-7 text-red-500" />
          </div>
          <h3 class="mb-2 text-lg font-semibold text-gray-900 dark:text-white">{{ t('members.list.modal.title') }}</h3>
          <p class="mb-6 text-sm text-gray-500 dark:text-gray-400">
            {{
              t('members.list.modal.content', {
                name: deletingMember ? getDisplayName(deletingMember) : '',
                count: deletingMember?.messageCount.toLocaleString(),
              })
            }}
          </p>
          <div class="flex justify-center gap-3">
            <UButton variant="outline" @click="cancelDelete">{{ t('members.list.modal.cancel') }}</UButton>
            <UButton color="error" :loading="isDeleting" @click="confirmDelete">
              {{ t('members.list.modal.confirm') }}
            </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
