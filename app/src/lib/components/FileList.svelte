<script lang="ts">
  import type { Entry } from "../api";
  import { app, type SortKey } from "../store.svelte";
  import { i18n, t } from "../i18n.svelte";
  import { category, extension } from "../filetypes";
  import FileIcon from "./FileIcon.svelte";
  import Icon from "./Icon.svelte";

  const ROW = 34;
  const OVERSCAN = 8;

  let viewport: HTMLDivElement | undefined = $state();
  let height = $state(0);
  let scrollTop = $state(0);
  let anchor: string | null = null;
  let typeahead = "";
  let typeaheadTimer: ReturnType<typeof setTimeout> | undefined;

  let rows = $derived(app.visible);
  let start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  let end = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN));
  let slice = $derived(rows.slice(start, end));
  let focusIndex = $derived(app.focusName === null ? -1 : rows.findIndex((r) => r.name === app.focusName));
  let pageRows = $derived(Math.max(1, Math.floor(height / ROW) - 1));

  const columns: { key: SortKey; label: string; cls: string }[] = [
    { key: "name", label: "list.name", cls: "c-name" },
    { key: "mtime", label: "list.modified", cls: "c-date" },
    { key: "type", label: "list.type", cls: "c-type" },
    { key: "size", label: "list.size", cls: "c-size" },
  ];

  function typeLabel(e: Entry): string {
    if (e.kind === "d") return t("list.folder");
    if (e.kind === "s") return t("list.link");
    if (e.kind === "?") return t("list.unknown");
    const cat = category(e);
    if (cat !== "file") return t(`type.${cat}`);
    const ext = extension(e.name);
    return ext ? t("type.ext", { ext: ext.toUpperCase() }) : t("type.file");
  }

  function rowId(i: number): string {
    return `row-${i}`;
  }

  // Back to the top when the folder changes (not when only the time changes).
  let folderKey = $derived(app.source ? app.shownPath.join("/") : "");
  $effect(() => {
    void folderKey;
    if (viewport) viewport.scrollTop = 0;
    anchor = null;
  });

  function ensureVisible(i: number) {
    if (!viewport || i < 0) return;
    const top = i * ROW;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + ROW > viewport.scrollTop + height - ROW) viewport.scrollTop = top + 2 * ROW - height;
  }

  $effect(() => {
    ensureVisible(focusIndex);
  });

  function rangeNames(a: number, b: number): string[] {
    const [lo, hi] = a <= b ? [a, b] : [b, a];
    return rows.slice(lo, hi + 1).map((r) => r.name);
  }

  function clickRow(e: MouseEvent, entry: Entry, i: number) {
    viewport?.focus();
    if (e.shiftKey && anchor !== null) {
      const a = rows.findIndex((r) => r.name === anchor);
      const names = rangeNames(a < 0 ? i : a, i);
      app.selection = new Set(e.ctrlKey ? [...app.selection, ...names] : names);
    } else if (e.ctrlKey || e.metaKey) {
      const next = new Set(app.selection);
      if (next.has(entry.name)) next.delete(entry.name);
      else next.add(entry.name);
      app.selection = next;
      anchor = entry.name;
    } else {
      app.selection = new Set([entry.name]);
      anchor = entry.name;
    }
    app.focusName = entry.name;
  }

  function moveTo(i: number, e: KeyboardEvent) {
    if (rows.length === 0) return;
    const j = Math.max(0, Math.min(rows.length - 1, i));
    const name = rows[j].name;
    if (e.shiftKey) {
      if (anchor === null) anchor = rows[Math.max(0, focusIndex)]?.name ?? name;
      const a = rows.findIndex((r) => r.name === anchor);
      app.selection = new Set(rangeNames(a < 0 ? j : a, j));
    } else if (!e.ctrlKey) {
      app.selection = new Set([name]);
      anchor = name;
    }
    app.focusName = name;
  }

  function onkeydown(e: KeyboardEvent) {
    const cur = focusIndex;
    switch (e.key) {
      case "ArrowDown":
        if (e.altKey) return;
        moveTo(cur + 1, e);
        break;
      case "ArrowUp":
        if (e.altKey) {
          app.up();
          break;
        }
        moveTo(cur < 0 ? 0 : cur - 1, e);
        break;
      case "Home":
        moveTo(0, e);
        break;
      case "End":
        moveTo(rows.length - 1, e);
        break;
      case "PageDown":
        moveTo(cur + pageRows, e);
        break;
      case "PageUp":
        moveTo(cur - pageRows, e);
        break;
      case "Enter":
        if (cur >= 0) app.open(rows[cur]);
        break;
      case "Backspace":
        app.up();
        break;
      case "ArrowLeft":
        if (!e.altKey) return;
        app.goBack();
        break;
      case "ArrowRight":
        if (!e.altKey) return;
        app.goForward();
        break;
      case " ":
        if (cur >= 0) {
          const next = new Set(app.selection);
          const name = rows[cur].name;
          if (next.has(name)) next.delete(name);
          else next.add(name);
          app.selection = next;
          anchor = name;
        }
        break;
      case "Escape":
        app.selection = new Set();
        break;
      default:
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
          app.selection = new Set(rows.map((r) => r.name));
          break;
        }
        if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
          // Type-ahead: jump to the next item starting with the typed text.
          clearTimeout(typeaheadTimer);
          typeahead += e.key.toLowerCase();
          typeaheadTimer = setTimeout(() => (typeahead = ""), 800);
          const from = typeahead.length === 1 ? cur + 1 : Math.max(0, cur);
          for (let k = 0; k < rows.length; k++) {
            const r = rows[(from + k) % rows.length];
            if (r.name.toLowerCase().startsWith(typeahead)) {
              moveTo((from + k) % rows.length, new KeyboardEvent("keydown"));
              break;
            }
          }
          break;
        }
        return;
    }
    e.preventDefault();
  }

  let missingName = $derived(app.wantedPath[app.shownDepth] ?? "");
  let shownName = $derived(app.shownPath.at(-1) ?? app.source?.path.split("/").filter(Boolean).at(-1) ?? "/");
</script>

<div class="list" class:busy={app.listing}>
  {#if app.shownDepth < app.wantedPath.length && !app.listing}
    <div class="notice" role="status">
      <Icon name="info" size={16} />
      <span>{t("notice.missing", { name: missingName, shown: shownName })}</span>
    </div>
  {/if}

  <div class="header" role="presentation">
    {#each columns as c (c.key)}
      <button
        class="col {c.cls}"
        class:sorted={app.sortKey === c.key}
        onclick={() => app.sortBy(c.key)}
        aria-label="{t(c.label)}{app.sortKey === c.key ? (app.sortAsc ? ' ▲' : ' ▼') : ''}"
      >
        <span>{t(c.label)}</span>
        {#if app.sortKey === c.key}
          <Icon name={app.sortAsc ? "chevron-up" : "chevron-down"} size={13} stroke={1.8} />
        {/if}
      </button>
    {/each}
  </div>

  <div
    class="viewport"
    bind:this={viewport}
    bind:clientHeight={height}
    onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
    {onkeydown}
    tabindex="0"
    role="listbox"
    aria-multiselectable="true"
    aria-label={t("list.name")}
    aria-activedescendant={focusIndex >= 0 ? rowId(focusIndex) : undefined}
  >
    <div class="spacer" style:height="{rows.length * ROW}px">
      {#each slice as entry, k (entry.name)}
        {@const i = start + k}
        {@const selected = app.selection.has(entry.name)}
        <div
          id={rowId(i)}
          class="row"
          class:selected
          class:focused={i === focusIndex}
          style:transform="translateY({i * ROW}px)"
          role="option"
          aria-selected={selected}
          aria-setsize={rows.length}
          aria-posinset={i + 1}
          tabindex="-1"
          onclick={(e) => clickRow(e, entry, i)}
          ondblclick={() => app.open(entry)}
          onkeydown={() => {}}
        >
          <span class="c-name">
            <FileIcon category={category(entry)} />
            <span class="name" title={entry.name}>{entry.name}</span>
          </span>
          <span class="c-date">{entry.mtime ? i18n.dateTime(entry.mtime) : ""}</span>
          <span class="c-type">{typeLabel(entry)}</span>
          <span class="c-size" class:dir={entry.kind === "d"}>{entry.kind === "s" ? "" : i18n.size(entry.size)}</span>
        </div>
      {/each}
    </div>

    {#if rows.length === 0}
      <div class="state">
        {#if app.listing}
          <div class="spinner"></div>
          <span>{t("list.loading")}</span>
        {:else if app.listError}
          <Icon name="warning" size={22} />
          <span>{t(`err.${app.listError.code}`)}</span>
          <button class="btn" onclick={() => app.loadListing()}>{t("common.retry")}</button>
        {:else if app.search && app.entries.length > 0}
          <Icon name="search" size={22} />
          <span>{t("list.noMatch", { q: app.search })}</span>
        {:else if app.point}
          <span>{t("list.empty")}</span>
        {/if}
      </div>
    {/if}
  </div>
  <div class="progress" aria-hidden="true"></div>
</div>

<style>
  .list {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 12px 6px;
    padding: 7px 12px;
    border-radius: var(--radius-sm);
    background: var(--info-bg);
    color: var(--info-fg);
    font-size: 13px;
  }

  .header,
  .row {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) 190px 170px 104px;
    align-items: center;
    column-gap: 8px;
    padding: 0 16px;
  }

  .header {
    height: 34px;
    border-bottom: 1px solid var(--border);
    margin: 0 6px;
    padding: 0 10px;
  }

  .col {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 8px;
    margin: 0 -8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
    text-align: left;
  }

  .col:hover {
    background: var(--hover);
  }

  .col.sorted {
    color: var(--text);
  }

  .col.c-size {
    justify-content: flex-end;
  }

  .viewport {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    outline: none;
    padding: 4px 6px 8px;
  }

  .viewport:focus-visible {
    box-shadow: inset 0 0 0 2px var(--focus);
    border-radius: var(--radius-sm);
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: var(--row);
    padding: 0 10px;
    border-radius: var(--radius-sm);
    cursor: default;
    color: var(--text);
  }

  .row:hover {
    background: var(--hover);
  }

  .row.selected {
    background: var(--selected);
  }

  .row.selected:hover {
    background: var(--selected-hover);
  }

  .row.focused {
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }

  .row.selected.focused {
    box-shadow: inset 3px 0 0 0 var(--accent);
  }

  .c-name {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .c-date,
  .c-type,
  .c-size {
    color: var(--text-2);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }

  .c-size {
    text-align: right;
  }

  .c-size.dir {
    color: var(--text-3);
  }

  .state {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-3);
    text-align: center;
    padding: 24px;
  }

  .busy .spacer {
    opacity: 0.55;
    transition: opacity 0.2s 0.15s;
  }

  .progress {
    position: absolute;
    left: 16px;
    right: 16px;
    top: 34px;
    height: 2px;
    overflow: hidden;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .busy .progress {
    opacity: 1;
    transition-delay: 0.15s;
  }

  .progress::after {
    content: "";
    position: absolute;
    inset: 0;
    width: 30%;
    border-radius: 1px;
    background: var(--accent);
    animation: slide 1.1s var(--ease) infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }

  @media (max-width: 1000px) {
    .header,
    .row {
      grid-template-columns: minmax(200px, 1fr) 160px 0 96px;
    }
    .c-type {
      display: none;
    }
  }
</style>
