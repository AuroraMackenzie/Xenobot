import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { AnalysisSession, ImportProgress } from '@/types/base'

/** English note.
export interface MigrationInfo {
  version: number
  /** English note.
  description: string
  /** English note.
  userMessage: string
}

/** English note.
export interface MigrationCheckResult {
  needsMigration: boolean
  count: number
  currentVersion: number
  pendingMigrations: MigrationInfo[]
}

/** English note.
export type BatchFileStatus = 'pending' | 'importing' | 'success' | 'failed' | 'cancelled'

/** English note.
export interface BatchFileInfo {
  path: string
  name: string
  status: BatchFileStatus
  progress?: ImportProgress
  error?: string
  diagnosisSuggestion?: string
  sessionId?: string
}

/** English note.
export interface BatchImportResult {
  total: number
  success: number
  failed: number
  cancelled: number
  files: BatchFileInfo[]
}

/** English note.
export type MergeFileStatus = 'pending' | 'parsing' | 'done'

/** English note.
export interface MergeFileInfo {
  path: string
  name: string
  status: MergeFileStatus
  info?: {
    name: string
    format: string
    platform: string
    messageCount: number
    memberCount: number
    fileSize?: number
  }
}

/** English note.
export type MergeImportStage = 'parsing' | 'merging' | 'done' | 'error'

/** English note.
export interface MergeImportResult {
  success: boolean
  sessionId?: string
  error?: string
}

/**
 * English note.
 */
export const useSessionStore = defineStore(
  'session',
  () => {
    // English engineering note.
    const sessions = ref<AnalysisSession[]>([])
    // English engineering note.
    const currentSessionId = ref<string | null>(null)
    // English engineering note.
    const isImporting = ref(false)
    const importProgress = ref<ImportProgress | null>(null)
    // English engineering note.
    const isInitialized = ref(false)

    // English engineering note.
    const isBatchImporting = ref(false)
    const batchFiles = ref<BatchFileInfo[]>([])
    const batchImportCancelled = ref(false)
    const batchImportResult = ref<BatchImportResult | null>(null)

    // English engineering note.
    const isMergeImporting = ref(false)
    const mergeFiles = ref<MergeFileInfo[]>([])
    const mergeStage = ref<MergeImportStage>('parsing')
    const mergeError = ref<string | null>(null)
    const mergeResult = ref<MergeImportResult | null>(null)

    // English engineering note.
    const currentSession = computed(() => {
      if (!currentSessionId.value) return null
      return sessions.value.find((s) => s.id === currentSessionId.value) || null
    })

    // English engineering note.
    const migrationNeeded = ref(false)
    const migrationCount = ref(0)
    const pendingMigrations = ref<MigrationInfo[]>([])
    const isMigrating = ref(false)

    /**
     * English note.
     */
    async function checkMigration(): Promise<MigrationCheckResult> {
      try {
        const result = await window.chatApi.checkMigration()
        migrationNeeded.value = result.needsMigration
        migrationCount.value = result.count
        pendingMigrations.value = result.pendingMigrations || []
        return result
      } catch (error) {
        console.error('检查迁移失败:', error)
        return { needsMigration: false, count: 0, currentVersion: 0, pendingMigrations: [] }
      }
    }

    /**
     * English note.
     */
    async function runMigration(): Promise<{ success: boolean; error?: string }> {
      isMigrating.value = true
      try {
        const result = await window.chatApi.runMigration()
        if (result.success) {
          migrationNeeded.value = false
          migrationCount.value = 0
        }
        return result
      } catch (error) {
        console.error('执行迁移失败:', error)
        return { success: false, error: String(error) }
      } finally {
        isMigrating.value = false
      }
    }

    /**
     * English note.
     */
    async function loadSessions() {
      try {
        const list = await window.chatApi.getSessions()
        sessions.value = list
        // English engineering note.
        if (currentSessionId.value && !list.find((s) => s.id === currentSessionId.value)) {
          currentSessionId.value = null
        }
        isInitialized.value = true
      } catch (error) {
        console.error('加载会话列表失败:', error)
        isInitialized.value = true
      }
    }

    /**
     * English note.
     */
    async function importFile(): Promise<{
      success: boolean
      error?: string
      diagnosisSuggestion?: string
    }> {
      try {
        const result = await window.chatApi.selectFile()
        // English engineering note.
        if (!result) {
          return { success: false, error: 'error.no_file_selected' }
        }
        // English engineering note.
        if (result.error) {
          const diagnosisSuggestion = result.diagnosis?.suggestion
          return { success: false, error: result.error, diagnosisSuggestion }
        }
        // English engineering note.
        if (!result.filePath) {
          return { success: false, error: 'error.no_file_selected' }
        }
        return await importFileFromPath(result.filePath)
      } catch (error) {
        return { success: false, error: String(error) }
      }
    }

    /** English note.
    interface ImportDiagnosticsInfo {
      logFile: string | null
      detectedFormat: string | null
      messagesReceived: number
      messagesWritten: number
      messagesSkipped: number
      skipReasons: {
        noSenderId: number
        noAccountName: number
        invalidTimestamp: number
        noType: number
      }
    }

    /**
     * English note.
     */
    async function importFileFromPath(filePath: string): Promise<{
      success: boolean
      error?: string
      diagnosisSuggestion?: string
      diagnostics?: ImportDiagnosticsInfo
    }> {
      try {
        isImporting.value = true
        importProgress.value = {
          stage: 'detecting',
          progress: 0,
          message: '', // Progress text is handled by frontend i18n
        }

        // English engineering note.
        const queue: ImportProgress[] = []
        let isProcessing = false
        let currentStage = 'reading'
        let lastStageTime = Date.now()
        const MIN_STAGE_TIME = 1000

        /**
         * English note.
         */
        const processQueue = async () => {
          if (isProcessing) return
          isProcessing = true

          while (queue.length > 0) {
            const next = queue[0]

            if (next.stage !== currentStage) {
              const elapsed = Date.now() - lastStageTime
              if (elapsed < MIN_STAGE_TIME) {
                await new Promise((resolve) => setTimeout(resolve, MIN_STAGE_TIME - elapsed))
              }
              currentStage = next.stage
              lastStageTime = Date.now()
            }

            importProgress.value = queue.shift()!
          }
          isProcessing = false
        }

        const unsubscribe = window.chatApi.onImportProgress((progress) => {
          if (progress.stage === 'done') return
          queue.push(progress)
          processQueue()
        })

        const importResult = await window.chatApi.import(filePath)
        unsubscribe()

        while (queue.length > 0 || isProcessing) {
          await new Promise((resolve) => setTimeout(resolve, 100))
        }

        const elapsed = Date.now() - lastStageTime
        if (elapsed < MIN_STAGE_TIME) {
          await new Promise((resolve) => setTimeout(resolve, MIN_STAGE_TIME - elapsed))
        }

        if (importProgress.value) {
          importProgress.value.progress = 100
        }
        await new Promise((resolve) => setTimeout(resolve, 300))

        if (importResult.success && importResult.sessionId) {
          await loadSessions()
          currentSessionId.value = importResult.sessionId

          // English engineering note.
          try {
            const savedThreshold = localStorage.getItem('sessionGapThreshold')
            const gapThreshold = savedThreshold ? parseInt(savedThreshold, 10) : 1800 // English engineering note.
            await window.sessionApi.generate(importResult.sessionId, gapThreshold)
          } catch (error) {
            console.error('自动生成会话索引失败:', error)
            // English engineering note.
          }

          return { success: true, diagnostics: importResult.diagnostics }
        } else {
          // English engineering note.
          const diagnosisSuggestion = importResult.diagnosis?.suggestion
          return {
            success: false,
            error: importResult.error || 'error.import_failed',
            diagnosisSuggestion,
            diagnostics: importResult.diagnostics,
          }
        }
      } catch (error) {
        return { success: false, error: String(error) }
      } finally {
        isImporting.value = false
        setTimeout(() => {
          importProgress.value = null
        }, 500)
      }
    }

    /**
     * English note.
     */
    async function importFilesFromPaths(filePaths: string[]): Promise<BatchImportResult> {
      if (filePaths.length === 0) {
        return { total: 0, success: 0, failed: 0, cancelled: 0, files: [] }
      }

      // English engineering note.
      isBatchImporting.value = true
      batchImportCancelled.value = false
      batchImportResult.value = null

      // English engineering note.
      batchFiles.value = filePaths.map((path) => ({
        path,
        name: path.split('/').pop() || path.split('\\').pop() || path,
        status: 'pending' as BatchFileStatus,
      }))

      let successCount = 0
      let failedCount = 0
      let cancelledCount = 0

      // English engineering note.
      const markRemainingAsCancelled = (startIndex: number) => {
        for (let j = startIndex; j < batchFiles.value.length; j++) {
          if (batchFiles.value[j].status === 'pending') {
            batchFiles.value[j].status = 'cancelled'
            cancelledCount++
          }
        }
      }

      // English engineering note.
      for (let i = 0; i < batchFiles.value.length; i++) {
        // English engineering note.
        if (batchImportCancelled.value) {
          markRemainingAsCancelled(i)
          break
        }

        const file = batchFiles.value[i]
        file.status = 'importing'
        file.progress = {
          stage: 'detecting',
          progress: 0,
          message: '',
        }

        try {
          // English engineering note.
          const queue: ImportProgress[] = []
          let isProcessing = false
          let currentStage = 'reading'
          let lastStageTime = Date.now()
          const MIN_STAGE_TIME = 300 // English engineering note.

          const processQueue = async () => {
            if (isProcessing) return
            isProcessing = true

            while (queue.length > 0) {
              // English engineering note.
              if (batchImportCancelled.value) {
                queue.length = 0
                break
              }

              const next = queue[0]

              if (next.stage !== currentStage) {
                const elapsed = Date.now() - lastStageTime
                if (elapsed < MIN_STAGE_TIME) {
                  await new Promise((resolve) => setTimeout(resolve, MIN_STAGE_TIME - elapsed))
                }
                currentStage = next.stage
                lastStageTime = Date.now()
              }

              file.progress = queue.shift()!
            }
            isProcessing = false
          }

          const unsubscribe = window.chatApi.onImportProgress((progress) => {
            if (progress.stage === 'done') return
            queue.push(progress)
            processQueue()
          })

          const importResult = await window.chatApi.import(file.path)
          unsubscribe()

          // English engineering note.
          let waitCount = 0
          while ((queue.length > 0 || isProcessing) && !batchImportCancelled.value && waitCount < 100) {
            await new Promise((resolve) => setTimeout(resolve, 30))
            waitCount++
          }

          // English engineering note.
          if (batchImportCancelled.value) {
            // English engineering note.
            if (importResult.success && importResult.sessionId) {
              file.status = 'success'
              file.sessionId = importResult.sessionId
              successCount++

              // English engineering note.
              try {
                const savedThreshold = localStorage.getItem('sessionGapThreshold')
                const gapThreshold = savedThreshold ? parseInt(savedThreshold, 10) : 1800
                await window.sessionApi.generate(importResult.sessionId, gapThreshold)
              } catch (error) {
                console.error('自动生成会话索引失败:', error)
              }
            } else {
              file.status = 'failed'
              file.error = importResult.error || 'error.import_failed'
              failedCount++
            }
            // English engineering note.
            markRemainingAsCancelled(i + 1)
            break
          }

          if (importResult.success && importResult.sessionId) {
            file.status = 'success'
            file.sessionId = importResult.sessionId
            successCount++

            // English engineering note.
            if (!batchImportCancelled.value) {
              try {
                const savedThreshold = localStorage.getItem('sessionGapThreshold')
                const gapThreshold = savedThreshold ? parseInt(savedThreshold, 10) : 1800
                await window.sessionApi.generate(importResult.sessionId, gapThreshold)
              } catch (error) {
                console.error('自动生成会话索引失败:', error)
              }
            }
          } else {
            file.status = 'failed'
            file.error = importResult.error || 'error.import_failed'
            file.diagnosisSuggestion = importResult.diagnosis?.suggestion
            failedCount++
          }
        } catch (error) {
          file.status = 'failed'
          file.error = String(error)
          failedCount++
        }
      }

      // English engineering note.
      await loadSessions()

      // English engineering note.
      const result: BatchImportResult = {
        total: filePaths.length,
        success: successCount,
        failed: failedCount,
        cancelled: cancelledCount,
        files: [...batchFiles.value],
      }

      batchImportResult.value = result
      isBatchImporting.value = false

      return result
    }

    /**
     * English note.
     */
    function cancelBatchImport() {
      batchImportCancelled.value = true
    }

    /**
     * English note.
     */
    function clearBatchImportResult() {
      batchImportResult.value = null
      batchFiles.value = []
    }

    /**
     * English note.
     */
    async function mergeImportFiles(filePaths: string[]): Promise<MergeImportResult> {
      if (filePaths.length < 2) {
        return { success: false, error: '合并导入至少需要2个文件' }
      }

      // English engineering note.
      const MIN_STAGE_TIME = 800

      isMergeImporting.value = true
      mergeError.value = null
      mergeResult.value = null
      mergeStage.value = 'parsing'

      // English engineering note.
      mergeFiles.value = filePaths.map((path) => ({
        path,
        name: path.split('/').pop() || path.split('\\').pop() || path,
        status: 'pending' as MergeFileStatus,
      }))

      let stageStartTime = Date.now()

      try {
        // English engineering note.
        for (let i = 0; i < mergeFiles.value.length; i++) {
          const file = mergeFiles.value[i]
          const fileStartTime = Date.now()
          file.status = 'parsing'

          try {
            const info = await window.mergeApi.parseFileInfo(file.path)
            file.info = info

            // English engineering note.
            const elapsed = Date.now() - fileStartTime
            const minFileTime = Math.max(300, MIN_STAGE_TIME / filePaths.length)
            if (elapsed < minFileTime) {
              await new Promise((resolve) => setTimeout(resolve, minFileTime - elapsed))
            }

            file.status = 'done'
          } catch (err) {
            throw new Error(`解析文件失败: ${file.name} - ${err instanceof Error ? err.message : String(err)}`)
          }
        }

        // English engineering note.
        const parsingElapsed = Date.now() - stageStartTime
        if (parsingElapsed < MIN_STAGE_TIME) {
          await new Promise((resolve) => setTimeout(resolve, MIN_STAGE_TIME - parsingElapsed))
        }

        // English engineering note.
        stageStartTime = Date.now()
        mergeStage.value = 'merging'

        // English engineering note.
        const names = mergeFiles.value.map((f) => f.info?.name).filter(Boolean)
        const uniqueNames = [...new Set(names)]
        const outputName = uniqueNames.length === 1 ? uniqueNames[0]! : names[0] || '合并记录'

        const result = await window.mergeApi.mergeFiles({
          filePaths,
          outputName,
          conflictResolutions: [], // English engineering note.
          andAnalyze: true, // English engineering note.
        })

        if (!result.success) {
          throw new Error(result.error || '合并失败')
        }

        // English engineering note.
        await window.mergeApi.clearCache()

        // English engineering note.
        const mergingElapsed = Date.now() - stageStartTime
        if (mergingElapsed < MIN_STAGE_TIME) {
          await new Promise((resolve) => setTimeout(resolve, MIN_STAGE_TIME - mergingElapsed))
        }

        mergeStage.value = 'done'
        mergeResult.value = { success: true, sessionId: result.sessionId }

        // English engineering note.
        await loadSessions()

        // English engineering note.
        if (result.sessionId) {
          try {
            const savedThreshold = localStorage.getItem('sessionGapThreshold')
            const gapThreshold = savedThreshold ? parseInt(savedThreshold, 10) : 1800
            await window.sessionApi.generate(result.sessionId, gapThreshold)
          } catch (error) {
            console.error('自动生成会话索引失败:', error)
          }
        }

        return { success: true, sessionId: result.sessionId }
      } catch (err) {
        mergeStage.value = 'error'
        const errorMessage = err instanceof Error ? err.message : String(err)
        mergeError.value = errorMessage
        mergeResult.value = { success: false, error: errorMessage }
        // English engineering note.
        await window.mergeApi.clearCache()
        return { success: false, error: errorMessage }
      }
    }

    /**
     * English note.
     */
    function clearMergeImportResult() {
      isMergeImporting.value = false
      mergeFiles.value = []
      mergeResult.value = null
      mergeError.value = null
    }

    /**
     * English note.
     */
    function selectSession(id: string) {
      currentSessionId.value = id
    }

    /**
     * English note.
     */
    async function deleteSession(id: string): Promise<boolean> {
      try {
        const success = await window.chatApi.deleteSession(id)
        if (success) {
          const index = sessions.value.findIndex((s) => s.id === id)
          if (index !== -1) {
            sessions.value.splice(index, 1)
          }
          if (currentSessionId.value === id) {
            currentSessionId.value = null
          }
          await loadSessions()
        }
        return success
      } catch (error) {
        console.error('删除会话失败:', error)
        return false
      }
    }

    /**
     * English note.
     */
    async function renameSession(id: string, newName: string): Promise<boolean> {
      try {
        const success = await window.chatApi.renameSession(id, newName)
        if (success) {
          const session = sessions.value.find((s) => s.id === id)
          if (session) {
            session.name = newName
          }
        }
        return success
      } catch (error) {
        console.error('重命名会话失败:', error)
        return false
      }
    }

    /**
     * English note.
     */
    function clearSelection() {
      currentSessionId.value = null
    }

    /**
     * English note.
     */
    async function updateSessionOwnerId(id: string, ownerId: string | null): Promise<boolean> {
      try {
        const success = await window.chatApi.updateSessionOwnerId(id, ownerId)
        if (success) {
          const session = sessions.value.find((s) => s.id === id)
          if (session) {
            session.ownerId = ownerId
          }
        }
        return success
      } catch (error) {
        console.error('更新会话所有者失败:', error)
        return false
      }
    }

    // English engineering note.
    const pinnedSessionIds = ref<string[]>([])

    // English engineering note.
    const sortedSessions = computed(() => {
      // English engineering note.
      const pinIndexMap = new Map(pinnedSessionIds.value.map((id, index) => [id, index]))

      return [...sessions.value].sort((a, b) => {
        const aPinIndex = pinIndexMap.get(a.id)
        const bPinIndex = pinIndexMap.get(b.id)
        const aPinned = aPinIndex !== undefined
        const bPinned = bPinIndex !== undefined

        // English engineering note.
        if (aPinned && bPinned) {
          return bPinIndex! - aPinIndex!
        }
        // English engineering note.
        if (aPinned && !bPinned) return -1
        if (!aPinned && bPinned) return 1

        // English engineering note.
        return 0
      })
    })

    /**
     * English note.
     */
    function togglePinSession(id: string) {
      const index = pinnedSessionIds.value.indexOf(id)
      if (index !== -1) {
        pinnedSessionIds.value.splice(index, 1)
      } else {
        pinnedSessionIds.value.push(id)
      }
    }

    /**
     * English note.
     */
    function isPinned(id: string): boolean {
      return pinnedSessionIds.value.includes(id)
    }

    return {
      sessions,
      sortedSessions,
      pinnedSessionIds,
      currentSessionId,
      isImporting,
      importProgress,
      isInitialized,
      currentSession,
      // English engineering note.
      migrationNeeded,
      migrationCount,
      pendingMigrations,
      isMigrating,
      checkMigration,
      runMigration,
      // English engineering note.
      loadSessions,
      importFile,
      importFileFromPath,
      selectSession,
      deleteSession,
      renameSession,
      clearSelection,
      updateSessionOwnerId,
      togglePinSession,
      isPinned,
      // English engineering note.
      isBatchImporting,
      batchFiles,
      batchImportCancelled,
      batchImportResult,
      importFilesFromPaths,
      cancelBatchImport,
      clearBatchImportResult,
      // English engineering note.
      isMergeImporting,
      mergeFiles,
      mergeStage,
      mergeError,
      mergeResult,
      mergeImportFiles,
      clearMergeImportResult,
    }
  },
  {
    persist: [
      {
        pick: ['currentSessionId'],
        storage: sessionStorage,
      },
      {
        pick: ['pinnedSessionIds'],
        storage: localStorage,
      },
    ],
  }
)
