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

/** The canonical prefix for title tags (used when creating new tags) */
export const TITLE_TAG_PREFIX = 't:';

/** All valid prefixes for title tags (for matching) */
export const TITLE_TAG_PREFIXES = ['t:', 'title:'];

/**
 * Check if a tag is a title tag (matches any title prefix).
 * @param tag The tag to check
 * @returns true if the tag is a title tag
 */
export function isTitleTag(tag: string): boolean {
  return TITLE_TAG_PREFIXES.some((prefix) => tag.startsWith(prefix));
}

/**
 * Get the title value from a title tag, stripping the prefix.
 * @param tag The title tag
 * @returns The title value, or undefined if not a title tag
 */
export function getTitleFromTag(tag: string): string | undefined {
  for (const prefix of TITLE_TAG_PREFIXES) {
    if (tag.startsWith(prefix)) {
      return tag.substring(prefix.length);
    }
  }
  return undefined;
}

/**
 * Check if a tag is a virtual tag (has a known prefix).
 * @param tag The tag to check
 * @returns true if the tag is a virtual tag
 */
export function isVirtualTag(tag: string): boolean {
  // Check title tags (which have multiple valid prefixes)
  if (isTitleTag(tag)) return true;
  // Check other virtual tags by their canonical prefix
  return Object.values(VIRTUAL_TAGS).some(
    (config) => config.prefix !== 't:' && tag.startsWith(config.prefix)
  );
}

/**
 * Get the virtual tag config for a tag, if any.
 * @param tag The tag to check
 * @returns The config if tag is virtual, undefined otherwise
 */
export function getVirtualTagConfig(tag: string): VirtualTagConfig | undefined {
  // Handle title tags with multiple prefixes
  if (isTitleTag(tag)) {
    return VIRTUAL_TAGS.title;
  }
  return Object.values(VIRTUAL_TAGS).find(
    (config) => config.prefix !== 't:' && tag.startsWith(config.prefix)
  );
}

/**
 * Get the display information for a virtual tag.
 * @param tag The virtual tag
 * @returns Display info (displayKey, value, style) or null if not a virtual tag
 */
export function getVirtualTagDisplay(
  tag: string
): { displayKey: string; value: string; style: string } | null {
  // Handle title tags with multiple prefixes
  if (isTitleTag(tag)) {
    const value = getTitleFromTag(tag);
    return {
      displayKey: VIRTUAL_TAGS.title.displayKey,
      value: value || '',
      style: VIRTUAL_TAGS.title.style,
    };
  }
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
  // Handle title tags with multiple prefixes
  if (isTitleTag(tag)) {
    return 'title';
  }
  for (const [type, config] of Object.entries(VIRTUAL_TAGS)) {
    if (config.prefix !== 't:' && tag.startsWith(config.prefix)) {
      return type;
    }
  }
  return undefined;
}
