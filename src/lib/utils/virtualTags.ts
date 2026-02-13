/**
 * Virtual Tag System
 *
 * Virtual tags are special tags with prefixes that have semantic meaning.
 * They are stored with their prefix but displayed differently in the UI.
 *
 * Current virtual tags:
 * - t: (title) - Custom note title
 *
 * Future virtual tags could include:
 * - d: (due date)
 * - p: (priority)
 * - etc.
 */

export interface VirtualTagConfig {
  /** The prefix identifying this virtual tag type (e.g., 't:') */
  prefix: string;
  /** The i18n key for the display name (e.g., 'tags.virtual.title') */
  displayKey: string;
  /** Whether only one tag of this type is allowed per note */
  singular: boolean;
  /** Visual styling for the tag */
  style: 'italic' | 'bold';
}

/**
 * Configuration for all virtual tag types.
 * Keys are the virtual tag type names (used internally).
 */
export const VIRTUAL_TAGS: Record<string, VirtualTagConfig> = {
  title: {
    prefix: 't:',
    displayKey: 'tags.virtual.title',
    singular: true,
    style: 'italic',
  },
  // Future virtual tags can be added here
};

/** The prefix for title tags */
export const TITLE_TAG_PREFIX = 't:';

/**
 * Check if a tag is a virtual tag (has a known prefix).
 * @param tag The tag to check
 * @returns true if the tag is a virtual tag
 */
export function isVirtualTag(tag: string): boolean {
  return Object.values(VIRTUAL_TAGS).some((config) => tag.startsWith(config.prefix));
}

/**
 * Get the virtual tag config for a tag, if any.
 * @param tag The tag to check
 * @returns The config if tag is virtual, undefined otherwise
 */
export function getVirtualTagConfig(tag: string): VirtualTagConfig | undefined {
  return Object.values(VIRTUAL_TAGS).find((config) => tag.startsWith(config.prefix));
}

/**
 * Get the display information for a virtual tag.
 * @param tag The virtual tag
 * @returns Display info (displayKey, value, style) or null if not a virtual tag
 */
export function getVirtualTagDisplay(
  tag: string
): { displayKey: string; value: string; style: string } | null {
  const config = getVirtualTagConfig(tag);
  if (!config) return null;
  return {
    displayKey: config.displayKey,
    value: tag.substring(config.prefix.length),
    style: config.style,
  };
}

/**
 * Filter tags to get only regular (non-virtual) tags.
 * @param tags Array of tags
 * @returns Array of tags that are not virtual tags
 */
export function getRegularTags(tags: string[]): string[] {
  return tags.filter((tag) => !isVirtualTag(tag));
}

/**
 * Filter tags to get only virtual tags.
 * @param tags Array of tags
 * @returns Array of virtual tags
 */
export function getVirtualTags(tags: string[]): string[] {
  return tags.filter((tag) => isVirtualTag(tag));
}

/**
 * Get the virtual tag type name for a tag.
 * @param tag The tag to check
 * @returns The type name (e.g., 'title') or undefined if not virtual
 */
export function getVirtualTagType(tag: string): string | undefined {
  for (const [type, config] of Object.entries(VIRTUAL_TAGS)) {
    if (tag.startsWith(config.prefix)) {
      return type;
    }
  }
  return undefined;
}
