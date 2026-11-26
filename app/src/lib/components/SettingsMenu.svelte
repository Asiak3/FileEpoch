<script lang="ts">
  import type { Language, Settings, Theme } from "../api";
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import { dismissable } from "../popover";
  import Icon from "./Icon.svelte";

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let message = $state("");

  $effect(() => {
    if (open && root) return dismissable(root, () => (open = false));
  });

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

  async function forget() {
    const had = await app.forgetPassword().catch(() => false);
    message = had ? t("app.forgotten") : "";
  }
</script>

<div class="menu" bind:this={root}>
  <button class="icon-btn" onclick={() => ((open = !open), (message = ""))} aria-haspopup="dialog" aria-expanded={open} title={t("app.settings")} aria-label={t("app.settings")}>
    <Icon name="settings" size={19} />
  </button>

  {#if open}
    <div class="pop" role="dialog" aria-label={t("app.settings")}>
      <div class="field">
        <span class="label"><Icon name="palette" size={15} /> {t("app.theme")}</span>
        <div class="segmented" role="radiogroup" aria-label={t("app.theme")}>
          {#each themes as th (th)}
            <button role="radio" aria-checked={app.settings.theme === th} onclick={() => app.saveSettings({ theme: th })}>{t(`theme.${th}`)}</button>
          {/each}
        </div>
      </div>

      <div class="field">
        <label class="label" for="lang"><Icon name="globe" size={15} /> {t("app.language")}</label>
        <select id="lang" value={app.settings.language} onchange={(e) => app.saveSettings({ language: e.currentTarget.value as Language })}>
          {#each languages as l (l.value)}
            <option value={l.value}>{l.value === "auto" ? t("lang.auto") : l.label}</option>
          {/each}
        </select>
      </div>

      <label class="check">
        <input type="checkbox" checked={app.settings.thumbnails} onchange={(e) => app.saveSettings({ thumbnails: e.currentTarget.checked })} />
        <span>{t("settings.thumbnails")}</span>
      </label>

      <fieldset class="restore">
        <legend class="label"><Icon name="restore" size={15} /> {t("settings.restore")}</legend>
        <label class="row stack">
          <span>{t("settings.naming")}</span>
          <select value={naming} onchange={(e) => setNaming(e.currentTarget.value as NamingChoice)}>
            <option value="snapshot">{t("settings.naming.snapshot")}</option>
            <option value="modified">{t("settings.naming.modified")}</option>
            <option value="original">{t("settings.naming.original")}</option>
          </select>
        </label>
        <p class="example" title={example}>{t("settings.naming.example", { name: example })}</p>
        <label class="row">
          <span>{t("settings.conflict")}</span>
          <select value={app.settings.conflict} onchange={(e) => app.saveSettings({ conflict: e.currentTarget.value as Settings["conflict"] })}>
            <option value="rename">{t("settings.conflict.rename")}</option>
            <option value="skip">{t("settings.conflict.skip")}</option>
            <option value="overwrite">{t("settings.conflict.overwrite")}</option>
          </select>
        </label>
        <label class="check">
          <input type="checkbox" checked={app.settings.verify} onchange={(e) => app.saveSettings({ verify: e.currentTarget.checked })} />
          <span>{t("settings.verify")}</span>
        </label>
      </fieldset>

      <div class="server">
        <Icon name="server" size={16} />
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
        <button class="item" onclick={forget}><Icon name="key" size={16} /> {t("app.forget")}</button>
        <button class="item" onclick={() => ((open = false), app.disconnect())}><Icon name="plug" size={16} /> {t("app.disconnect")}</button>
      </div>
      {#if message}<p class="msg" role="status">{message}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
  }

  .pop {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 20;
    width: 380px;
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    display: flex;
    flex-direction: column;
    gap: 14px;
    animation: pop 0.14s var(--ease);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--text-2);
    font-size: 12.5px;
  }

  .segmented {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    padding: 3px;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
  }

  .segmented button {
    height: 28px;
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

  select {
    height: 32px;
    padding: 0 8px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
  }

  fieldset.restore {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    border: none;
  }

  fieldset.restore legend {
    margin-bottom: 7px;
    padding: 0;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr 1.15fr;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-2);
  }

  .row.stack {
    grid-template-columns: 1fr;
    gap: 6px;
  }

  .row select {
    min-width: 0;
    font-size: 12.5px;
  }

  .example {
    margin: -2px 0 2px;
    font-size: 11.5px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-2);
  }

  .check input {
    accent-color: var(--accent);
  }

  .server {
    display: flex;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 12.5px;
  }

  .url {
    color: var(--text);
    font-weight: 500;
    word-break: break-all;
  }

  .meta {
    margin-top: 2px;
  }

  .ro {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--success);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-top: 1px solid var(--border);
    padding-top: 8px;
    margin: 0 -6px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
  }

  .item:hover {
    background: var(--hover);
  }

  .msg {
    margin: 0;
    font-size: 12.5px;
    color: var(--success);
  }
</style>
