/**
 * Frames holding a note, for as long as their print dialog is open. Kept apart from
 * noteExport so that the stores can clear them without loading every service.
 */
const printFrames = new Set<HTMLIFrameElement>();

export function trackPrintFrame(frame: HTMLIFrameElement): void {
  printFrames.add(frame);
}

export function removePrintFrame(frame: HTMLIFrameElement): void {
  frame.remove();
  printFrames.delete(frame);
}

/** Remove any note still in a print frame, which matters if the app locks meanwhile */
export function removePrintFrames(): void {
  printFrames.forEach(removePrintFrame);
}
