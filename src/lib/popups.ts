import { invoke } from "@tauri-apps/api/core";

/** Open (or toggle) a widget's card above/below the given tile element. */
export function openCard(widget: string, el: Element, width: number, height: number) {
  const r = el.getBoundingClientRect();
  invoke("open_card", { widget, anchorX: r.left, anchorWidth: r.width, width, height });
}

// Window preview layout, shared by the dock (to size the window) and the preview view.
export const PREVIEW = { thumbW: 208, thumbH: 124, header: 30, pad: 10, gap: 8, perRow: 5 };

export function previewSize(count: number) {
  const n = Math.max(1, count);
  const cols = Math.min(n, PREVIEW.perRow);
  const rows = Math.ceil(n / PREVIEW.perRow);
  const p = PREVIEW;
  return {
    width: p.pad * 2 + cols * p.thumbW + (cols - 1) * p.gap,
    height: p.pad * 2 + rows * (p.header + p.thumbH) + (rows - 1) * p.gap,
  };
}
