import { ref, watch, type Ref, type ComputedRef } from "vue";

interface TimeFilter {
  startTs?: number;
  endTs?: number;
}

interface UseAsyncDataOptions<T> {
  /** Start loading immediately after the watcher is bound. */
  immediate?: boolean;
  /** Re-run when nested fields inside the watched filter change. */
  deep?: boolean;
  /** Optional callback for centralized error handling. */
  onError?: (error: Error) => void;
  /** Initial fallback value before the first successful fetch. */
  defaultValue?: T;
}

interface UseAsyncDataReturn<T> {
  /** Latest resolved payload. */
  data: Ref<T | null>;
  /** Loading state for the current fetch cycle. */
  isLoading: Ref<boolean>;
  /** Last captured error, if any. */
  error: Ref<Error | null>;
  /** Force a manual reload with the current inputs. */
  reload: () => Promise<void>;
}

/** Shared async loader for session-scoped views that depend on an optional time filter. */
export function useAsyncData<T>(
  fetchFn: (sessionId: string, timeFilter?: TimeFilter) => Promise<T>,
  sessionId: Ref<string> | ComputedRef<string>,
  timeFilter?:
    | Ref<TimeFilter | undefined>
    | ComputedRef<TimeFilter | undefined>,
  options: UseAsyncDataOptions<T> = {},
): UseAsyncDataReturn<T> {
  const { immediate = true, deep = true, onError, defaultValue } = options;

  const data = ref<T | null>(defaultValue ?? null) as Ref<T | null>;
  const isLoading = ref(false);
  const error = ref<Error | null>(null);

  async function load() {
    const sid = sessionId.value;
    if (!sid) return;

    isLoading.value = true;
    error.value = null;

    try {
      data.value = await fetchFn(sid, timeFilter?.value);
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e));
      error.value = err;
      if (onError) {
        onError(err);
      } else {
        console.error("[useAsyncData] Data loading failed:", err);
      }
    } finally {
      isLoading.value = false;
    }
  }

  // English engineering note.
  watch(
    () => [sessionId.value, timeFilter?.value],
    () => {
      if (sessionId.value) {
        load();
      }
    },
    { immediate, deep },
  );

  return {
    data,
    isLoading,
    error,
    reload: load,
  };
}

/** Aggregate multiple async loaders under a single loading flag. */
export function useMultipleAsyncData(loaders: Array<() => Promise<void>>) {
  const isLoading = ref(false);

  async function loadAll() {
    isLoading.value = true;
    try {
      await Promise.all(loaders.map((loader) => loader()));
    } finally {
      isLoading.value = false;
    }
  }

  return {
    isLoading,
    loadAll,
  };
}
