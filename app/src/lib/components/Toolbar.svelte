<script lang="ts">
  import type { ViewMode } from "../api";
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  let searchInput: HTMLInputElement | undefined = $state();

  const views: { mode: ViewMode; icon: string }[] = [
    { mode: "details", icon: "view-details" },
    { mode: "list", icon: "view-list" },
    { mode: "icons", icon: "view-icons" },
    { mode: "tiles", icon: "view-tiles" },
  ];

  let rootName = $derived(app.source ? (app.source.path.split("/").filter(Boolean).at(-1) ?? "/") : "");
  let shown = $derived(app.shownPath);
  let missing = $derived(app.wantedPath.slice(app.shownDepth));

  // Collapse long paths: root › … › last three.
  let crumbs = $derived.by(() => {
    const all = shown.map((name, i) => ({ name, path: shown.slice(0, i + 1) }));
    if (all.length <= 4) return { hidden: [] as typeof all, visible: all };
    return { hidden: all.slice(0, all.length - 3), visible: all.slice(-3) };
  });

  function onWindowKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<div class="toolbar">
  <div class="nav">
    <button class="icon-btn" onclick={() => app.goBack()} disabled={!app.canBack} title={t("nav.back")} aria-label={t("nav.back")}>
      <Icon name="arrow-left" />
    </button>
    <button class="icon-btn" onclick={() => app.goForward()} disabled={!app.canForward} title={t("nav.forward")} aria-label={t("nav.forward")}>
      <Icon name="arrow-right" />
    </button>
    <button class="icon-btn" onclick={() => app.up()} disabled={shown.length === 0} title={t("nav.up")} aria-label={t("nav.up")}>
      <Icon name="arrow-up" />
    </button>
  </div>

  <nav class="crumbs" aria-label={t("app.source")}>
    <ol>
      <li>
        <button class="crumb root" onclick={() => app.navigate([])} aria-current={shown.length === 0 ? "location" : undefined}>
          <Icon name="server" size={15} />
          <span>{rootName}</span>
        </button>
      </li>
      {#if crumbs.hidden.length}
        <li class="sep" aria-hidden="true"><Icon name="chevron-right" size={13} /></li>
        <li>
          <button class="crumb" onclick={() => app.navigate(crumbs.hidden.at(-1)!.path)} title={crumbs.hidden.map((c) => c.name).join(" / ")}>…</button>
        </li>
      {/if}
      {#each crumbs.visible as c, i (c.path.join("/"))}
        <li class="sep" aria-hidden="true"><Icon name="chevron-right" size={13} /></li>
        <li>
          <button
            class="crumb"
            onclick={() => app.navigate(c.path)}
            aria-current={i === crumbs.visible.length - 1 ? "location" : undefined}>{c.name}</button
          >
        </li>
      {/each}
      {#each missing as m (m)}
        <li class="sep missing" aria-hidden="true"><Icon name="chevron-right" size={13} /></li>
        <li class="crumb missing" title={t("notice.missing", { name: m, shown: shown.at(-1) ?? rootName })}>{m}</li>
      {/each}
    </ol>
  </nav>

  <div class="views" role="radiogroup" aria-label={t("view.label")}>
    {#each views as v (v.mode)}
      <button
        class="icon-btn"
        class:active={app.settings.viewMode === v.mode}
        role="radio"
        aria-checked={app.settings.viewMode === v.mode}
        title={t(`view.${v.mode}`)}
        aria-label={t(`view.${v.mode}`)}
        onclick={() => app.setViewMode(v.mode)}
      >
        <Icon name={v.icon} size={17} />
      </button>
    {/each}
  </div>

  <div class="search">
    <Icon name="search" size={16} />
    <input
      bind:this={searchInput}
      type="search"
      placeholder={t("nav.search")}
      aria-label={t("nav.search")}
      bind:value={app.search}
      onkeydown={(e) => {
        if (e.key === "Escape") app.search = "";
      }}
    />
    {#if app.search}
      <button class="clear" onclick={() => (app.search = "")} aria-label={t("nav.clearSearch")}><Icon name="x" size={14} /></button>
    {/if}
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 8px;
  }

  .nav {
    display: flex;
    gap: 2px;
  }

  .crumbs {
    flex: 1;
    min-width: 0;
    height: 34px;
    display: flex;
    align-items: center;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    overflow: hidden;
  }

  ol {
    display: flex;
    align-items: center;
    gap: 1px;
    list-style: none;
    margin: 0;
    padding: 0;
    min-width: 0;
    white-space: nowrap;
  }

  li {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 7px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--text);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.crumb:hover {
    background: var(--hover);
  }

  .crumb.root {
    color: var(--text-2);
  }

  .crumb.missing {
    color: var(--text-3);
    font-style: italic;
    text-decoration: line-through;
    text-decoration-color: var(--text-3);
    padding: 0 7px;
  }

  .sep {
    color: var(--text-3);
  }

  .sep.missing {
    opacity: 0.6;
  }

  .views {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
  }

  .views .icon-btn {
    width: 30px;
    height: 28px;
  }

  .views .icon-btn.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow);
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
    width: 260px;
    color: var(--text-3);
  }

  .search :global(svg:first-child) {
    position: absolute;
    left: 10px;
    pointer-events: none;
  }

  .search input {
    width: 100%;
    padding-left: 34px;
    padding-right: 30px;
    background: var(--surface-2);
    border-color: var(--border);
  }

  .search input::-webkit-search-cancel-button {
    display: none;
  }

  .clear {
    position: absolute;
    right: 5px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--text-2);
  }

  .clear:hover {
    background: var(--hover);
  }
</style>
