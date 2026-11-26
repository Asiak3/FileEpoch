<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  let dlg = $derived(app.confirmState!);
  let cancelButton: HTMLButtonElement | undefined = $state();
  let dialog: HTMLDivElement | undefined = $state();

  // The safe choice has the focus; Escape cancels; Tab stays inside the dialog.
  onMount(() => cancelButton?.focus());

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      dlg.resolve(false);
    } else if (e.key === "Tab" && dialog) {
      const buttons = [...dialog.querySelectorAll<HTMLElement>("button")];
      const i = buttons.indexOf(document.activeElement as HTMLElement);
      e.preventDefault();
      buttons[(i + (e.shiftKey ? buttons.length - 1 : 1)) % buttons.length]?.focus();
    }
  }
</script>

<div class="backdrop">
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-body" tabindex="-1" bind:this={dialog} {onkeydown}>
    <div class="icon"><Icon name="warning" size={26} /></div>
    <h2 id="confirm-title">{dlg.title}</h2>
    <p id="confirm-body">{dlg.body}</p>
    <ul>
      {#each dlg.items.slice(0, 8) as item (item)}
        <li><code>{item}</code></li>
      {/each}
      {#if dlg.items.length > 8}
        <li class="more">{t("restore.more", { n: dlg.items.length - 8 })}</li>
      {/if}
    </ul>
    <div class="buttons">
      <button class="btn" bind:this={cancelButton} onclick={() => dlg.resolve(false)}>{t("common.cancel")}</button>
      <button class="btn danger" onclick={() => dlg.resolve(true)}>{dlg.yes}</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
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
    width: min(520px, 100%);
    padding: 24px 26px 22px;
    border-radius: 12px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    outline: none;
  }

  .icon {
    color: var(--danger);
  }

  h2 {
    margin: 8px 0 8px;
    font-size: 18px;
    font-weight: 600;
  }

  p {
    margin: 0 0 12px;
    color: var(--text-2);
  }

  ul {
    margin: 0 0 18px;
    padding: 8px 12px;
    list-style: none;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    max-height: 180px;
    overflow: auto;
  }

  li {
    padding: 2px 0;
    font-size: 12.5px;
    word-break: break-all;
  }

  code {
    font-family: var(--mono);
  }

  .more {
    color: var(--text-3);
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn.danger {
    background: var(--danger);
    border-color: transparent;
    color: #fff;
  }

  .btn.danger:hover {
    filter: brightness(1.08);
    background: var(--danger);
  }
</style>
