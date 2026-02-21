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
    Android?: {
      postMessage(json: string): void;
    };
    bridge: {
      setContent(content: string, isDark: boolean): void;
      setTheme(isDark: boolean): void;
      setFontSize(px: number): void;
      resolveAttachment(id: string, dataUrl: string): void;
    };
  }
}

/** Post a message to the native host. Supports iOS (WKWebView) and Android (@JavascriptInterface). */
export function postToSwift(message: JsToSwift): void {
  if (window.webkit?.messageHandlers?.bridge) {
    window.webkit.messageHandlers.bridge.postMessage(message);
    return;
  }
  if (window.Android?.postMessage) {
    window.Android.postMessage(JSON.stringify(message));
    return;
  }
}
