<script lang="ts">
  import type { Language, Theme } from "../api";
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
    width: 330px;
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
