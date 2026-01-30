/**
 * Comprehensive markdown stripping utility
 * Removes all markdown signifiers for clean text display
 */

/**
 * Strip all markdown formatting from text
 * This is more comprehensive than the simple version used for titles
 */
export function stripMarkdownSignifiers(text: string): string {
  if (!text) return '';

  let result = text;

  // Remove markdown headers (# ## ### etc.)
  result = result.replace(/^#+\s+/gm, '');

  // Remove horizontal rules (---, ***, ___)
  result = result.replace(/^(?:[-*_]){3,}\s*$/gm, '');

  // Remove bold (**text** or __text__)
  result = result.replace(/(\*\*|__)(.*?)\1/g, '$2');

  // Remove italic (*text* or _text_)
  result = result.replace(/([*_])(.*?)\1/g, '$2');

  // Remove strikethrough (~~text~~)
  result = result.replace(/~~(.*?)~~/g, '$1');

  // Remove inline code (`text`)
  result = result.replace(/`([^`]+)`/g, '$1');

  // Remove code blocks (```text```)
  result = result.replace(/```[\s\S]*?```/g, '');
  result = result.replace(/`{3,}.*$/gm, '');

  // Remove links but keep text [text](url)
  result = result.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1');

  // Remove images ![alt](url)
  result = result.replace(/!\[([^\]]*)\]\([^)]+\)/g, '$1');

  // Remove reference-style links [text][ref]
  result = result.replace(/\[([^\]]+)\]\[[^\]]*\]/g, '$1');

  // Remove blockquotes (> text)
  result = result.replace(/^>\s+/gm, '');

  // Remove unordered list markers (-, *, +)
  result = result.replace(/^[\s]*[-*+]\s+/gm, '');

  // Remove ordered list markers (1. 2. etc.)
  result = result.replace(/^[\s]*\d+\.\s+/gm, '');

  // Remove task list markers (- [ ] or - [x])
  result = result.replace(/^[\s]*[-*+]\s+\[[ x]\]\s+/gm, '');

  // Remove HTML tags (loop to handle nested/fragmented tags like <scr<script>ipt>)
  let previousResult;
  do {
    previousResult = result;
    result = result.replace(/<[^>]+>/g, '');
  } while (result !== previousResult);

  // Remove any remaining angle brackets that could form incomplete tags
  // This catches attempts to bypass sanitization with unclosed tags like "<script"
  result = result.replace(/</g, '').replace(/>/g, '');

  // Remove extra whitespace
  result = result.replace(/\s+/g, ' ');

  return result.trim();
}

/**
 * Check if a language is markdown
 */
export function isMarkdownLanguage(language: string | undefined): boolean {
  return !language || language === 'markdown' || language === 'md';
}
