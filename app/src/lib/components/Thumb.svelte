<script lang="ts">
  import type { Entry } from "../api";
  import { category } from "../filetypes";
  import { canThumbnail, thumbs } from "../thumbs.svelte";
  import FileIcon from "./FileIcon.svelte";

  let { entry, width, height, icon }: { entry: Entry; width: number; height: number; icon: number } = $props();

  /** Wait a moment before downloading, so scrolling past items does not fetch them. */
  const SETTLE_MS = 180;

  let wanted = $derived(canThumbnail(entry));
  let url = $derived(wanted ? thumbs.cache.get(entry.obj) : null);

  $effect(() => {
    if (!wanted) return;
    const oid = entry.obj;
    const timer = setTimeout(() => thumbs.request(oid), SETTLE_MS);
    return () => {
      clearTimeout(timer);
      thumbs.cancel(oid);
    };
  });
</script>

<span class="thumb" style:width="{width}px" style:height="{height}px">
  {#if url}
    <img src={url} alt="" draggable="false" />
  {:else}
    <FileIcon category={category(entry)} size={icon} />
  {/if}
</span>

<style>
  .thumb {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
  }

  img {
    display: block;
    max-width: 100%;
    max-height: 100%;
    min-width: 0;
    min-height: 0;
    object-fit: contain;
    border-radius: 4px;
    box-shadow: 0 0 0 1px var(--border), 0 1px 3px rgb(0 0 0 / 0.18);
    animation: appear 0.18s var(--ease);
  }

  @keyframes appear {
    from {
      opacity: 0;
    }
  }
</style>
