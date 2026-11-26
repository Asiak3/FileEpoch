<script lang="ts">
  import { tick } from "svelte";
  import { sourceKey, type Source } from "../api";
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import { arrowNav, dismissable } from "../popover";
  import Icon from "./Icon.svelte";

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();

  const lastName = (s: Source) => s.path.split("/").filter(Boolean).at(-1) ?? "/";

  let groups = $derived.by(() => {
    const map = new Map<string, Source[]>();
    for (const s of app.sources) {
      const k = `${s.userName}@${s.host}`;
      map.set(k, [...(map.get(k) ?? []), s]);
    }
    return [...map.entries()];
  });

  $effect(() => {
    if (open && root) return dismissable(root, () => (open = false));
  });

  async function toggle() {
    open = !open;
    if (open) {
      await tick();
      (list?.querySelector('[aria-selected="true"]') as HTMLElement | null)?.focus();
    }
  }

  function choose(s: Source) {
    open = false;
    if (!app.source || sourceKey(s) !== sourceKey(app.source)) void app.selectSource(s);
  }
</script>

<div class="picker" bind:this={root}>
  <button class="current" onclick={toggle} aria-haspopup="listbox" aria-expanded={open} aria-label={t("app.source")} disabled={app.sources.length === 0}>
    <span class="icon"><Icon name="layers" size={17} /></span>
    {#if app.source}
      <span class="text">
        <strong>{lastName(app.source)}</strong>
        <small>{app.source.userName}@{app.source.host}:{app.source.path}</small>
      </span>
    {:else}
      <span class="text"><strong>{t("app.source")}</strong></span>
    {/if}
    <Icon name="chevron-down" size={15} />
  </button>

  {#if open}
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div class="pop" role="listbox" aria-label={t("app.source")} bind:this={list} onkeydown={(e) => list && arrowNav(e, list, "[role=option]")}>
      {#each groups as [label, items] (label)}
        <div class="group" role="presentation">{label}</div>
        {#each items as s (sourceKey(s))}
          {@const selected = !!app.source && sourceKey(s) === sourceKey(app.source)}
          <button class="option" role="option" aria-selected={selected} onclick={() => choose(s)}>
            <span class="text">
              <strong>{lastName(s)}</strong>
              <small>{s.path}</small>
            </span>
            {#if selected}<Icon name="check" size={16} />{/if}
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
  }

  .current {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 10px 0 6px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: transparent;
    text-align: left;
    color: var(--text-2);
    max-width: 440px;
  }

  .current:hover:not(:disabled) {
    background: var(--hover);
    border-color: var(--border);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 7px;
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }

  .text strong {
    color: var(--text);
    font-weight: 600;
    font-size: 14px;
  }

  .text small,
  .text strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .text small {
    color: var(--text-3);
    font-size: 12px;
  }

  .pop {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 20;
    min-width: 320px;
    max-width: 480px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    animation: pop 0.14s var(--ease);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  .group {
    padding: 8px 10px 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    color: var(--accent);
  }

  .option:hover,
  .option:focus-visible {
    background: var(--hover);
    outline: none;
  }

  .option[aria-selected="true"] {
    background: var(--accent-softer);
  }
</style>
