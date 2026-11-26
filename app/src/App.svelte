<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/store.svelte";
  import { i18n, t } from "./lib/i18n.svelte";
  import ConnectView from "./lib/components/ConnectView.svelte";
  import MismatchView from "./lib/components/MismatchView.svelte";
  import Timeline from "./lib/components/Timeline.svelte";
  import Toolbar from "./lib/components/Toolbar.svelte";
  import FileList from "./lib/components/FileList.svelte";
  import SourcePicker from "./lib/components/SourcePicker.svelte";
  import SettingsMenu from "./lib/components/SettingsMenu.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import RestorePanel from "./lib/components/RestorePanel.svelte";
  import ConfirmDialog from "./lib/components/ConfirmDialog.svelte";
  import type { Entry } from "./lib/api";
  import type { MenuItem } from "./lib/components/ContextMenu.svelte";

  let untestedDismissed = $state(false);
  let panelButton: HTMLButtonElement | undefined = $state();

  const restoreItems = (selected: Entry[]): MenuItem[] =>
    selected.length && app.point ? [{ label: t("restore.action"), icon: "restore", primary: true, action: () => app.restore(selected) }] : [];
  const restoreFolder = (): MenuItem[] =>
    app.point ? [{ label: t("restore.folder"), icon: "restore", primary: true, action: () => app.restore(null) }] : [];

  function restoreButton() {
    const selected = app.visible.filter((e) => app.selection.has(e.name));
    void app.restore(selected.length ? selected : null);
  }

  // The selection only contains visible items (a search hides some), so the restore button,
  // the status bar and what is actually restored always agree.
  $effect(() => {
    const names = new Set(app.visible.map((e) => e.name));
    const selection = app.selection;
    if ([...selection].some((n) => !names.has(n))) app.selection = new Set([...selection].filter((n) => names.has(n)));
  });

  let active = $derived(app.activeJobs);
  let activePct = $derived.by(() => {
    const total = active.reduce((a, j) => a + j.bytesTotal, 0);
    return total ? (active.reduce((a, j) => a + j.bytesDone, 0) / total) * 100 : 0;
  });

  onMount(() => {
    void app.init();
    // Only FileEpoch's own context menus: hide the browser's (Back, Refresh, Print…) except in text fields.
    const suppress = (e: MouseEvent) => {
      const el = e.target instanceof Element ? e.target : null;
      if (!el?.closest("input, textarea, [contenteditable=true]")) e.preventDefault();
    };
    document.addEventListener("contextmenu", suppress);
    return () => document.removeEventListener("contextmenu", suppress);
  });

  // Follow the system theme live when "system" is selected.
  $effect(() => {
    document.documentElement.dataset.theme = app.resolvedTheme;
  });

  let point = $derived(app.point);
  let count = $derived(app.visible.length);
</script>

{#if app.screen === "loading"}
  <div class="splash"><div class="spinner"></div></div>
{:else if app.screen === "connect"}
  <ConnectView />
{:else if app.screen === "mismatch"}
  <MismatchView />
{:else}
  <div class="shell">
    <header class="topbar">
      <SourcePicker />
      <div class="spacer"></div>
      <SettingsMenu />
    </header>

    {#if app.connection && !app.connection.readonly}
      <div class="banner warn" role="alert">
        <Icon name="warning" size={17} />
        <span>{t("banner.notReadonly")}</span>
      </div>
    {/if}
    {#if app.update && !app.updateDismissed}
      <div class="banner info update" role="status">
        <Icon name="download" size={17} />
        {#if app.updateStatus === "installing"}
          <span>{t("update.installing", { pct: app.updatePercent === null ? "" : `${app.updatePercent} %` })}</span>
        {:else}
          <span>{t("update.available", { version: app.update.version, current: app.update.current })}</span>
          <button class="link-btn" onclick={() => app.api.openReleasePage(app.update!.version)}>{t("update.notes")}</button>
          <button class="btn primary small" onclick={() => app.installUpdate()}>{t("update.install")}</button>
          <button class="icon-btn" onclick={() => (app.updateDismissed = true)} aria-label={t("common.close")}><Icon name="x" size={14} /></button>
        {/if}
      </div>
    {/if}
    {#if app.connection?.untestedVersion && !untestedDismissed}
      <div class="banner info" role="status">
        <Icon name="info" size={17} />
        <span>{t("banner.untested", { version: app.connection.version ?? "?", tested: app.connection.testedVersions.join(", ") })}</span>
        <button class="icon-btn" onclick={() => (untestedDismissed = true)} aria-label={t("common.close")}><Icon name="x" size={14} /></button>
      </div>
    {/if}

    <div class="timeline-band" class:glass={app.settings.timeline === "glass"}>
      <Timeline />
    </div>

    <section class="browser">
      <Toolbar />
      <FileList extraItemActions={restoreItems} extraFolderActions={restoreFolder} />
    </section>

    <footer class="status">
      <span>{t("list.items", { n: count })}</span>
      {#if point}
        <span class="dot" aria-hidden="true">·</span>
        <span>{t("list.snapshotOf", { time: i18n.dateTime(point.time) })}</span>
      {/if}
      {#if app.selection.size > 0}
        <span class="dot" aria-hidden="true">·</span>
        <span>{t("list.selected", { n: app.selection.size, size: i18n.size(app.selectedSize) })}</span>
      {/if}
      <span class="spacer"></span>
      {#if app.connection}
        <span class="conn" class:ro={app.connection.readonly}>
          <Icon name={app.connection.readonly ? "shield" : "warning"} size={14} />
          Kopia {app.connection.version ? `v${app.connection.version}` : ""}{app.connection.readonly ? ` · ${t("app.readonly")}` : ""}
        </span>
      {/if}
      <button
        class="jobs-btn"
        class:busy={active.length > 0}
        bind:this={panelButton}
        onclick={() => (app.restorePanelOpen = !app.restorePanelOpen)}
        aria-expanded={app.restorePanelOpen}
        aria-haspopup="dialog"
        title={t("restore.panel")}
      >
        {#if active.length}
          <svg class="ring" width="18" height="18" viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="10" cy="10" r="8" fill="none" stroke="var(--surface-3)" stroke-width="3" />
            <circle cx="10" cy="10" r="8" fill="none" stroke="var(--accent)" stroke-width="3" stroke-dasharray="{(activePct / 100) * 50.3} 50.3" transform="rotate(-90 10 10)" />
          </svg>
        {:else}
          <Icon name="download" size={16} />
        {/if}
        <span>{t("restore.panel")}</span>
        {#if active.length}<span class="count">{active.length}</span>{/if}
      </button>
      <button class="btn primary restore" onclick={restoreButton} disabled={!app.point}>
        <Icon name="restore" size={16} />
        {app.selection.size ? `${t("restore.button")} (${app.selection.size})` : t("restore.buttonFolder")}
      </button>
    </footer>
  </div>

  {#if app.restorePanelOpen}
    <RestorePanel anchor={panelButton} />
  {/if}
{/if}

{#if app.confirmState}
  <ConfirmDialog />
{/if}

{#if app.toast}
  <div class="toast" role="status">
    <Icon name="warning" size={16} />
    <span>{app.toast}</span>
    <button class="icon-btn" onclick={() => (app.toast = null)} aria-label={t("common.close")}><Icon name="x" size={14} /></button>
  </div>
{/if}

<style>
  .splash {
    height: 100%;
    display: grid;
    place-items: center;
  }

  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 56px;
    padding: 0 12px 0 12px;
    flex: none;
  }





  .spacer {
    flex: 1;
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 12px 8px;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    flex: none;
  }

  .banner span {
    flex: 1;
  }

  .banner.warn {
    background: var(--warn-bg);
    color: var(--warn-fg);
    border: 1px solid var(--warn-border);
  }

  .banner.info {
    background: var(--info-bg);
    color: var(--info-fg);
  }

  .banner.update span {
    flex: 1;
  }

  .link-btn {
    border: none;
    background: none;
    color: inherit;
    font-size: 13px;
    text-decoration: underline;
    padding: 2px 4px;
  }

  .btn.small {
    height: 28px;
    padding: 0 12px;
    font-size: 12.5px;
  }

  .banner .icon-btn {
    width: 26px;
    height: 26px;
    color: inherit;
  }

  .timeline-band {
    margin: 0 12px;
    border: 1px solid var(--border);
    border-radius: 10px 10px 0 0;
    border-bottom: none;
    background: var(--bg-elev);
    flex: none;
  }

  /* "Glass" timeline (default): neutral frosted surfaces over a barely tinted backdrop
     (see Timeline.svelte). */
  .timeline-band.glass {
    border-color: rgba(255, 255, 255, 0.6);
    background:
      radial-gradient(90% 160% at 10% 0%, rgba(255, 255, 255, 0.85), transparent 60%),
      radial-gradient(80% 160% at 90% 100%, rgba(148, 163, 184, 0.2), transparent 62%),
      linear-gradient(135deg, rgba(10, 98, 201, 0.04), rgba(100, 116, 139, 0.06)),
      var(--bg-elev);
  }

  :global([data-theme="dark"]) .timeline-band.glass {
    border-color: rgba(255, 255, 255, 0.08);
    background:
      radial-gradient(90% 160% at 10% 0%, rgba(255, 255, 255, 0.06), transparent 60%),
      radial-gradient(80% 160% at 90% 100%, rgba(148, 163, 184, 0.08), transparent 62%),
      linear-gradient(135deg, rgba(76, 180, 255, 0.03), rgba(100, 116, 139, 0.05)),
      var(--bg-elev);
  }

  .browser {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    margin: 0 12px;
    border: 1px solid var(--border);
    border-radius: 0 0 10px 10px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 48px;
    padding: 0 14px 0 22px;
    color: var(--text-2);
    font-size: 12.5px;
    flex: none;
  }

  .dot {
    color: var(--text-3);
  }

  .conn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-3);
  }

  .conn.ro {
    color: var(--success);
  }

  .jobs-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    margin-left: 10px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
  }

  .jobs-btn:hover,
  .jobs-btn[aria-expanded="true"] {
    background: var(--hover);
    border-color: var(--border);
  }

  .jobs-btn.busy {
    color: var(--accent);
  }

  .count {
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--accent);
    color: var(--accent-fg);
    font-size: 11px;
    font-weight: 600;
    display: grid;
    place-items: center;
  }

  .restore {
    height: 32px;
    padding: 0 14px;
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 60px;
    transform: translateX(-50%);
    z-index: 40;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(640px, calc(100vw - 40px));
    padding: 8px 8px 8px 14px;
    border-radius: var(--radius);
    background: var(--text);
    color: var(--bg);
    box-shadow: var(--shadow-pop);
    font-size: 13px;
  }

  .toast .icon-btn {
    width: 26px;
    height: 26px;
    color: inherit;
  }
</style>
