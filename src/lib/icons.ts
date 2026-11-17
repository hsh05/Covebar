import { invoke } from "@tauri-apps/api/core";

const cache = new Map<string, Promise<string | null>>();

/** Data URL for an app icon key, fetched once per key and size. */
export function iconUrl(key: string, cssPx: number): Promise<string | null> {
  const want = cssPx * (window.devicePixelRatio || 1);
  const size = [32, 48, 64, 96, 128, 256].find((s) => s >= want) ?? 256;
  const k = `${size}|${key}`;
  let p = cache.get(k);
  if (!p) {
    p = invoke<string | null>("get_icon", { key, size }).catch(() => null);
    cache.set(k, p);
  }
  return p;
}
