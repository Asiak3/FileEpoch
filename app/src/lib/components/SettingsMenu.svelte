<script lang="ts">
  import { tick } from "svelte";
  import type { Language, Settings, Theme } from "../api";
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  type Tab = "system" | "display" | "restore" | "connection" | "about";
  const tabs: { id: Tab; icon: string }[] = [
    { id: "system", icon: "monitor" },
    { id: "display", icon: "palette" },
    { id: "restore", icon: "restore" },
    { id: "connection", icon: "server" },
    { id: "about", icon: "info" },
  ];

  let open = $state(false);
  let tab = $state<Tab>("system");
  let message = $state("");
  let dialog: HTMLDivElement | undefined = $state();
  let opener: HTMLButtonElement | undefined = $state();

  const themes: Theme[] = ["system", "light", "dark"];
  const languages: { value: Language; label: string }[] = [
    { value: "auto", label: "" },
    { value: "es", label: "Español" },
    { value: "de", label: "Deutsch" },
    { value: "en", label: "English" },
  ];

  type NamingChoice = "snapshot" | "modified" | "original";
  let naming = $derived<NamingChoice>(app.settings.restoreNaming === "original" ? "original" : app.settings.suffixDate);

  function setNaming(v: NamingChoice) {
    void app.saveSettings(v === "original" ? { restoreNaming: "original" } : { restoreNaming: "suffix", suffixDate: v });
  }

  const pad = (n: number) => String(n).padStart(2, "0");
  /** Same format as the restore engine: 2026-11-25 03h02 (local time). */
  function stamp(ms: number): string {
    const d = new Date(ms);
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}h${pad(d.getMinutes())}`;
  }

  /** What a file of the current folder would be called, so the choice is concrete. */
  let example = $derived.by(() => {
    const file = app.visible.find((e) => e.kind === "f" && e.mtime && /.\.[^.]+$/.test(e.name));
    const name = file?.name ?? "report.pdf";
    if (naming === "original") return name;
    const time = naming === "modified" ? (file?.mtime ?? Date.now()) : app.point ? Date.parse(app.point.snapshot.startTime) : Date.now();
    const dot = name.lastIndexOf(".");
    const word = t(naming === "modified" ? "restore.word.modified" : "restore.word.snapshot");
    return `${name.slice(0, dot)} (${word} ${stamp(time)})${name.slice(dot)}`;
  });

  async function show() {
    open = true;
    message = "";
    await tick();
    dialog?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();
  }

  function close() {
    open = false;
    opener?.focus();
  }

  async function forget() {
    const had = await app.forgetPassword().catch(() => false);
    message = had ? t("app.forgotten") : "";
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Tab" && dialog) {
      // Keep the focus inside the dialog.
      const items = [...dialog.querySelectorAll<HTMLElement>("button, select, input, [tabindex='0']")].filter((el) => !el.hasAttribute("disabled"));
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  }

  function tabKeys(e: KeyboardEvent) {
    const i = tabs.findIndex((x) => x.id === tab);
    let j = -1;
    if (e.key === "ArrowDown") j = (i + 1) % tabs.length;
    else if (e.key === "ArrowUp") j = (i - 1 + tabs.length) % tabs.length;
    else if (e.key === "Home") j = 0;
    else if (e.key === "End") j = tabs.length - 1;
    if (j < 0) return;
    e.preventDefault();
    tab = tabs[j].id;
    void tick().then(() => dialog?.querySelector<HTMLElement>(`#tab-${tab}`)?.focus());
  }
</script>

<button class="icon-btn" bind:this={opener} onclick={show} aria-haspopup="dialog" title={t("app.settings")} aria-label={t("app.settings")}>
  <Icon name="settings" size={19} />
</button>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="backdrop" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" bind:this={dialog} {onkeydown}>
      <nav class="side">
        <h2 id="settings-title">{t("app.settings")}</h2>
        <div class="tabs" role="tablist" aria-orientation="vertical" tabindex="-1" onkeydown={tabKeys}>
          {#each tabs as x (x.id)}
            <button
              id="tab-{x.id}"
              role="tab"
              aria-selected={tab === x.id}
              aria-controls="panel"
              tabindex={tab === x.id ? 0 : -1}
              onclick={() => (tab = x.id)}
            >
              <Icon name={x.icon} size={17} />
              <span>{t(`settings.tab.${x.id}`)}</span>
            </button>
          {/each}
        </div>
      </nav>

      <section class="content">
        <header>
          <h3>{t(`settings.tab.${tab}`)}</h3>
          <button class="icon-btn" onclick={close} aria-label={t("common.close")} title={t("common.close")}><Icon name="x" size={16} /></button>
        </header>

        <div class="panel" id="panel" role="tabpanel" aria-labelledby="tab-{tab}">
          {#if tab === "system"}
            <div class="field">
              <label for="lang">{t("app.language")}</label>
              <select id="lang" value={app.settings.language} onchange={(e) => app.saveSettings({ language: e.currentTarget.value as Language })}>
                {#each languages as l (l.value)}
                  <option value={l.value}>{l.value === "auto" ? t("lang.auto") : l.label}</option>
                {/each}
              </select>
            </div>
            <div class="group">
              <h4>{t("settings.updates")}</h4>
              <label class="check">
                <input type="checkbox" checked={app.settings.autoUpdate} onchange={(e) => app.saveSettings({ autoUpdate: e.currentTarget.checked })} />
                <span>{t("settings.autoUpdate")}<small>{t("settings.autoUpdateHint")}</small></span>
              </label>
              <div class="update-row">
                <button class="btn" onclick={() => app.checkForUpdates(true)} disabled={app.updateStatus === "checking" || app.updateStatus === "installing"}>
                  <Icon name="refresh" size={15} />
                  {t("settings.checkNow")}
                </button>
                <span class="update-status" role="status">
                  {#if app.updateStatus === "checking"}{t("settings.checking")}
                  {:else if app.update}{t("settings.updateFound", { version: app.update.version })}
                  {:else if app.updateStatus === "current"}{t("settings.upToDate", { version: __APP_VERSION__ })}
                  {:else if app.updateStatus === "failed"}{t("settings.checkFailed")}
                  {/if}
                </span>
              </div>
            </div>
            <div class="group">
              <h4>{t("settings.desktop")}</h4>
              <label class="check">
                <input type="checkbox" checked={app.settings.tray} onchange={(e) => app.saveSettings({ tray: e.currentTarget.checked })} />
                <span>{t("settings.tray")}<small>{t("settings.trayHint")}</small></span>
              </label>
              <label class="check">
                <input type="checkbox" checked={app.autostart} onchange={(e) => app.setAutostart(e.currentTarget.checked)} />
                <span>{t("settings.autostart")}<small>{t("settings.autostartHint")}</small></span>
              </label>
            </div>
          {:else if tab === "display"}
            <div class="field">
              <span class="flabel" id="theme-label">{t("app.theme")}</span>
              <div class="segmented" role="radiogroup" aria-labelledby="theme-label">
                {#each themes as th (th)}
                  <button role="radio" aria-checked={app.settings.theme === th} onclick={() => app.saveSettings({ theme: th })}>{t(`theme.${th}`)}</button>
                {/each}
              </div>
            </div>
            <div class="field">
              <span class="flabel" id="tl-label">{t("settings.timelineStyle")}</span>
              <div class="segmented two" role="radiogroup" aria-labelledby="tl-label">
                {#each ["glass", "simple"] as const as st (st)}
                  <button role="radio" aria-checked={app.settings.timeline === st} onclick={() => app.saveSettings({ timeline: st })}>
                    {t(`settings.timelineStyle.${st}`)}
                  </button>
                {/each}
              </div>
            </div>
            <label class="check">
              <input type="checkbox" checked={app.settings.thumbnails} onchange={(e) => app.saveSettings({ thumbnails: e.currentTarget.checked })} />
              <span>{t("settings.thumbnails")}<small>{t("settings.thumbnailsHint")}</small></span>
            </label>
          {:else if tab === "restore"}
            <div class="field">
              <label for="naming">{t("settings.naming")}</label>
              <select id="naming" value={naming} onchange={(e) => setNaming(e.currentTarget.value as NamingChoice)}>
                <option value="snapshot">{t("settings.naming.snapshot")}</option>
                <option value="modified">{t("settings.naming.modified")}</option>
                <option value="original">{t("settings.naming.original")}</option>
              </select>
              <p class="example">{t("settings.naming.example", { name: example })}</p>
            </div>
            <div class="field">
              <label for="conflict">{t("settings.conflict")}</label>
              <select id="conflict" value={app.settings.conflict} onchange={(e) => app.saveSettings({ conflict: e.currentTarget.value as Settings["conflict"] })}>
                <option value="rename">{t("settings.conflict.rename")}</option>
                <option value="skip">{t("settings.conflict.skip")}</option>
                <option value="overwrite">{t("settings.conflict.overwrite")}</option>
              </select>
            </div>
            <label class="check">
              <input type="checkbox" checked={app.settings.verify} onchange={(e) => app.saveSettings({ verify: e.currentTarget.checked })} />
              <span>{t("settings.verify")}</span>
            </label>
          {:else if tab === "connection"}
            <div class="server">
              <Icon name="server" size={18} />
              <div>
                <div class="url">{app.settings.server}</div>
                <div class="meta">
                  {app.settings.user}
                  {#if app.connection?.version}· Kopia v{app.connection.version}{/if}
                  {#if app.connection?.readonly}· <span class="ro"><Icon name="shield" size={13} /> {t("app.readonly")}</span>{/if}
                </div>
              </div>
            </div>
            <div class="actions">
              <button class="btn" onclick={forget}><Icon name="key" size={16} /> {t("app.forget")}</button>
              <button class="btn" onclick={() => ((open = false), app.disconnect())}><Icon name="plug" size={16} /> {t("app.disconnect")}</button>
            </div>
            {#if message}<p class="msg" role="status">{message}</p>{/if}
          {:else}
            <div class="about">
              <svg width="56" height="56" viewBox="0 0 1024 1024" aria-hidden="true">
                <defs>
                  <linearGradient id="about-g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3aa0ff" /><stop offset="1" stop-color="#1450c8" /></linearGradient>
                </defs>
                <rect x="64" y="64" width="896" height="896" rx="208" fill="url(#about-g)" />
                <path d="M196 470A316 316 0 1 1 300 746" fill="none" stroke="#fff" stroke-width="72" stroke-linecap="round" />
                <path d="M96 446L296 446L196 596z" fill="#fff" />
                <circle cx="512" cy="512" r="200" fill="#fff" />
                <path d="M512 400v122l84 56" fill="none" stroke="#1450c8" stroke-width="56" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <div>
                <div class="name">FileEpoch <span>{__APP_VERSION__}</span></div>
                <div class="by">{t("app.by")}</div>
              </div>
            </div>
            <p class="text">{t("settings.about.desc")}</p>
            <button class="link" onclick={() => app.api.openProjectPage()} title={t("app.projectPage")}>
              github.com/Asiak3/FileEpoch <Icon name="external" size={13} />
            </button>
            <p class="text small">{t("settings.about.license")}</p>
          {/if}
        </div>
      </section>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(10, 14, 22, 0.4);
    animation: fade 0.15s var(--ease);
  }

  :global([data-theme="dark"]) .backdrop {
    background: rgba(0, 0, 0, 0.58);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .dialog {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    width: min(760px, 100%);
    height: min(540px, 100%);
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
    outline: none;
    animation: rise 0.18s var(--ease);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.985);
    }
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px 10px;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
    min-width: 0;
  }

  h2 {
    margin: 0 0 4px;
    padding: 0 10px;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 600;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  [role="tab"] {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
    text-align: left;
    min-width: 0;
  }

  [role="tab"] span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  [role="tab"]:hover {
    background: var(--hover);
  }

  [role="tab"][aria-selected="true"] {
    background: var(--selected);
    color: var(--text);
    font-weight: 500;
  }

  .content {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 14px 6px 26px;
  }

  h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
  }

  .panel {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 26px 26px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 7px;
    max-width: 420px;
  }

  .field label,
  .flabel,
  h4 {
    margin: 0;
    color: var(--text);
    font-size: 13px;
    font-weight: 500;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  select {
    height: 34px;
    padding: 0 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    font-size: 13px;
    min-width: 0;
  }

  .segmented {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    padding: 3px;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
  }

  .segmented.two {
    grid-template-columns: repeat(2, 1fr);
  }

  .segmented button {
    min-height: 30px;
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
  }

  .segmented button[aria-checked="true"] {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow);
    font-weight: 500;
  }

  .example {
    margin: 0;
    color: var(--text-3);
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 13px;
    color: var(--text);
  }

  .check input {
    margin-top: 2px;
    accent-color: var(--accent);
    flex: none;
  }

  .check small {
    display: block;
    margin-top: 3px;
    color: var(--text-3);
    font-size: 12px;
    line-height: 1.4;
  }

  .update-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
  }

  .update-row .btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }

  .update-status {
    color: var(--text-2);
    font-size: 12.5px;
  }

  .server {
    display: flex;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--text-2);
    font-size: 13px;
  }

  .url {
    color: var(--text);
    font-weight: 500;
    overflow-wrap: anywhere;
  }

  .meta {
    margin-top: 3px;
  }

  .ro {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--success);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .actions .btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .msg {
    margin: 0;
    font-size: 13px;
    color: var(--success);
  }

  .about {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .name {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
  }

  .name span {
    margin-left: 4px;
    color: var(--text-3);
    font-size: 14px;
    font-weight: 400;
  }

  .by {
    color: var(--text-2);
    font-size: 13px;
  }

  .text {
    margin: 0;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.5;
  }

  .text.small {
    color: var(--text-3);
    font-size: 12px;
  }

  .link {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 0;
    border: none;
    background: none;
    color: var(--accent);
    font-size: 13px;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
