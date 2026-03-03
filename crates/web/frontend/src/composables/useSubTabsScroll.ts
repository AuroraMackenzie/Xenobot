import { ref, onMounted, onUnmounted, type Ref, type ComputedRef } from 'vue'

/**
 * English note.
 */
export interface SubTabNavItem {
  id: string
  label: string
  icon?: string
}

/**
 * English note.
 * English note.
 */
export function useSubTabsScroll(navItems: ComputedRef<SubTabNavItem[]> | Ref<SubTabNavItem[]>) {
  // English engineering note.
  const activeNav = ref(navItems.value[0]?.id || '')

  // English engineering note.
  const isUserClick = ref(false)

  // English engineering note.
  const scrollContainerRef = ref<HTMLElement | null>(null)

  // English engineering note.
  const sectionRefs = ref<Record<string, HTMLElement | null>>({})

  /**
   * English note.
   */
  function setSectionRef(id: string, el: HTMLElement | null) {
    sectionRefs.value[id] = el
  }

  /**
   * English note.
   */
  function handleNavChange(id: string) {
    const section = sectionRefs.value[id]
    if (section && scrollContainerRef.value) {
      // English engineering note.
      isUserClick.value = true
      section.scrollIntoView({ behavior: 'smooth', block: 'start' })
      // English engineering note.
      setTimeout(() => {
        isUserClick.value = false
      }, 500)
    }
  }

  /**
   * English note.
   */
  function handleScroll() {
    // English engineering note.
    if (isUserClick.value || !scrollContainerRef.value) return

    const container = scrollContainerRef.value
    const containerRect = container.getBoundingClientRect()
    const offset = 50 // English engineering note.

    // English engineering note.
    const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 5
    if (isAtBottom) {
      // English engineering note.
      const lastItem = navItems.value[navItems.value.length - 1]
      if (lastItem) {
        activeNav.value = lastItem.id
      }
      return
    }

    // English engineering note.
    for (const item of navItems.value) {
      const section = sectionRefs.value[item.id]
      if (section) {
        const rect = section.getBoundingClientRect()
        // English engineering note.
        if (rect.top <= containerRect.top + offset && rect.bottom > containerRect.top + offset) {
          activeNav.value = item.id
          break
        }
      }
    }
  }

  // English engineering note.
  onMounted(() => {
    scrollContainerRef.value?.addEventListener('scroll', handleScroll)
  })

  onUnmounted(() => {
    scrollContainerRef.value?.removeEventListener('scroll', handleScroll)
  })

  /**
   * English note.
   */
  function scrollToId(id: string) {
    const section = sectionRefs.value[id]
    if (section && scrollContainerRef.value) {
      isUserClick.value = true
      section.scrollIntoView({ behavior: 'smooth', block: 'start' })
      activeNav.value = id
      setTimeout(() => {
        isUserClick.value = false
      }, 500)
    }
  }

  return {
    activeNav,
    scrollContainerRef,
    sectionRefs,
    setSectionRef,
    handleNavChange,
    scrollToId,
  }
}
