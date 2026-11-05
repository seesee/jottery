/**
 * Copies text, and says whether it worked. Browsers have no clipboard API on plain
 * HTTP, which is how a default install is served, so callers need another way out.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (!navigator.clipboard) {
      return false;
    }
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
