<script lang="ts">
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";
</script>

<main class="mismatch">
  <div class="card" role="alert">
    <div class="icon"><Icon name="shield-x" size={30} /></div>
    <h1>{t("mismatch.title")}</h1>
    <p>{t("mismatch.body")}</p>
    <dl>
      <dt>{t("mismatch.trusted")}</dt>
      <dd><code>{app.settings.fingerprint}</code></dd>
      <dt>{t("mismatch.presented")}</dt>
      <dd><code class="new">{app.presentedFingerprint}</code></dd>
    </dl>
    <p class="server">{app.settings.server}</p>
    <button class="btn" onclick={() => app.retrustCertificate()}>{t("mismatch.reset")}</button>
  </div>
</main>

<style>
  .mismatch {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .card {
    width: min(560px, 100%);
    padding: 28px 30px;
    border-radius: 12px;
    border: 1px solid var(--border);
    border-top: 4px solid var(--danger);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    user-select: text;
  }

  .icon {
    color: var(--danger);
  }

  h1 {
    margin: 8px 0 8px;
    font-size: 20px;
    font-weight: 600;
  }

  p {
    color: var(--text-2);
    margin: 0 0 16px;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    margin: 0 0 16px;
    font-size: 13px;
  }

  dt {
    color: var(--text-3);
  }

  dd {
    margin: 0;
  }

  code {
    font-family: var(--mono);
    font-size: 12px;
    word-break: break-all;
  }

  code.new {
    color: var(--danger);
  }

  .server {
    font-size: 13px;
    color: var(--text-3);
  }
</style>
