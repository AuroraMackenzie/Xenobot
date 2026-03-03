/**
 * English note.
 */

/**
 * English note.
 * English note.
 */
export type FooterLinkType = 'link'

/**
 * English note.
 */
export interface FooterLinkConfig {
  /** English note.
  id: string
  /** English note.
  icon: string
  /** English note.
  title: string
  /** English note.
  subtitle: string
  /** English note.
  type: FooterLinkType
  /** English note.
  url: string
}

/**
 * English note.
 */
export const defaultFooterLinks: FooterLinkConfig[] = [
  {
    id: 'website',
    icon: 'heroicons:globe-alt',
    title: '官网',
    subtitle: '下载最新版客户端',
    type: 'link',
    url: 'https://xenobot.app/cn/',
  },
  {
    id: 'github',
    icon: 'brand:github',
    title: 'Github',
    subtitle: '提交 Issue，反馈 BUG',
    type: 'link',
    url: 'https://github.com/xenobot-labs/Xenobot',
  },
]
