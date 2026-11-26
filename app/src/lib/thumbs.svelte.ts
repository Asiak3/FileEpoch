// Image thumbnails for the icon views: only for items that stay on screen, a few at a time,
// with a bounded in-memory cache (nothing is stored on disk).

import { SvelteMap } from "svelte/reactivity";
import type { Entry } from "./api";
import { app } from "./store.svelte";
import { extension } from "./filetypes";

/** Formats the backend can decode (see src-tauri/src/thumbnails.rs). */
const DECODABLE = new Set(["jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "bmp", "tif", "tiff"]);
/** Same limit as the backend: bigger files are not downloaded for a thumbnail. */
const MAX_BYTES = 64 * 1024 * 1024;
const CONCURRENCY = 3;
const CACHE_ENTRIES = 600;

export function canThumbnail(e: Entry): boolean {
  return app.settings.thumbnails && e.kind === "f" && e.size > 0 && e.size <= MAX_BYTES && DECODABLE.has(extension(e.name).toLowerCase());
}

class Thumbs {
  /** oid → data URL, or null when it cannot be shown. Insertion order = age (LRU). */
  readonly cache = new SvelteMap<string, string | null>();
  private queue: string[] = [];
  private running = 0;
  /** Bumped on disconnect so answers for the old server are dropped. */
  private generation = 0;

  request(oid: string) {
    if (this.cache.has(oid)) {
      // Refresh its age.
      const v = this.cache.get(oid)!;
      this.cache.delete(oid);
      this.cache.set(oid, v);
      return;
    }
    if (!this.queue.includes(oid)) this.queue.push(oid);
    this.pump();
  }

  /** The item left the screen before its turn: do not download it. */
  cancel(oid: string) {
    const i = this.queue.indexOf(oid);
    if (i >= 0) this.queue.splice(i, 1);
  }

  clear() {
    this.generation++;
    this.queue = [];
    this.cache.clear();
  }

  private pump() {
    while (this.running < CONCURRENCY && this.queue.length) {
      // In request order (top to bottom); items that left the screen were already cancelled.
      const oid = this.queue.shift()!;
      const gen = this.generation;
      this.running++;
      app.api
        .thumbnail(oid)
        .catch(() => null)
        .then((url) => {
          if (gen !== this.generation) return;
          this.cache.set(oid, url);
          while (this.cache.size > CACHE_ENTRIES) this.cache.delete(this.cache.keys().next().value!);
        })
        .finally(() => {
          this.running--;
          this.pump();
        });
    }
  }
}

export const thumbs = new Thumbs();
