// Frame decode worker.
// Receives JPEG ArrayBuffers from the main thread, decodes them via
// createImageBitmap, and draws them to an OffscreenCanvas.
// Also reports frame timestamps back so the main thread can compute fps.

let canvas: OffscreenCanvas | null = null;
let ctx: OffscreenCanvasRenderingContext2D | null = null;
let decoding = false;

self.onmessage = async (ev: MessageEvent) => {
  const { type, data } = ev.data as { type: string; data: unknown };

  if (type === "init") {
    canvas = data as OffscreenCanvas;
    ctx = canvas.getContext("2d");
    return;
  }

  if (type === "frame") {
    if (!ctx || !canvas || decoding) return;
    decoding = true;
    const jpeg = data as ArrayBuffer;
    try {
      const bitmap = await createImageBitmap(new Blob([jpeg], { type: "image/jpeg" }));
      ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
      bitmap.close();
      self.postMessage({ type: "frameDone", ts: performance.now() });
    } catch {
      // malformed jpeg — skip
    } finally {
      decoding = false;
    }
  }
};
