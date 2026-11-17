import { invoke } from "@tauri-apps/api/core";

const cache = new Map<string, Promise<string | null>>();

/** Data URL for an app icon key, fetched once per key and size. */
export function iconUrl(key: string, cssPx: number): Promise<string | null> {
  // Ask for the exact device-pixel size so the icon is drawn 1:1, never resampled in the page.
  const size = Math.min(256, Math.round(cssPx * (window.devicePixelRatio || 1)));
  const k = `${size}|${key}`;
  let p = cache.get(k);
  if (!p) {
    p = invoke<string | null>("get_icon", { key, size }).catch(() => null);
    cache.set(k, p);
    // Forget failures so the next render tries again.
    p.then((u) => u || cache.delete(k));
  }
  return p;
}
