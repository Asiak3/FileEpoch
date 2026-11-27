<script lang="ts">
  import { onMount } from "svelte";
  import type { FileChange } from "../api";
  import { app } from "../store.svelte";
  import { i18n, t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  /** Rows shown before "show all". The list arrives sorted, biggest change first. */
  const TOP = 50;

  let st = $derived(app.diffState!);
  let all = $state(false);
  let dialog: HTMLDivElement | undefined = $state();
  let closeButton: HTMLButtonElement | undefined = $state();

  let diff = $derived(st.diff);
  let rows = $derived(diff ? (all ? diff.changes : diff.changes.slice(0, TOP)) : []);

  onMount(() => closeButton?.focus());

  // Escape closes; Tab stays inside the dialog.
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      app.closeChanges();
    } else if (e.key === "Tab" && dialog) {
      const focusable = [...dialog.querySelectorAll<HTMLElement>("button:not([disabled])")];
      const i = focusable.indexOf(document.activeElement as HTMLElement);
      e.preventDefault();
      focusable[(i + (e.shiftKey ? focusable.length - 1 : 1)) % focusable.length]?.focus();
    }
  }

  function parts(path: string): { dir: string; name: string } {
    const i = path.lastIndexOf("/");
    return i < 0 ? { dir: "", name: path } : { dir: path.slice(0, i + 1), name: path.slice(i + 1) };
  }

  function sizeText(c: FileChange): string {
    if (c.kind === "added") return `+${i18n.size(c.sizeAfter)}`;
    if (c.kind === "removed") return `−${i18n.size(c.sizeBefore)}`;
    return `${i18n.size(c.sizeBefore)} → ${i18n.size(c.sizeAfter)}`;
  }

  function netText(n: number): string {
    return `${n > 0 ? "+" : n < 0 ? "−" : ""}${i18n.size(Math.abs(n))}`;
  }

  const symbol = { added: "+", removed: "−", modified: "~" } as const;
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && app.closeChanges()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="diff-title" tabindex="-1" bind:this={dialog} {onkeydown}>
    <header>
      <div class="heading">
        <h2 id="diff-title">{t("diff.title")}</h2>
        <small>{t("diff.range", { time: i18n.dateTime(st.current.time), prev: i18n.dateTime(st.previous.time) })}</small>
      </div>
      <button class="icon-btn" bind:this={closeButton} onclick={() => app.closeChanges()} aria-label={t("common.close")}>
        <Icon name="x" size={16} />
      </button>
    </header>

    {#if st.loading}
      <div class="state" role="status">
        <div class="spinner" aria-hidden="true"></div>
        <span>{t("diff.loading")}</span>
      </div>
    {:else if st.error}
      <div class="state" role="alert">
        <Icon name="warning" size={22} />
        <span>{t(`err.${st.error.code}`)}</span>
        <button class="btn" onclick={() => app.showChanges()}>{t("common.retry")}</button>
      </div>
    {:else if diff}
      {#if diff.changes.length === 0}
        <div class="state">
          <Icon name="check" size={22} />
          <span>{t("diff.none")}</span>
        </div>
      {:else}
        <div class="totals">
          {#if diff.addedFiles}<span class="chip added">{t("diff.added", { n: i18n.number(diff.addedFiles) })}</span>{/if}
          {#if diff.modifiedFiles}<span class="chip modified">{t("diff.modified", { n: i18n.number(diff.modifiedFiles) })}</span>{/if}
          {#if diff.removedFiles}<span class="chip removed">{t("diff.removed", { n: i18n.number(diff.removedFiles) })}</span>{/if}
          <span class="net">{t("diff.net", { delta: netText(diff.netBytes) })}</span>
        </div>

        {#if diff.truncated}
          <p class="note" role="status"><Icon name="warning" size={15} />{t("diff.truncated")}</p>
        {/if}

        <ul class="list" aria-label={t("diff.title")}>
          {#each rows as c (c.kind + c.path)}
            {@const p = parts(c.path)}
            <li class="row {c.kind}">
              <span class="sym" title={t(`diff.k.${c.kind}`)} aria-label={t(`diff.k.${c.kind}`)}>{symbol[c.kind]}</span>
              <span class="path" title={c.path}>
                <span class="dir">{p.dir}</span><span class="name">{p.name}</span>
                {#if c.isDir}<small>{t("diff.folder", { n: c.files })}</small>{/if}
              </span>
              <span class="size">{sizeText(c)}</span>
            </li>
          {/each}
        </ul>

        {#if diff.changes.length > TOP && !all}
          <footer>
            <span class="muted">{t("diff.top", { n: TOP })}</span>
            <button class="btn" onclick={() => (all = true)}>{t("diff.showAll", { n: i18n.number(diff.changes.length) })}</button>
          </footer>
        {/if}
      {/if}
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 55;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(0, 0, 0, 0.35);
    animation: fade 0.12s;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .dialog {
    width: min(760px, 100%);
    max-height: min(640px, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    outline: none;
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 18px 16px 12px 22px;
    flex: none;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  small {
    color: var(--text-3);
    font-size: 12.5px;
  }

  .state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 40px 22px 48px;
    color: var(--text-2);
    text-align: center;
  }

  .spinner {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2.5px solid var(--surface-3);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .totals {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 0 22px 12px;
    flex: none;
  }

  .chip {
    padding: 2px 9px;
    border-radius: 10px;
    font-size: 12.5px;
    font-weight: 500;
    background: var(--surface-2);
  }

  .chip.added {
    color: var(--success);
  }

  .chip.removed {
    color: var(--danger);
  }

  .chip.modified {
    color: var(--accent);
  }

  .net {
    margin-left: auto;
    color: var(--text-2);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }

  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 22px 10px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    background: var(--warn-bg);
    color: var(--warn-fg);
    font-size: 12.5px;
    flex: none;
  }

  .list {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 0 8px 8px;
    overflow-y: auto;
    list-style: none;
    border-top: 1px solid var(--border);
  }

  .row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    font-size: 13px;
  }

  .row:hover {
    background: var(--hover);
  }

  .sym {
    flex: none;
    width: 14px;
    text-align: center;
    font-weight: 700;
    font-family: var(--mono);
  }

  .row.added .sym {
    color: var(--success);
  }

  .row.removed .sym {
    color: var(--danger);
  }

  .row.modified .sym {
    color: var(--accent);
  }

  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dir {
    color: var(--text-3);
  }

  .name {
    color: var(--text);
  }

  .path small {
    margin-left: 8px;
  }

  .size {
    flex: none;
    color: var(--text-2);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 22px;
    border-top: 1px solid var(--border);
    flex: none;
  }

  .muted {
    color: var(--text-3);
    font-size: 12.5px;
  }
</style>
