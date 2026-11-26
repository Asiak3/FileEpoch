<script lang="ts">
  import { toApiError, type ApiError, type CertInfo } from "../api";
  import { app } from "../store.svelte";
  import { t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  let server = $state(app.settings.server || "https://");
  let user = $state(app.settings.user);
  let trusted = $state(app.settings.fingerprint);
  let cert = $state<CertInfo | null>(null);
  let expected = $state("");
  let trustChecked = $state(false);
  let password = $state("");
  let remember = $state(true);
  let probing = $state(false);
  let connecting = $state(false);
  let localError = $state<ApiError | null>(null);

  // A trusted fingerprint only applies to the server it was confirmed for.
  let serverChanged = $derived(server.trim() !== app.settings.server);
  let hasTrust = $derived(!!trusted && !serverChanged);
  let error = $derived(localError ?? app.connectError);

  const clean = (s: string) => s.replace(/[\s:]/g, "").toLowerCase();
  let compare = $derived(cert && clean(expected).length > 0 ? (clean(expected) === cert.hex ? "match" : "nomatch") : null);

  async function probe() {
    localError = null;
    cert = null;
    trustChecked = false;
    probing = true;
    try {
      cert = await app.api.probeCertificate(server.trim());
    } catch (e) {
      localError = toApiError(e);
    } finally {
      probing = false;
    }
  }

  let canConnect = $derived(
    !!server.trim() && !!user.trim() && (hasTrust || (!!cert && trustChecked && compare !== "nomatch")) && !connecting,
  );

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canConnect) return;
    localError = null;
    connecting = true;
    const fingerprint = hasTrust ? trusted : cert!.hex;
    const ok = await app.connect({
      server: server.trim(),
      user: user.trim(),
      fingerprint,
      password: password || null,
      remember,
    });
    connecting = false;
    if (!ok) {
      password = "";
      if (app.connectError?.code === "password_required" || !app.connectError) {
        localError = { code: "password_required", message: "" };
      }
    }
  }

  function changeCertificate() {
    trusted = "";
    cert = null;
    trustChecked = false;
  }
</script>

<main class="connect">
  <form class="card" onsubmit={submit}>
    <div class="brand">
      <svg width="44" height="44" viewBox="0 0 1024 1024" aria-hidden="true">
        <defs>
          <linearGradient id="lg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3aa0ff" /><stop offset="1" stop-color="#1450c8" /></linearGradient>
        </defs>
        <rect x="64" y="64" width="896" height="896" rx="208" fill="url(#lg)" />
        <path d="M196 470A316 316 0 1 1 300 746" fill="none" stroke="#fff" stroke-width="64" stroke-linecap="round" />
        <path d="M106 452L286 452L196 586z" fill="#fff" stroke="#fff" stroke-width="16" stroke-linejoin="round" />
        <circle cx="512" cy="512" r="206" fill="#fff" />
        <path d="M512 400v122l84 56" fill="none" stroke="#1450c8" stroke-width="44" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      <div>
        <h1>{t("connect.title")}</h1>
        <p>{t("connect.subtitle")}</p>
      </div>
    </div>

    <label class="field">
      <span>{t("connect.server")}</span>
      <input type="url" bind:value={server} placeholder="https://backup.example:51515" autocomplete="off" spellcheck="false" required />
    </label>

    <label class="field">
      <span>{t("connect.user")}</span>
      <input type="text" bind:value={user} autocomplete="username" spellcheck="false" required />
      <small>{t("connect.userHint")}</small>
    </label>

    <section class="cert" aria-label={t("connect.certTitle")}>
      {#if hasTrust}
        <div class="trusted">
          <Icon name="shield" size={18} />
          <div>
            <strong>{t("connect.trusted")}</strong>
            <code>{trusted.slice(0, 16)}…{trusted.slice(-8)}</code>
          </div>
          <button type="button" class="link" onclick={changeCertificate}>{t("connect.checkCert")}</button>
        </div>
      {:else if !cert}
        <button type="button" class="btn" onclick={probe} disabled={probing || !server.trim()}>
          {#if probing}<span class="spinner small"></span>{t("connect.checking")}{:else}<Icon name="shield" size={16} />{t("connect.checkCert")}{/if}
        </button>
      {:else}
        <div class="fp">
          <strong>{t("connect.certTitle")} (SHA-256)</strong>
          <code class="hex">{cert.hex}</code>
          <code class="colon">{cert.colon}</code>
          <p>{t("connect.certHelp")}</p>
          <label class="field compact">
            <span>{t("connect.certCompare")}</span>
            <input type="text" bind:value={expected} spellcheck="false" autocomplete="off" />
          </label>
          {#if compare === "match"}
            <p class="ok"><Icon name="check" size={16} /> {t("connect.certMatch")}</p>
          {:else if compare === "nomatch"}
            <p class="bad"><Icon name="warning" size={16} /> {t("connect.certNoMatch")}</p>
          {/if}
          <label class="check">
            <input type="checkbox" bind:checked={trustChecked} disabled={compare === "nomatch"} />
            <span>{t("connect.trust")}</span>
          </label>
        </div>
      {/if}
    </section>

    <label class="field">
      <span>{t("connect.password")}</span>
      <input type="password" bind:value={password} autocomplete="current-password" />
    </label>

    <label class="check">
      <input type="checkbox" bind:checked={remember} />
      <span>{t("connect.remember")}</span>
    </label>

    {#if error}
      <div class="error" role="alert">
        <Icon name="warning" size={16} />
        <span>{t(`err.${error.code}`)}</span>
      </div>
    {/if}

    <button class="btn primary submit" type="submit" disabled={!canConnect}>
      {#if connecting}<span class="spinner small light"></span>{t("connect.connecting")}{:else}{t("connect.connect")}{/if}
    </button>
  </form>
</main>

<style>
  .connect {
    height: 100%;
    overflow-y: auto;
    display: grid;
    place-items: center;
    padding: 32px 16px;
    background:
      radial-gradient(1200px 600px at 20% -10%, var(--accent-softer), transparent 60%),
      radial-gradient(900px 500px at 110% 110%, var(--accent-softer), transparent 60%),
      var(--bg);
  }

  .card {
    width: min(480px, 100%);
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 28px 30px 30px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    user-select: text;
  }

  .brand {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    margin-bottom: 4px;
  }

  .brand svg {
    flex: none;
  }

  h1 {
    margin: 0 0 4px;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
  }

  .brand p {
    margin: 0;
    color: var(--text-2);
    font-size: 13px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field > span {
    font-size: 13px;
    font-weight: 500;
  }

  .field small {
    color: var(--text-3);
    font-size: 12px;
  }

  .field.compact > span {
    font-weight: 400;
    color: var(--text-2);
    font-size: 12.5px;
  }

  .check {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    font-size: 13px;
    color: var(--text-2);
    cursor: pointer;
  }

  .check input {
    margin: 2px 0 0;
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .cert {
    display: flex;
    flex-direction: column;
  }

  .trusted {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--success);
  }

  .trusted div {
    flex: 1;
    display: flex;
    flex-direction: column;
    color: var(--text);
    font-size: 13px;
  }

  code {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
    word-break: break-all;
  }

  .link {
    border: none;
    background: none;
    color: var(--accent);
    font-size: 13px;
    padding: 4px;
  }

  .link:hover {
    text-decoration: underline;
  }

  .fp {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    font-size: 13px;
  }

  .fp p {
    margin: 0;
    color: var(--text-2);
  }

  .fp .hex {
    color: var(--text);
  }

  .ok,
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
  }

  .fp .ok {
    color: var(--success);
  }

  .fp .bad {
    color: var(--danger);
  }

  .error {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    background: var(--danger-soft);
    color: var(--danger);
    font-size: 13px;
  }

  .submit {
    height: 38px;
    margin-top: 4px;
  }

  .spinner.small {
    width: 14px;
    height: 14px;
  }

  .spinner.light {
    border-color: rgba(255, 255, 255, 0.35);
    border-top-color: var(--accent-fg);
  }
</style>
