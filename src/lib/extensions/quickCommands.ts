/**
 * Quick Commands Extension for CodeMirror
 * Provides customisable slash commands with autocomplete and inline expansion
 */

import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import type { QuickCommandConfig } from '../types/models';

/**
 * Format a date using a format string
 * Supported tokens: YYYY, MM, DD, HH, mm, ss
 */
function formatWithPattern(date: Date, format: string): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  const seconds = String(date.getSeconds()).padStart(2, '0');

  return format
    .replace(/YYYY/g, String(year))
    .replace(/MM/g, month)
    .replace(/DD/g, day)
    .replace(/HH/g, hours)
    .replace(/mm/g, minutes)
    .replace(/ss/g, seconds);
}

/**
 * Generate a UUID v4
 */
function generateUUID(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  // Fallback for older browsers
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

/**
 * Generate content for a quick command
 */
export function generateCommandContent(config: QuickCommandConfig): string {
  const now = new Date();

  switch (config.type) {
    case 'datetime':
      return formatWithPattern(now, config.format || 'YYYY-MM-DD HH:mm:ss');
    case 'date':
      return formatWithPattern(now, config.format || 'YYYY-MM-DD');
    case 'time':
      return formatWithPattern(now, config.format || 'HH:mm:ss');
    case 'uuid':
      return generateUUID();
    case 'text':
      return config.value || '';
    default:
      return '';
  }
}

/**
 * Create autocomplete source for quick commands
 * Shows dropdown when user types / followed by letters
 * Only triggers when / is at start of line or preceded by whitespace
 */
export function createQuickCommandCompletion(commands: QuickCommandConfig[]) {
  return function quickCommandCompletion(context: CompletionContext): CompletionResult | null {
    // Match /word pattern - must start with /
    const word = context.matchBefore(/\/\w*/);
    if (!word) return null;

    // Check that the slash is at start of line or preceded by whitespace
    if (word.from > 0) {
      const charBefore = context.state.doc.sliceString(word.from - 1, word.from);
      if (!/\s/.test(charBefore)) {
        return null; // Slash is preceded by non-whitespace (e.g., "date/time")
      }
    }

    // Filter commands that match the typed text
    const matchingCommands = commands.filter(cmd =>
      cmd.trigger.toLowerCase().startsWith(word.text.toLowerCase())
    );

    if (matchingCommands.length === 0) return null;

    return {
      from: word.from,
      options: matchingCommands.map(cmd => ({
        label: cmd.trigger,
        detail: cmd.description,
        apply: (view: EditorView, _completion: { label: string }, from: number, to: number) => {
          const generated = generateCommandContent(cmd);
          view.dispatch({
            changes: { from, to, insert: generated },
            selection: { anchor: from + generated.length },
          });
        },
      })),
      validFor: /^\/\w*$/,
    };
  };
}

/**
 * Create input handler for inline expansion
 * Expands commands when user presses space or enter after a complete command
 * Only triggers when / is at start of line or preceded by whitespace
 */
export function createQuickCommandInputHandler(commands: QuickCommandConfig[]): Extension {
  return EditorView.inputHandler.of((view, from, _to, text) => {
    // Only trigger on space or newline
    if (text !== ' ' && text !== '\n') return false;

    // Get text before cursor position
    const line = view.state.doc.lineAt(from);
    const lineText = line.text.slice(0, from - line.from);

    // Look for a slash command at the end of the line
    // Must be preceded by whitespace or at start of line (to avoid "date/time")
    const match = lineText.match(/(?:^|\s)(\/\w+)$/);
    if (!match) return false;

    const commandText = match[1]; // The captured group includes the slash
    const command = commands.find(
      cmd => cmd.trigger.toLowerCase() === commandText.toLowerCase()
    );

    if (!command) return false;

    // Replace the command with generated text (plus the typed space/newline)
    const generated = generateCommandContent(command);
    const commandStart = from - commandText.length;

    view.dispatch({
      changes: { from: commandStart, to: from, insert: generated + text },
      selection: { anchor: commandStart + generated.length + text.length },
    });

    return true;
  });
}
