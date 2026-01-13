/**
 * Quick Commands Extension for CodeMirror
 * Provides slash commands like /now, /date, /time, /uuid, /hr
 * with both autocomplete dropdown and inline expansion
 */

import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';

/**
 * Quick command definition
 */
export interface QuickCommand {
  trigger: string;      // e.g., "/now"
  label: string;        // Display name in autocomplete
  description: string;  // Description shown in dropdown
  generate: () => string;
}

/**
 * Format date/time consistently using user's locale
 */
function formatDateTime(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  const seconds = String(date.getSeconds()).padStart(2, '0');
  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
}

function formatDate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function formatTime(date: Date): string {
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  const seconds = String(date.getSeconds()).padStart(2, '0');
  return `${hours}:${minutes}:${seconds}`;
}

/**
 * Generate a UUID v4
 */
function generateUUID(): string {
  // Use crypto.randomUUID if available (modern browsers)
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
 * Available quick commands
 */
export const quickCommands: QuickCommand[] = [
  {
    trigger: '/now',
    label: '/now',
    description: 'Insert current date and time',
    generate: () => formatDateTime(new Date()),
  },
  {
    trigger: '/date',
    label: '/date',
    description: 'Insert current date',
    generate: () => formatDate(new Date()),
  },
  {
    trigger: '/time',
    label: '/time',
    description: 'Insert current time',
    generate: () => formatTime(new Date()),
  },
  {
    trigger: '/uuid',
    label: '/uuid',
    description: 'Insert random UUID',
    generate: () => generateUUID(),
  },
  {
    trigger: '/hr',
    label: '/hr',
    description: 'Insert horizontal rule',
    generate: () => '---',
  },
];

/**
 * Autocomplete source for quick commands
 * Shows dropdown when user types / followed by letters
 */
export function quickCommandCompletion(context: CompletionContext): CompletionResult | null {
  // Match /word pattern - must start with /
  const word = context.matchBefore(/\/\w*/);
  if (!word) return null;

  // Filter commands that match the typed text
  const matchingCommands = quickCommands.filter(cmd =>
    cmd.trigger.toLowerCase().startsWith(word.text.toLowerCase())
  );

  if (matchingCommands.length === 0) return null;

  return {
    from: word.from,
    options: matchingCommands.map(cmd => ({
      label: cmd.label,
      detail: cmd.description,
      apply: (view: EditorView, completion: { label: string }, from: number, to: number) => {
        // Replace the slash command with generated text
        const generated = cmd.generate();
        view.dispatch({
          changes: { from, to, insert: generated },
          selection: { anchor: from + generated.length },
        });
      },
    })),
    validFor: /^\/\w*$/,
  };
}

/**
 * Input handler for inline expansion
 * Expands commands when user presses space or enter after a complete command
 */
export function quickCommandInputHandler(): Extension {
  return EditorView.inputHandler.of((view, from, to, text) => {
    // Only trigger on space or newline
    if (text !== ' ' && text !== '\n') return false;

    // Get text before cursor position
    const line = view.state.doc.lineAt(from);
    const lineText = line.text.slice(0, from - line.from);

    // Look for a slash command at the end of the line
    const match = lineText.match(/\/(\w+)$/);
    if (!match) return false;

    const commandText = '/' + match[1];
    const command = quickCommands.find(
      cmd => cmd.trigger.toLowerCase() === commandText.toLowerCase()
    );

    if (!command) return false;

    // Replace the command with generated text (plus the typed space/newline)
    const generated = command.generate();
    const commandStart = from - commandText.length;

    view.dispatch({
      changes: { from: commandStart, to: from, insert: generated + text },
      selection: { anchor: commandStart + generated.length + text.length },
    });

    return true; // We handled this input
  });
}

/**
 * Create the quick commands extension
 * Returns just the input handler for inline expansion.
 * The autocomplete source (quickCommandCompletion) should be passed
 * to autocompletion() override in the editor configuration.
 */
export function quickCommandsExtension(): Extension {
  return quickCommandInputHandler();
}
