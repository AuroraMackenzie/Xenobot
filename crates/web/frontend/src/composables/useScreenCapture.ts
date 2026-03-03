/**
 * English note.
 * English note.
 */
import { ref } from 'vue'
import { captureAsImageData } from '@/utils/snapCapture'
import { useToast } from '@nuxt/ui/runtime/composables/useToast.js'
import { useLayoutStore } from '@/stores/layout'

/** English note.
const DEFAULT_MOBILE_MAX_WIDTH = 525

export interface ScreenCaptureOptions {
  /** English note.
  hideSelectors?: string[]
  /** English note.
  maxExportWidth?: number
  /** English note.
  backgroundColor?: string
  /** English note.
  fullContent?: boolean
  /**
   * English note.
   * English note.
   * English note.
   * English note.
   */
  mobileWidth?: number | boolean
}

/**
 * English note.
 */
export function useScreenCapture() {
  const layoutStore = useLayoutStore()
  const toast = useToast()
  const isCapturing = ref(false)
  const captureError = ref<string | null>(null)

  // English engineering note.
  let lastCapturedImage: string | null = null

  /**
   * English note.
   */
  async function downloadImage(imageData: string) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
    const filename = `xenobot-screenshot-${timestamp}.png`

    try {
      const result = await window.cacheApi.saveToDownloads(filename, imageData)
      if (result.success) {
        toast.add({
          title: '截图已保存',
          description: `已保存到下载目录：${filename}`,
          icon: 'i-heroicons-check-circle',
          color: 'primary',
          duration: 3000,
          actions: [
            {
              label: '打开目录',
              onClick: () => {
                window.cacheApi.openDir('downloads')
              },
            },
          ],
        })
      } else {
        throw new Error(result.error)
      }
    } catch (error) {
      console.error('保存图片失败:', error)
      toast.add({
        title: '保存失败',
        description: String(error),
        icon: 'i-heroicons-x-circle',
        color: 'error',
        duration: 3000,
      })
    }
  }

  /**
   * English note.
   */
  function showSuccessToast(imageData: string) {
    lastCapturedImage = imageData

    toast.add({
      title: '截图已复制到剪贴板',
      icon: 'i-heroicons-check-circle',
      color: 'primary',
      duration: 3000,
      actions: [
        {
          label: '预览截图',
          icon: 'i-heroicons-eye',
          onClick: () => {
            if (lastCapturedImage) {
              layoutStore.openScreenCaptureModal(lastCapturedImage)
            }
          },
        },
        {
          label: '保存',
          icon: 'i-heroicons-arrow-down-tray',
          onClick: () => {
            if (lastCapturedImage) {
              downloadImage(lastCapturedImage)
            }
          },
        },
      ],
    })
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  async function capture(selector: string, options?: ScreenCaptureOptions): Promise<boolean> {
    const element = document.querySelector(selector) as HTMLElement | null
    if (!element) {
      captureError.value = `未找到元素: ${selector}`
      return false
    }
    return captureElement(element, options)
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  async function captureElement(element: HTMLElement, options?: ScreenCaptureOptions): Promise<boolean> {
    if (isCapturing.value) return false

    isCapturing.value = true
    captureError.value = null

    // English engineering note.
    const originalPadding = element.style.padding
    const originalPaddingBottom = element.style.paddingBottom
    const originalPosition = element.style.position
    const originalWidth = element.style.width
    const originalMinWidth = element.style.minWidth
    const originalMaxWidth = element.style.maxWidth

    element.style.padding = '16px'
    element.style.paddingBottom = '48px' // English engineering note.
    const computedPosition = window.getComputedStyle(element).position
    if (computedPosition === 'static') {
      element.style.position = 'relative'
    }

    // English engineering note.
    let appliedMobileWidth = false
    if (options?.mobileWidth) {
      const baseWidth = typeof options.mobileWidth === 'number' ? options.mobileWidth : DEFAULT_MOBILE_MAX_WIDTH

      // English engineering note.
      const currentWidth = element.getBoundingClientRect().width

      // English engineering note.
      if (currentWidth > baseWidth) {
        // English engineering note.
        // English engineering note.
        const scaleFactor = 0.3
        const targetWidth = Math.round(baseWidth + (currentWidth - baseWidth) * scaleFactor)

        element.style.width = `${targetWidth}px`
        element.style.minWidth = `${targetWidth}px`
        element.style.maxWidth = `${targetWidth}px`
        appliedMobileWidth = true
      }
    }

    // English engineering note.
    const watermark = document.createElement('div')
    watermark.className = '__capture-watermark__'
    watermark.style.cssText = `
      position: absolute;
      left: 0;
      right: 0;
      bottom: 16px;
      text-align: center;
      font-size: 14px;
      color: #9ca3af;
    `
    watermark.textContent = 'Xenobot · Local-First Chat Intelligence'
    element.appendChild(watermark)

    // English engineering note.
    const hiddenElements: HTMLElement[] = []
    const HIDDEN_CLASS = '__capture-hidden__'

    // English engineering note.
    let styleTag = document.getElementById('__capture-style__')
    if (!styleTag) {
      styleTag = document.createElement('style')
      styleTag.id = '__capture-style__'
      styleTag.textContent = `.${HIDDEN_CLASS} { display: none !important; }`
      document.head.appendChild(styleTag)
    }

    // English engineering note.
    const noCaptureElements = element.querySelectorAll('.no-capture')
    noCaptureElements.forEach((el) => {
      const htmlEl = el as HTMLElement
      hiddenElements.push(htmlEl)
      htmlEl.classList.add(HIDDEN_CLASS)
    })

    // English engineering note.
    if (options?.hideSelectors) {
      for (const selector of options.hideSelectors) {
        const elements = document.querySelectorAll(selector)
        elements.forEach((el) => {
          const htmlEl = el as HTMLElement
          hiddenElements.push(htmlEl)
          htmlEl.classList.add(HIDDEN_CLASS)
        })
      }
    }

    // English engineering note.
    const fullContent = options?.fullContent !== false
    const overflowElements: {
      el: HTMLElement
      originalOverflow: string
      originalHeight: string
      originalMaxHeight: string
    }[] = []

    if (fullContent) {
      // English engineering note.
      const elementsWithOverflow = [element, ...Array.from(element.querySelectorAll('*'))] as HTMLElement[]
      for (const node of elementsWithOverflow) {
        const style = window.getComputedStyle(node)
        const overflow = style.overflow
        const overflowY = style.overflowY
        const maxHeight = style.maxHeight

        if (
          overflow === 'hidden' ||
          overflow === 'auto' ||
          overflow === 'scroll' ||
          overflowY === 'hidden' ||
          overflowY === 'auto' ||
          overflowY === 'scroll' ||
          (maxHeight !== 'none' && maxHeight !== '0px')
        ) {
          overflowElements.push({
            el: node,
            originalOverflow: node.style.overflow,
            originalHeight: node.style.height,
            originalMaxHeight: node.style.maxHeight,
          })
          node.style.overflow = 'visible'
          node.style.maxHeight = 'none'
          // English engineering note.
          if (style.height !== 'auto' && node.scrollHeight > node.clientHeight) {
            node.style.height = 'auto'
          }
        }
      }

      // English engineering note.
      let parent: HTMLElement | null = element.parentElement
      while (parent) {
        const style = window.getComputedStyle(parent)
        const overflow = style.overflow
        const overflowY = style.overflowY
        if (
          overflow === 'hidden' ||
          overflow === 'auto' ||
          overflow === 'scroll' ||
          overflowY === 'hidden' ||
          overflowY === 'auto' ||
          overflowY === 'scroll'
        ) {
          overflowElements.push({
            el: parent,
            originalOverflow: parent.style.overflow,
            originalHeight: parent.style.height,
            originalMaxHeight: parent.style.maxHeight,
          })
          parent.style.overflow = 'visible'
        }
        parent = parent.parentElement
      }
    }

    // English engineering note.
    const canvasElements: { el: HTMLCanvasElement; originalOutline: string; originalBorder: string }[] = []
    const canvases = element.querySelectorAll('canvas')
    canvases.forEach((canvas) => {
      canvasElements.push({
        el: canvas,
        originalOutline: canvas.style.outline,
        originalBorder: canvas.style.border,
      })
      canvas.style.outline = 'none'
      canvas.style.border = 'none'
    })

    // English engineering note.
    // English engineering note.
    const headingElements: {
      el: HTMLElement
      originalStyles: {
        border: string
        outline: string
        boxShadow: string
      }
    }[] = []
    const headings = element.querySelectorAll('h1, h2, h3, h4, h5, h6')
    headings.forEach((heading) => {
      const htmlEl = heading as HTMLElement
      headingElements.push({
        el: htmlEl,
        originalStyles: {
          border: htmlEl.style.border,
          outline: htmlEl.style.outline,
          boxShadow: htmlEl.style.boxShadow,
        },
      })
      // English engineering note.
      htmlEl.style.border = 'none'
      htmlEl.style.outline = 'none'
      htmlEl.style.boxShadow = 'none'
    })

    // English engineering note.
    // English engineering note.
    // English engineering note.
    const listElements: {
      el: HTMLElement
      originalStyles: {
        listStyleType: string
        paddingLeft: string
        marginLeft: string
        border: string
        outline: string
        boxShadow: string
      }
      addedPrefixes: HTMLSpanElement[]
    }[] = []
    const lists = element.querySelectorAll('ol, ul')
    lists.forEach((list) => {
      const htmlEl = list as HTMLElement
      const isOrdered = htmlEl.tagName.toLowerCase() === 'ol'
      const addedPrefixes: HTMLSpanElement[] = []

      listElements.push({
        el: htmlEl,
        originalStyles: {
          listStyleType: htmlEl.style.listStyleType,
          paddingLeft: htmlEl.style.paddingLeft,
          marginLeft: htmlEl.style.marginLeft,
          border: htmlEl.style.border,
          outline: htmlEl.style.outline,
          boxShadow: htmlEl.style.boxShadow,
        },
        addedPrefixes,
      })

      // English engineering note.
      htmlEl.style.listStyleType = 'none'
      htmlEl.style.paddingLeft = '0'
      htmlEl.style.marginLeft = '0'
      // English engineering note.
      htmlEl.style.border = 'none'
      htmlEl.style.outline = 'none'
      htmlEl.style.boxShadow = 'none'

      // English engineering note.
      const lis = htmlEl.querySelectorAll(':scope > li')
      lis.forEach((li, index) => {
        const prefix = document.createElement('span')
        prefix.className = '__screen-capture-list-prefix__'
        prefix.style.cssText = 'display: inline-block; min-width: 1.5em; margin-right: 0.25em; text-align: right;'
        prefix.textContent = isOrdered ? `${index + 1}.` : '•'
        li.insertBefore(prefix, li.firstChild)
        addedPrefixes.push(prefix)
      })
    })

    // English engineering note.
    const textNodesBackup: { node: Text; originalText: string }[] = []
    const cleanProblematicCharacters = (text: string): string => {
      // English engineering note.
      // English engineering note.
      // English engineering note.
      return text.replace(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/g, '\uFFFD')
    }
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT, null)
    let textNode: Text | null
    while ((textNode = walker.nextNode() as Text | null)) {
      const originalText = textNode.textContent || ''
      const cleanedText = cleanProblematicCharacters(originalText)
      if (originalText !== cleanedText) {
        textNodesBackup.push({ node: textNode, originalText })
        textNode.textContent = cleanedText
      }
    }

    try {
      // English engineering note.
      if (appliedMobileWidth) {
        await new Promise<void>((resolve) => {
          requestAnimationFrame(() => {
            requestAnimationFrame(() => resolve())
          })
        })
      }

      const imageData = await captureAsImageData(element, {
        maxExportWidth: options?.maxExportWidth,
        backgroundColor: options?.backgroundColor,
        fullContent: options?.fullContent,
      })

      // English engineering note.
      const copyResult = await window.api.clipboard.copyImage(imageData)

      if (copyResult.success) {
        // English engineering note.
        showSuccessToast(imageData)
      } else {
        // English engineering note.
        toast.add({
          title: '截图完成',
          description: '复制到剪贴板失败，请手动保存',
          icon: 'i-heroicons-exclamation-triangle',
          color: 'warning',
          duration: 3000,
        })
        layoutStore.openScreenCaptureModal(imageData)
      }

      return true
    } catch (error) {
      captureError.value = String(error)
      console.error('截屏失败:', error)

      // English engineering note.
      let errorMessage = String(error)
      if (errorMessage.includes('URI malformed')) {
        errorMessage = '页面包含无法处理的特殊字符，请尝试截屏其他区域'
      }

      toast.add({
        title: '截屏失败',
        description: errorMessage,
        icon: 'i-heroicons-x-circle',
        color: 'error',
        duration: 3000,
      })
      return false
    } finally {
      // English engineering note.
      watermark.remove()

      // English engineering note.
      element.style.padding = originalPadding
      element.style.paddingBottom = originalPaddingBottom
      element.style.position = originalPosition
      element.style.width = originalWidth
      element.style.minWidth = originalMinWidth
      element.style.maxWidth = originalMaxWidth

      // English engineering note.
      for (const { node, originalText } of textNodesBackup) {
        node.textContent = originalText
      }
      // English engineering note.
      for (const { el, originalOutline, originalBorder } of canvasElements) {
        el.style.outline = originalOutline
        el.style.border = originalBorder
      }
      // English engineering note.
      for (const { el, originalStyles } of headingElements) {
        el.style.border = originalStyles.border
        el.style.outline = originalStyles.outline
        el.style.boxShadow = originalStyles.boxShadow
      }
      // English engineering note.
      for (const { el, originalStyles, addedPrefixes } of listElements) {
        el.style.listStyleType = originalStyles.listStyleType
        el.style.paddingLeft = originalStyles.paddingLeft
        el.style.marginLeft = originalStyles.marginLeft
        el.style.border = originalStyles.border
        el.style.outline = originalStyles.outline
        el.style.boxShadow = originalStyles.boxShadow
        // English engineering note.
        for (const prefix of addedPrefixes) {
          prefix.remove()
        }
      }
      // English engineering note.
      for (const { el, originalOverflow, originalHeight, originalMaxHeight } of overflowElements) {
        el.style.overflow = originalOverflow
        el.style.height = originalHeight
        el.style.maxHeight = originalMaxHeight
      }
      // English engineering note.
      for (const el of hiddenElements) {
        el.classList.remove('__capture-hidden__')
      }
      isCapturing.value = false
    }
  }

  /**
   * English note.
   * English note.
   * English note.
   */
  async function capturePage(options?: ScreenCaptureOptions): Promise<boolean> {
    // English engineering note.
    // English engineering note.
    const mainContent =
      document.querySelector('.main-content') ||
      document.querySelector('main .overflow-y-auto') ||
      document.querySelector('main')

    if (!mainContent) {
      captureError.value = '未找到可截屏的页面区域'
      return false
    }

    return captureElement(mainContent as HTMLElement, options)
  }

  return {
    /** English note.
    isCapturing,
    /** English note.
    captureError,
    /** English note.
    capture,
    /** English note.
    captureElement,
    /** English note.
    capturePage,
  }
}
