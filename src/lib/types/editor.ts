/**
 * Editor-related type definitions
 * Provides proper typing for editor components and third-party libraries
 */

import type { Editor } from '@tiptap/core';

/**
 * CodeEditor component reference
 * Exposes methods for programmatic control of the CodeMirror editor
 */
export interface CodeEditorRef {
  /** Focus the editor */
  focus(): void;
  /** Undo the last change */
  undo(): void;
  /** Redo the last undone change */
  redo(): void;
}

/**
 * WysiwygEditor component reference
 * Exposes methods for programmatic control of the Tiptap WYSIWYG editor
 */
export interface WysiwygEditorRef {
  /** Get the underlying Tiptap editor instance */
  getEditor(): Editor | null;
  /** Focus the editor */
  focus(): void;
  /** Undo the last change */
  undo(): void;
  /** Redo the last undone change */
  redo(): void;
  /** Toggle bold formatting */
  toggleBold(): void;
  /** Toggle italic formatting */
  toggleItalic(): void;
  /** Toggle strikethrough formatting */
  toggleStrike(): void;
  /** Toggle inline code formatting */
  toggleCode(): void;
  /** Toggle heading at specified level */
  toggleHeading(level: 1 | 2 | 3 | 4 | 5 | 6): void;
  /** Toggle bullet list */
  toggleBulletList(): void;
  /** Toggle ordered list */
  toggleOrderedList(): void;
  /** Toggle task list */
  toggleTaskList(): void;
  /** Toggle blockquote */
  toggleBlockquote(): void;
  /** Toggle code block */
  toggleCodeBlock(): void;
  /** Set or unset a link */
  setLink(url: string): void;
  /** Insert an image */
  insertImage(src: string, alt?: string): void;
  /** Insert a table */
  insertTable(rows?: number, cols?: number): void;
  /** Insert a horizontal rule */
  insertHorizontalRule(): void;
  /** Check if a format is active */
  isActive(name: string, attributes?: Record<string, unknown>): boolean;
}

/**
 * highlight.js instance interface
 * Minimal interface for code highlighting functionality
 */
export interface HighlightJsInstance {
  /** Highlight a code block with automatic language detection */
  highlightAuto(code: string): { value: string; language?: string };
  /** Highlight a code block with a specific language */
  highlight(code: string, options: { language: string; ignoreIllegals?: boolean }): { value: string };
  /** Get list of available languages */
  listLanguages(): string[];
  /** Register a language */
  registerLanguage(name: string, language: unknown): void;
  /** Get a registered language definition */
  getLanguage(name: string): object | undefined;
}

/**
 * PDF.js library interface
 * Minimal interface for PDF rendering functionality
 * Note: Uses loose typing to avoid conflicts with pdfjs-dist internal types
 */
export interface PdfJsLib {
  /** Load a PDF document from URL or data */
  getDocument(source: string | ArrayBuffer | Uint8Array | object): {
    promise: Promise<PdfDocument>;
  };
  /** Global worker source path */
  GlobalWorkerOptions: {
    workerSrc: string;
  };
}

/**
 * PDF.js document interface
 */
export interface PdfDocument {
  /** Number of pages in the document */
  numPages: number;
  /** Get a specific page (1-indexed) */
  getPage(pageNumber: number): Promise<PdfPage>;
  /** Clean up resources */
  destroy(): Promise<void>;
}

/**
 * PDF.js page interface
 */
export interface PdfPage {
  /** Get the viewport for rendering */
  getViewport(options: { scale: number }): PdfViewport;
  /** Render the page to a canvas context */
  render(params: { canvasContext: CanvasRenderingContext2D | null; viewport: PdfViewport }): {
    promise: Promise<void>;
  };
}

/**
 * PDF.js viewport interface
 */
export interface PdfViewport {
  width: number;
  height: number;
}

/**
 * FlexSearch enriched result item
 * When using enrich: true, results contain objects with id property
 */
export interface FlexSearchResultItem {
  id: string;
}

/**
 * FlexSearch enriched result set
 * Results are grouped by field when using enrich: true
 */
export interface FlexSearchEnrichedResult {
  field: string;
  result: (string | FlexSearchResultItem)[];
}

/**
 * Calculator evaluation result
 */
export interface CalcEvaluationResult {
  lineNumber: number;
  result: string;
  isError: boolean;
}

/**
 * Formatted calculation value types
 * Values that can be formatted by the calculator
 */
export type CalcValue = number | string | object;

/**
 * Landing page feature configuration
 */
export interface LandingFeature {
  id: string;
  icon: string;
  title: string;
  description: string;
  screenshot?: string;
  screenshotDark?: string;
}
