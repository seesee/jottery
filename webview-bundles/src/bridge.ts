/**
 * Shared bridge protocol for Swift <-> JavaScript communication
 * via WKWebView's message handler system.
 */

// JS -> Swift messages (via window.webkit.messageHandlers.bridge.postMessage)
export type JsToSwift =
  | { type: 'contentChanged'; content: string }
  | { type: 'ready' }
  | { type: 'requestAttachment'; id: string }
  | { type: 'openLink'; url: string };

declare global {
  interface Window {
    webkit?: {
      messageHandlers: {
        bridge: {
          postMessage(message: JsToSwift): void;
        };
      };
    };
    bridge: {
      setContent(content: string, isDark: boolean): void;
      setTheme(isDark: boolean): void;
      resolveAttachment(id: string, dataUrl: string): void;
    };
  }
}

/** Post a message to Swift. No-op if not running in WKWebView. */
export function postToSwift(message: JsToSwift): void {
  window.webkit?.messageHandlers?.bridge?.postMessage(message);
}
