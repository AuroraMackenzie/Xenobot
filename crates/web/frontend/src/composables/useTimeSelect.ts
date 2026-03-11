/**
 * English note.
 *
 * English note.
 * English note.
 */
import { ref, computed, watch } from "vue";
import type { Ref } from "vue";
import type { RouteLocationNormalizedLoaded, Router } from "vue-router";
import type {
  TimeRangeValue,
  TimeSelectState,
  TimeSelectMode,
} from "@/components/common/TimeSelect.vue";

interface UseTimeSelectOptions {
  // English engineering note.
  activeTab: Ref<string>;
  // English engineering note.
  isInitialLoad: Ref<boolean>;
  // English engineering note.
  currentSessionId: Ref<string | null>;
  // English engineering note.
  onTimeRangeChange?: () => void;
}

export function useTimeSelect(
  route: RouteLocationNormalizedLoaded,
  router: Router,
  options: UseTimeSelectOptions,
) {
  const { activeTab, isInitialLoad, currentSessionId, onTimeRangeChange } =
    options;

  // English engineering note.

  // English engineering note.
  const timeRangeValue = ref<TimeRangeValue | null>(null);

  // English engineering note.
  const fullTimeRange = ref<{ start: number; end: number } | null>(null);

  // English engineering note.
  const availableYears = ref<number[]>([]);

  // English engineering note.

  // English engineering note.
  const timeFilter = computed(() => {
    const v = timeRangeValue.value;
    if (!v) return undefined;
    return { startTs: v.startTs, endTs: v.endTs };
  });

  // English engineering note.
  const timeFilterKey = computed(() => {
    const v = timeRangeValue.value;
    if (!v) return "init";
    return `${v.startTs}-${v.endTs}`;
  });

  // English engineering note.
  const selectedYearForOverview = computed(() => {
    const v = timeRangeValue.value;
    if (!v || v.isFullRange) return null;
    return new Date(v.startTs * 1000).getFullYear();
  });

  // English engineering note.
  const initialTimeState = computed<Partial<TimeSelectState>>(() => {
    const q = route.query;
    const m = q.timeMode as TimeSelectMode | undefined;
    return {
      mode: m ?? undefined,
      recentDays: q.timeDays ? Number(q.timeDays) : undefined,
      year: q.timeYear ? Number(q.timeYear) : undefined,
      quarterYear: q.timeYear ? Number(q.timeYear) : undefined,
      quarter: q.timeQuarter ? Number(q.timeQuarter) : undefined,
      customStart: (q.timeStart as string) || undefined,
      customEnd: (q.timeEnd as string) || undefined,
    };
  });

  // English engineering note.

  watch([activeTab, timeRangeValue], ([newTab, newTimeRange]) => {
    if (isInitialLoad.value || !newTimeRange) return;

    const state = (newTimeRange as TimeRangeValue).state;
    const query: Record<string, string | number | undefined> = {
      tab: newTab as string,
      timeMode: state.mode,
    };
    if (state.mode === "recent") query.timeDays = state.recentDays;
    if (state.mode === "year") query.timeYear = state.year;
    if (state.mode === "quarter") {
      query.timeYear = state.quarterYear;
      query.timeQuarter = state.quarter;
    }
    if (state.mode === "custom") {
      query.timeStart = state.customStart;
      query.timeEnd = state.customEnd;
    }

    router.replace({ query });
  });

  // English engineering note.

  watch(
    timeRangeValue,
    (val) => {
      if (!val || !currentSessionId.value) return;
      onTimeRangeChange?.();
    },
    { immediate: true },
  );

  // English engineering note.

  // English engineering note.
  function resetTimeRange() {
    timeRangeValue.value = null;
    fullTimeRange.value = null;
    availableYears.value = [];
  }

  return {
    // English engineering note.
    timeRangeValue,
    fullTimeRange,
    availableYears,
    // English engineering note.
    timeFilter,
    timeFilterKey,
    selectedYearForOverview,
    initialTimeState,
    // English engineering note.
    resetTimeRange,
  };
}
