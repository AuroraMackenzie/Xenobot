export interface SanitizeSummaryOptions {
  allowedTags?: string[]
  allowedAttrs?: Record<string, string[]>
  allowedProtocols?: string[]
}

const DEFAULT_ALLOWED_TAGS = ['BR', 'A', 'IMG']
const DEFAULT_ALLOWED_ATTRS: Record<string, string[]> = {
  A: ['href', 'target', 'rel'],
  IMG: ['src', 'alt', 'title', 'width', 'height'],
  BR: [],
}
const DEFAULT_ALLOWED_PROTOCOLS = ['http:', 'https:']

function normalizeAllowedTags(allowedTags?: string[]) {
  return new Set(
    (allowedTags && allowedTags.length > 0 ? allowedTags : DEFAULT_ALLOWED_TAGS).map((tag) => tag.toUpperCase())
  )
}

function normalizeAllowedAttrs(allowedAttrs?: Record<string, string[]>) {
  const source = allowedAttrs && Object.keys(allowedAttrs).length > 0 ? allowedAttrs : DEFAULT_ALLOWED_ATTRS
  const normalized: Record<string, Set<string>> = {}
  for (const [tag, attrs] of Object.entries(source)) {
    normalized[tag.toUpperCase()] = new Set(attrs.map((attr) => attr.toLowerCase()))
  }
  return normalized
}

// English engineering note.
function isSafeUrl(url: string, allowedProtocols: string[]) {
  const trimmed = url.trim()
  if (!trimmed) return false
  const lower = trimmed.toLowerCase()
  if (lower.startsWith('javascript:') || lower.startsWith('vbscript:') || lower.startsWith('data:')) {
    return false
  }
  try {
    const parsed = new URL(trimmed, window.location.origin)
    return allowedProtocols.includes(parsed.protocol)
  } catch {
    return false
  }
}

// English engineering note.
export function sanitizeSummary(raw: string, options: SanitizeSummaryOptions = {}) {
  const allowedTags = normalizeAllowedTags(options.allowedTags)
  const allowedAttrs = normalizeAllowedAttrs(options.allowedAttrs)
  const allowedProtocols =
    options.allowedProtocols && options.allowedProtocols.length > 0
      ? options.allowedProtocols
      : DEFAULT_ALLOWED_PROTOCOLS

  const parser = new DOMParser()
  const doc = parser.parseFromString(raw || '', 'text/html')

  const sanitizeNode = (node: Element) => {
    const children = Array.from(node.children)
    for (const child of children) {
      const tag = child.tagName
      if (!allowedTags.has(tag)) {
        // English engineering note.
        const parent = child.parentNode
        if (parent) {
          while (child.firstChild) {
            parent.insertBefore(child.firstChild, child)
          }
          parent.removeChild(child)
        }
        continue
      }

      const allowedAttrsForTag = allowedAttrs[tag] || new Set<string>()
      for (const attr of Array.from(child.attributes)) {
        if (!allowedAttrsForTag.has(attr.name.toLowerCase())) {
          child.removeAttribute(attr.name)
        }
      }

      if (tag === 'A') {
        const href = child.getAttribute('href')?.trim() || ''
        if (!isSafeUrl(href, allowedProtocols)) {
          child.removeAttribute('href')
        }
        const target = child.getAttribute('target')
        if (target && target !== '_blank') {
          child.setAttribute('target', '_blank')
        }
        if (child.getAttribute('href')) {
          child.setAttribute('rel', 'noopener noreferrer')
        }
      }

      if (tag === 'IMG') {
        const src = child.getAttribute('src')?.trim() || ''
        if (!isSafeUrl(src, allowedProtocols)) {
          child.removeAttribute('src')
        }
      }

      sanitizeNode(child)
    }
  }

  sanitizeNode(doc.body)
  return doc.body.innerHTML
}
