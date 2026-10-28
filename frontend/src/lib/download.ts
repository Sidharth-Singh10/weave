/**
 * Browser-only file saving. Kept apart from `export.ts` (which is pure and
 * tested under Node) so that generating a graph never depends on the DOM.
 */

export function downloadBlob(fileName: string, blob: Blob): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.rel = "noopener";
  // Firefox only follows the click when the anchor is in the document.
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  // Revoking synchronously can cancel the download in some browsers.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

export function downloadText(
  fileName: string,
  text: string,
  mimeType: string
): void {
  downloadBlob(fileName, new Blob([text], { type: `${mimeType};charset=utf-8` }));
}

export function downloadBytes(
  fileName: string,
  bytes: Uint8Array,
  mimeType: string
): void {
  // A Uint8Array is a valid BlobPart at runtime; the cast only satisfies
  // TypeScript's ArrayBufferLike generics.
  downloadBlob(fileName, new Blob([bytes as unknown as BlobPart], { type: mimeType }));
}
