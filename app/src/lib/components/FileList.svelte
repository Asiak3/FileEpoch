<script lang="ts">
  import type { ColumnId, Entry, ViewMode } from "../api";
  import { app, DEFAULT_COLUMNS, MAX_COLUMN, MIN_COLUMN, type SortKey } from "../store.svelte";
  import { i18n, t } from "../i18n.svelte";
  import { category, extension } from "../filetypes";
  import FileIcon from "./FileIcon.svelte";
  import Icon from "./Icon.svelte";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";

  let { extraItemActions = () => [], extraFolderActions = () => [] }: {
    /** Additional context-menu entries for the selected items (e.g. restore). */
    extraItemActions?: (selected: Entry[]) => MenuItem[];
    /** Additional context-menu entries for the folder itself (right-click on empty space). */
    extraFolderActions?: () => MenuItem[];
  } = $props();

  const OVERSCAN = 6;
  const geometry: Record<ViewMode, { rowH: number; cellW: number; icon: number }> = {
    details: { rowH: 34, cellW: 0, icon: 20 },
    list: { rowH: 30, cellW: 0, icon: 18 },
    icons: { rowH: 128, cellW: 118, icon: 56 },
    tiles: { rowH: 72, cellW: 300, icon: 40 },
  };

  let viewport: HTMLDivElement | undefined = $state();
  let height = $state(0);
  let width = $state(0);
  let scrollTop = $state(0);
  let anchor: string | null = null;
  let typeahead = "";
  let typeaheadTimer: ReturnType<typeof setTimeout> | undefined;
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  /** Widths while a column separator is being dragged (saved on release). */
  let liveWidths = $state<Partial<Record<ColumnId, number>>>({});

  let mode = $derived(app.settings.viewMode);
  let geo = $derived(geometry[mode]);
  let grid = $derived(mode === "icons" || mode === "tiles");
  let rows = $derived(app.visible);
  let cols = $derived(grid ? Math.max(1, Math.floor((width - 12) / geo.cellW)) : 1);
  let rowCount = $derived(Math.ceil(rows.length / cols));
  let startRow = $derived(Math.max(0, Math.floor(scrollTop / geo.rowH) - OVERSCAN));
  let endRow = $derived(Math.min(rowCount, Math.ceil((scrollTop + height) / geo.rowH) + OVERSCAN));
  let slice = $derived(rows.slice(startRow * cols, endRow * cols));
  let focusIndex = $derived(app.focusName === null ? -1 : rows.findIndex((r) => r.name === app.focusName));
  let pageRows = $derived(Math.max(1, Math.floor(height / geo.rowH) - 1));

  let columns = $derived(app.visibleColumns.map((c) => ({ ...c, width: liveWidths[c.id] ?? c.width })));
  let template = $derived(`minmax(150px, 1fr) ${columns.map((c) => `${c.width}px`).join(" ")}`);

  const columnLabel: Record<ColumnId, string> = {
    mtime: "list.modified",
    type: "list.type",
    size: "list.size",
    mode: "col.mode",
    owner: "col.owner",
    items: "col.items",
  };

  function typeLabel(e: Entry): string {
    if (e.kind === "d") return t("list.folder");
    if (e.kind === "s") return t("list.link");
    if (e.kind === "?") return t("list.unknown");
    const cat = category(e);
    if (cat !== "file") return t(`type.${cat}`);
    const ext = extension(e.name);
    return ext ? t("type.ext", { ext: ext.toUpperCase() }) : t("type.file");
  }

  /** "0755" → "rwxr-xr-x". */
  function permissions(mode?: string): string {
    if (!mode) return "";
    const n = parseInt(mode, 8);
    if (Number.isNaN(n)) return mode;
    const bits = "rwxrwxrwx";
    let out = "";
    for (let i = 0; i < 9; i++) out += n & (1 << (8 - i)) ? bits[i] : "-";
    return out;
  }

  function cell(e: Entry, id: ColumnId): string {
    switch (id) {
      case "mtime":
        return e.mtime ? i18n.dateTime(e.mtime) : "";
      case "type":
        return typeLabel(e);
      case "size":
        return e.kind === "s" ? "" : i18n.size(e.size);
      case "mode":
        return permissions(e.mode);
      case "owner":
        return e.uid !== undefined ? `${e.uid}:${e.gid ?? 0}` : "";
      case "items":
        return e.kind === "d" && e.files !== undefined ? i18n.number(e.files) : "";
    }
  }

  function rowId(i: number): string {
    return `item-${i}`;
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
    const top = Math.floor(i / cols) * geo.rowH;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + geo.rowH > viewport.scrollTop + height) viewport.scrollTop = top + geo.rowH - height + 8;
  }

  $effect(() => {
    ensureVisible(focusIndex);
  });

  function rangeNames(a: number, b: number): string[] {
    const [lo, hi] = a <= b ? [a, b] : [b, a];
    return rows.slice(lo, hi + 1).map((r) => r.name);
  }

  function clickItem(e: MouseEvent, entry: Entry, i: number) {
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

  function moveTo(i: number, e: { shiftKey: boolean; ctrlKey: boolean }) {
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

  // --- context menus --------------------------------------------------------------------------

  function selectedEntries(): Entry[] {
    return rows.filter((r) => app.selection.has(r.name));
  }

  function snapshotPath(name: string): string {
    const base = app.source?.path.replace(/[\\/]+$/, "") ?? "";
    return [base, ...app.shownPath, name].join("/");
  }

  function copyPaths(entries: Entry[]) {
    void navigator.clipboard?.writeText(entries.map((e) => snapshotPath(e.name)).join("\n")).catch(() => {});
  }

  function viewItems(): MenuItem[] {
    const views: ViewMode[] = ["details", "list", "icons", "tiles"];
    return views.map((v) => ({ label: t(`view.${v}`), checked: mode === v, action: () => app.setViewMode(v) }));
  }

  function itemMenu(): MenuItem[] {
    const sel = selectedEntries();
    const items: MenuItem[] = [];
    if (sel.length === 1 && sel[0].kind === "d") {
      items.push({ label: t("menu.open"), icon: "folder-open", primary: true, action: () => app.open(sel[0]) });
    }
    items.push(...extraItemActions(sel));
    if (items.length) items.push({ separator: true });
    items.push({ label: t("menu.copyPath"), icon: "copy", action: () => copyPaths(sel) });
    items.push({ label: t("menu.selectAll"), icon: "select-all", action: () => selectAll() });
    return items;
  }

  function folderMenu(): MenuItem[] {
    const extra = extraFolderActions();
    return [
      ...extra,
      ...(extra.length ? [{ separator: true } as MenuItem] : []),
      { label: t("menu.selectAll"), icon: "select-all", action: () => selectAll(), disabled: rows.length === 0 },
      { separator: true },
      ...viewItems(),
    ];
  }

  function headerMenu(): MenuItem[] {
    return [
      ...app.columns.map((c) => ({
        label: t(columnLabel[c.id]),
        checked: c.visible,
        action: () => app.setColumn(c.id, { visible: !c.visible }),
      })),
      { separator: true },
      { label: t("col.reset"), action: () => app.resetColumns() },
    ];
  }

  function selectAll() {
    app.selection = new Set(rows.map((r) => r.name));
  }

  function openMenu(x: number, y: number, items: MenuItem[]) {
    menu = { x, y, items };
  }

  function closeMenu() {
    menu = null;
    viewport?.focus();
  }

  function onItemContext(e: MouseEvent, entry: Entry) {
    e.preventDefault();
    e.stopPropagation();
    if (!app.selection.has(entry.name)) {
      app.selection = new Set([entry.name]);
      anchor = entry.name;
    }
    app.focusName = entry.name;
    openMenu(e.clientX, e.clientY, itemMenu());
  }

  function onEmptyContext(e: MouseEvent) {
    e.preventDefault();
    app.selection = new Set();
    openMenu(e.clientX, e.clientY, folderMenu());
  }

  function menuFromKeyboard() {
    if (focusIndex >= 0 && app.selection.size > 0) {
      const el = document.getElementById(rowId(focusIndex));
      const r = el?.getBoundingClientRect();
      openMenu(r ? r.left + 40 : 100, r ? r.bottom : 100, itemMenu());
    } else {
      const r = viewport?.getBoundingClientRect();
      openMenu((r?.left ?? 0) + 40, (r?.top ?? 0) + 20, folderMenu());
    }
  }

  // --- column resizing ------------------------------------------------------------------------

  let drag: { id: ColumnId; startX: number; startW: number } | null = null;

  function startResize(e: PointerEvent, id: ColumnId, w: number) {
    e.preventDefault();
    e.stopPropagation();
    drag = { id, startX: e.clientX, startW: w };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function moveResize(e: PointerEvent) {
    if (!drag) return;
    // The separator is the left edge of the column: dragging right makes the column narrower.
    const w = Math.round(Math.min(MAX_COLUMN, Math.max(MIN_COLUMN, drag.startW - (e.clientX - drag.startX))));
    liveWidths = { ...liveWidths, [drag.id]: w };
  }

  function endResize() {
    if (!drag) return;
    const w = liveWidths[drag.id];
    if (w !== undefined) app.setColumn(drag.id, { width: w });
    drag = null;
    liveWidths = {};
  }

  function keyResize(e: KeyboardEvent, id: ColumnId, w: number) {
    const step = e.shiftKey ? 40 : 10;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      const next = w + (e.key === "ArrowLeft" ? step : -step);
      app.setColumn(id, { width: Math.min(MAX_COLUMN, Math.max(MIN_COLUMN, next)) });
    }
  }

  function defaultWidth(id: ColumnId): number {
    return DEFAULT_COLUMNS.find((c) => c.id === id)?.width ?? 120;
  }

  // --- keyboard -------------------------------------------------------------------------------

  function onkeydown(e: KeyboardEvent) {
    const cur = focusIndex;
    if ((e.shiftKey && e.key === "F10") || e.key === "ContextMenu") {
      e.preventDefault();
      menuFromKeyboard();
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        if (e.altKey) return;
        moveTo(cur < 0 ? 0 : cur + cols, e);
        break;
      case "ArrowUp":
        if (e.altKey) {
          app.up();
          break;
        }
        moveTo(cur < 0 ? 0 : cur - cols, e);
        break;
      case "ArrowLeft":
        if (e.altKey) app.goBack();
        else if (grid) moveTo(cur - 1, e);
        else return;
        break;
      case "ArrowRight":
        if (e.altKey) app.goForward();
        else if (grid) moveTo(cur + 1, e);
        else return;
        break;
      case "Home":
        moveTo(0, e);
        break;
      case "End":
        moveTo(rows.length - 1, e);
        break;
      case "PageDown":
        moveTo(cur + pageRows * cols, e);
        break;
      case "PageUp":
        moveTo(cur - pageRows * cols, e);
        break;
      case "Enter":
        if (cur >= 0) app.open(rows[cur]);
        break;
      case "Backspace":
        app.up();
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
          selectAll();
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
              moveTo((from + k) % rows.length, { shiftKey: false, ctrlKey: false });
              break;
            }
          }
          break;
        }
        return;
    }
    e.preventDefault();
  }

  function onBackgroundDown(e: PointerEvent) {
    if (e.button === 0 && (e.target === viewport || (e.target as HTMLElement).classList.contains("spacer"))) {
      if (!e.ctrlKey && !e.shiftKey) app.selection = new Set();
    }
  }

  let missingName = $derived(app.wantedPath[app.shownDepth] ?? "");
  let shownName = $derived(app.shownPath.at(-1) ?? app.source?.path.split("/").filter(Boolean).at(-1) ?? "/");

  function position(i: number): string {
    const r = Math.floor(i / cols);
    const c = i % cols;
    return grid ? `translate(${c * geo.cellW}px, ${r * geo.rowH}px)` : `translateY(${r * geo.rowH}px)`;
  }
</script>

<div class="list view-{mode}" class:busy={app.listing} style:--cols={template}>
  {#if app.shownDepth < app.wantedPath.length && !app.listing}
    <div class="notice" role="status">
      <Icon name="info" size={16} />
      <span>{t("notice.missing", { name: missingName, shown: shownName })}</span>
    </div>
  {/if}

  {#if mode === "details"}
    <div class="header" role="toolbar" aria-label={t("menu.columns")} tabindex="-1" oncontextmenu={(e) => (e.preventDefault(), openMenu(e.clientX, e.clientY, headerMenu()))}>
      {@render sortButton("name", t("list.name"), "c-name")}
      {#each columns as c (c.id)}
        <div class="hcell" class:right={c.id === "size" || c.id === "items"}>
          <!-- A focusable separator is an interactive "window splitter" in ARIA; Svelte's lint does not know. -->
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="resizer"
            role="separator"
            aria-orientation="vertical"
            aria-label="{t('col.resize')}: {t(columnLabel[c.id])}"
            aria-valuenow={c.width}
            aria-valuemin={MIN_COLUMN}
            aria-valuemax={MAX_COLUMN}
            tabindex="0"
            onpointerdown={(e) => startResize(e, c.id, c.width)}
            onpointermove={moveResize}
            onpointerup={endResize}
            onpointercancel={endResize}
            ondblclick={() => app.setColumn(c.id, { width: defaultWidth(c.id) })}
            onkeydown={(e) => keyResize(e, c.id, c.width)}
          ></div>
          {@render sortButton(c.id, t(columnLabel[c.id]), "")}
        </div>
      {/each}
    </div>
  {/if}

  <div
    class="viewport"
    bind:this={viewport}
    bind:clientHeight={height}
    bind:clientWidth={width}
    onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
    onpointerdown={onBackgroundDown}
    oncontextmenu={onEmptyContext}
    {onkeydown}
    tabindex="0"
    role="listbox"
    aria-multiselectable="true"
    aria-label={t("list.name")}
    aria-activedescendant={focusIndex >= 0 ? rowId(focusIndex) : undefined}
  >
    <div class="spacer" style:height="{rowCount * geo.rowH}px">
      {#each slice as entry, k (entry.name)}
        {@const i = startRow * cols + k}
        {@const selected = app.selection.has(entry.name)}
        <div
          id={rowId(i)}
          class="item"
          class:selected
          class:focused={i === focusIndex}
          style:transform={position(i)}
          role="option"
          aria-selected={selected}
          aria-setsize={rows.length}
          aria-posinset={i + 1}
          tabindex="-1"
          onclick={(e) => clickItem(e, entry, i)}
          ondblclick={() => app.open(entry)}
          oncontextmenu={(e) => onItemContext(e, entry)}
          onkeydown={() => {}}
        >
          {#if mode === "details"}
            <span class="c-name">
              <FileIcon category={category(entry)} size={geo.icon} />
              <span class="name" title={entry.name}>{entry.name}</span>
            </span>
            {#each columns as c (c.id)}
              <span class="c" class:right={c.id === "size" || c.id === "items"} class:dim={entry.kind === "d" && c.id === "size"}>{cell(entry, c.id)}</span>
            {/each}
          {:else if mode === "list"}
            <span class="c-name">
              <FileIcon category={category(entry)} size={geo.icon} />
              <span class="name" title={entry.name}>{entry.name}</span>
            </span>
          {:else if mode === "icons"}
            <FileIcon category={category(entry)} size={geo.icon} />
            <span class="label" title={entry.name}>{entry.name}</span>
          {:else}
            <FileIcon category={category(entry)} size={geo.icon} />
            <span class="tile-text">
              <span class="name" title={entry.name}>{entry.name}</span>
              <span class="meta">{typeLabel(entry)}</span>
              <span class="meta">{entry.kind === "s" ? "" : i18n.size(entry.size)}{entry.mtime ? ` · ${i18n.dateTime(entry.mtime)}` : ""}</span>
            </span>
          {/if}
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

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={closeMenu} />
{/if}

{#snippet sortButton(key: SortKey, label: string, cls: string)}
  <button
    class="col {cls}"
    class:sorted={app.sortKey === key}
    onclick={() => app.sortBy(key)}
    aria-label="{label}{app.sortKey === key ? (app.sortAsc ? ' ▲' : ' ▼') : ''}"
  >
    <span>{label}</span>
    {#if app.sortKey === key}
      <Icon name={app.sortAsc ? "chevron-up" : "chevron-down"} size={13} stroke={1.8} />
    {/if}
  </button>
{/snippet}

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

  .header {
    display: grid;
    grid-template-columns: var(--cols);
    align-items: center;
    height: 34px;
    margin: 0 6px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
  }

  .hcell {
    position: relative;
    display: flex;
    align-items: center;
    height: 100%;
    padding-left: 8px;
    min-width: 0;
  }

  .hcell.right {
    justify-content: flex-end;
  }

  .resizer {
    position: absolute;
    left: -4px;
    top: 6px;
    bottom: 6px;
    width: 9px;
    cursor: col-resize;
    z-index: 1;
  }

  .resizer::after {
    content: "";
    position: absolute;
    left: 4px;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--border);
    transition: background 0.1s;
  }

  .resizer:hover::after,
  .resizer:focus-visible::after {
    background: var(--accent);
    width: 2px;
  }

  .col {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 8px;
    margin: 0 -8px 0 -8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
    text-align: left;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }

  .hcell .col {
    margin: 0;
  }

  .col:hover {
    background: var(--hover);
  }

  .col.sorted {
    color: var(--text);
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

  .item {
    position: absolute;
    top: 0;
    left: 0;
    border-radius: var(--radius-sm);
    cursor: default;
    color: var(--text);
  }

  .item:hover {
    background: var(--hover);
  }

  .item.selected {
    background: var(--selected);
  }

  .item.selected:hover {
    background: var(--selected-hover);
  }

  .item.focused {
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }

  .item.selected.focused {
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  /* details */
  .view-details .item {
    right: 0;
    height: 34px;
    display: grid;
    grid-template-columns: var(--cols);
    align-items: center;
    padding: 0 10px;
  }

  .view-details .item.selected.focused {
    box-shadow: inset 3px 0 0 0 var(--accent);
  }

  .c {
    padding-left: 8px;
    color: var(--text-2);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }

  .c.right {
    text-align: right;
  }

  .c.dim {
    color: var(--text-3);
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

  /* list */
  .view-list .item {
    right: 0;
    height: 30px;
    display: flex;
    align-items: center;
    padding: 0 10px;
  }

  /* large icons */
  .view-icons .item {
    width: 112px;
    height: 122px;
    margin: 3px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 10px 6px 6px;
    text-align: center;
  }

  .label {
    font-size: 12.5px;
    line-height: 1.3;
    max-width: 100%;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }

  /* tiles */
  .view-tiles .item {
    width: 292px;
    height: 66px;
    margin: 3px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 12px;
  }

  .tile-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }

  .meta {
    color: var(--text-3);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    pointer-events: none;
  }

  .state :global(button) {
    pointer-events: auto;
  }

  .busy .spacer {
    opacity: 0.55;
    transition: opacity 0.2s 0.15s;
  }

  .progress {
    position: absolute;
    left: 16px;
    right: 16px;
    top: 0;
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
</style>
