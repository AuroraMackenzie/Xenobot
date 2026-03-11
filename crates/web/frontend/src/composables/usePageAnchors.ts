import { ref, onMounted, onUnmounted } from "vue";

export interface AnchorItem {
  id: string;
  label: string;
}

/**
 * English note.
 * English note.
 * English note.
 */
export function usePageAnchors(
  anchors: AnchorItem[],
  options: {
    // English engineering note.
    threshold?: number;
    // English engineering note.
    containerSelector?: string;
    // English engineering note.
    scrollLockDuration?: number;
  } = {},
) {
  const {
    threshold = 300,
    containerSelector = ".overflow-y-auto",
    scrollLockDuration = 800,
  } = options;

  // English engineering note.
  const activeAnchor = ref(anchors[0]?.id || "");
  // English engineering note.
  let isScrolling = false;
  // English engineering note.
  const contentRef = ref<HTMLElement | null>(null);
  // English engineering note.
  let scrollContainer: Element | null = null;

  /**
   * English note.
   */
  function updateActiveAnchor() {
    if (isScrolling) return;

    // English engineering note.
    const positions: { id: string; top: number }[] = [];
    anchors.forEach((anchor) => {
      const element = document.getElementById(anchor.id);
      if (element) {
        const rect = element.getBoundingClientRect();
        positions.push({ id: anchor.id, top: Math.round(rect.top) });
      }
    });

    // English engineering note.
    let activeIndex = 0;
    for (let i = 0; i < positions.length; i++) {
      if (positions[i].top > threshold) {
        activeIndex = Math.max(0, i - 1);
        break;
      }
      activeIndex = i;
    }

    activeAnchor.value = anchors[activeIndex]?.id || "";
  }

  /**
   * English note.
   */
  function scrollToAnchor(id: string) {
    const element = document.getElementById(id);
    if (element) {
      // English engineering note.
      activeAnchor.value = id;
      // English engineering note.
      isScrolling = true;
      element.scrollIntoView({ behavior: "smooth", block: "start" });
      // English engineering note.
      setTimeout(() => {
        isScrolling = false;
        updateActiveAnchor();
      }, scrollLockDuration);
    }
  }

  onMounted(() => {
    // English engineering note.
    scrollContainer = contentRef.value?.closest(containerSelector) || null;
    if (scrollContainer) {
      scrollContainer.addEventListener("scroll", updateActiveAnchor, {
        passive: true,
      });
      updateActiveAnchor();
    }
  });

  onUnmounted(() => {
    if (scrollContainer) {
      scrollContainer.removeEventListener("scroll", updateActiveAnchor);
    }
  });

  return {
    // English engineering note.
    contentRef,
    // English engineering note.
    activeAnchor,
    // English engineering note.
    scrollToAnchor,
    // English engineering note.
    updateActiveAnchor,
  };
}
