import { ref, watch, type Ref, type ComputedRef } from 'vue'

/**
 * English note.
 * English note.
 */

interface TimeFilter {
  startTs?: number
  endTs?: number
}

interface UseAsyncDataOptions<T> {
  /** English note.
  immediate?: boolean
  /** English note.
  deep?: boolean
  /** English note.
  onError?: (error: Error) => void
  /** English note.
  defaultValue?: T
}

interface UseAsyncDataReturn<T> {
  /** English note.
  data: Ref<T | null>
  /** English note.
  isLoading: Ref<boolean>
  /** English note.
  error: Ref<Error | null>
  /** English note.
  reload: () => Promise<void>
}

/**
 * English note.
 * English note.
 * English note.
 * English note.
 * English note.
 */
export function useAsyncData<T>(
  fetchFn: (sessionId: string, timeFilter?: TimeFilter) => Promise<T>,
  sessionId: Ref<string> | ComputedRef<string>,
  timeFilter?: Ref<TimeFilter | undefined> | ComputedRef<TimeFilter | undefined>,
  options: UseAsyncDataOptions<T> = {}
): UseAsyncDataReturn<T> {
  const { immediate = true, deep = true, onError, defaultValue } = options

  const data = ref<T | null>(defaultValue ?? null) as Ref<T | null>
  const isLoading = ref(false)
  const error = ref<Error | null>(null)

  async function load() {
    const sid = sessionId.value
    if (!sid) return

    isLoading.value = true
    error.value = null

    try {
      data.value = await fetchFn(sid, timeFilter?.value)
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e))
      error.value = err
      if (onError) {
        onError(err)
      } else {
        console.error('数据加载失败:', err)
      }
    } finally {
      isLoading.value = false
    }
  }

  // English engineering note.
  watch(
    () => [sessionId.value, timeFilter?.value],
    () => {
      if (sessionId.value) {
        load()
      }
    },
    { immediate, deep }
  )

  return {
    data,
    isLoading,
    error,
    reload: load,
  }
}

/**
 * English note.
 * English note.
 */
export function useMultipleAsyncData(loaders: Array<() => Promise<void>>) {
  const isLoading = ref(false)

  async function loadAll() {
    isLoading.value = true
    try {
      await Promise.all(loaders.map((loader) => loader()))
    } finally {
      isLoading.value = false
    }
  }

  return {
    isLoading,
    loadAll,
  }
}
