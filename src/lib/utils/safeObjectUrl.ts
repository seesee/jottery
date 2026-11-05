/**
 * Blob URLs for decrypted images.
 *
 * A blob URL has the app's origin. Inside <img> an SVG's scripts stay inert, but
 * "Open image in new tab" loads it as a document, where they run with access to the
 * vault. Attachment types are plaintext, so a server can relabel any blob as one.
 */

// Past this, a canvas may fail to allocate
const MAX_RASTER_SIZE = 4096;

/** Whether a blob of this type would run as a page if opened in a tab of its own */
export function isScriptableType(mimeType: string): boolean {
  const type = mimeType.split(';')[0].trim().toLowerCase();
  return type.includes('xml') || type.includes('html');
}

/** Draw an image to a PNG. It's only ever an <img> here, so nothing in it runs. */
async function rasterise(blob: Blob): Promise<Blob> {
  const source = URL.createObjectURL(blob);
  try {
    const image = new Image();
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error('This image could not be read'));
      image.src = source;
    });

    const scale = Math.min(1, MAX_RASTER_SIZE / Math.max(image.naturalWidth, image.naturalHeight, 1));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));

    const context = canvas.getContext('2d');
    if (!context) throw new Error('This image could not be drawn');
    context.drawImage(image, 0, 0, canvas.width, canvas.height);

    return await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob(png => (png ? resolve(png) : reject(new Error('This image could not be drawn'))), 'image/png');
    });
  } finally {
    URL.revokeObjectURL(source);
  }
}

/**
 * A URL for showing a decrypted image, safe to open in a tab. An image that can hold
 * script becomes a PNG, and anything else that would run as a page fails to load here.
 * The caller revokes it.
 */
export async function imageObjectUrl(blob: Blob): Promise<string> {
  return URL.createObjectURL(isScriptableType(blob.type) ? await rasterise(blob) : blob);
}
