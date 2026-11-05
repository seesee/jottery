/**
 * Lets anything about to leave the page ask the editor to save what it's holding.
 * The editor debounces saves by a second, so closing, reloading or locking inside
 * that second would otherwise lose the last edit.
 */
type Flush = () => Promise<void>;

let flush: Flush | null = null;

/** @returns A function that removes the registration */
export function registerPendingSave(save: Flush): () => void {
  flush = save;
  return () => {
    if (flush === save) {
      flush = null;
    }
  };
}

export async function flushPendingSave(): Promise<void> {
  await flush?.();
}
