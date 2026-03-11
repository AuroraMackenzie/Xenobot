/**
 * Footer action categories shown in the launchpad footer.
 */

export type FooterLinkType = "link";

/**
 * Launchpad footer link descriptor.
 */
export interface FooterLinkConfig {
  /** Stable identifier for rendering and analytics. */
  id: string;
  /** Icon name consumed by the frontend icon pipeline. */
  icon: string;
  /** Primary label shown in the launchpad footer. */
  title: string;
  /** Secondary descriptor shown below the title. */
  subtitle: string;
  /** Footer action kind. */
  type: FooterLinkType;
  /** Navigation target. */
  url: string;
}

/**
 * Default footer links shipped with the launchpad entry experience.
 */
export const defaultFooterLinks: FooterLinkConfig[] = [
  {
    id: "website",
    icon: "heroicons:globe-alt",
    title: "Website",
    subtitle: "Download the latest desktop build",
    type: "link",
    url: "https://xenobot.app/cn/",
  },
  {
    id: "github",
    icon: "brand:github",
    title: "Github",
    subtitle: "Open issues and report bugs",
    type: "link",
    url: "https://github.com/xenobot-labs/Xenobot",
  },
];
