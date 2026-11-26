<script lang="ts">
  import type { RestoreJob } from "../api";
  import { app } from "../store.svelte";
  import { i18n, t } from "../i18n.svelte";
  import { dismissable } from "../popover";
  import Icon from "./Icon.svelte";

  let { anchor }: { anchor: HTMLElement | undefined } = $props();
  let root: HTMLDivElement | undefined = $state();
  let expanded = $state<Set<number>>(new Set());

  $effect(() => {
    if (!root) return;
    return dismissable(root, () => (app.restorePanelOpen = false), anchor);
  });

  // Transfer speed per job (smoothed), from successive progress updates.
  const samples = new Map<number, { t: number; b: number; speed: number }>();
  function speed(job: RestoreJob): number {
    const now = performance.now();
    const s = samples.get(job.id);
    if (!s) {
      samples.set(job.id, { t: now, b: job.bytesDone, speed: 0 });
      return 0;
    }
    if (now - s.t > 700) {
      const v = (job.bytesDone - s.b) / ((now - s.t) / 1000);
      s.speed = s.speed ? s.speed * 0.6 + v * 0.4 : v;
      s.t = now;
      s.b = job.bytesDone;
    }
    return Math.max(0, s.speed);
  }

  function title(job: RestoreJob): string {
    if (job.itemCount === 1) return t("restore.title", { n: 1, name: job.items[0] });
    const folder = job.sourcePath.split(/[\\/]/).filter(Boolean).at(-1) ?? "/";
    return t("restore.title", { n: job.itemCount, folder });
  }

  function toggle(id: number) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded = next;
  }

  const stateIcon: Record<RestoreJob["state"], string> = {
    queued: "clock",
    running: "download",
    done: "check",
    cancelled: "stop",
    failed: "warning",
  };
</script>

<div class="panel" bind:this={root} role="dialog" aria-label={t("restore.panel")}>
  <header>
    <h2>{t("restore.panel")}</h2>
    <button class="link" onclick={() => app.clearRestores()} disabled={!app.jobs.some((j) => j.state !== "queued" && j.state !== "running")}>
      {t("restore.clear")}
    </button>
    <button class="icon-btn" onclick={() => (app.restorePanelOpen = false)} aria-label={t("common.close")}><Icon name="x" size={15} /></button>
  </header>

  <div class="jobs">
    {#each app.jobs as job (job.id)}
      {@const pct = job.bytesTotal > 0 ? Math.min(100, (job.bytesDone / job.bytesTotal) * 100) : 0}
      {@const s = job.summary}
      <article class="job state-{job.state}">
        <div class="row">
          <span class="state-icon"><Icon name={stateIcon[job.state]} size={16} /></span>
          <div class="text">
            <strong title={job.items.join(", ")}>{title(job)}</strong>
            <small title={job.dest}>{t("restore.from", { time: i18n.dateTime(job.snapshotTime), dest: job.dest })}</small>
          </div>
          {#if job.state === "running" || job.state === "queued"}
            <button class="btn small" onclick={() => app.cancelRestore(job.id)}>{t("restore.cancel")}</button>
          {:else if s && s.topLevel.length > 0}
            <button class="btn small" onclick={() => app.api.openRestoreFolder(job.id)}>
              <Icon name="folder-open" size={15} />
              {t("restore.open")}
            </button>
          {/if}
        </div>

        {#if job.state === "running"}
          <div class="bar" class:indeterminate={job.phase === "scanning"} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
            <span style:width="{pct}%"></span>
          </div>
          <p class="status">
            {#if job.phase === "scanning"}
              {t("restore.scanning", { n: job.dirsScanned })}
            {:else if job.phase === "finishing"}
              {t("restore.finishing")}
            {:else}
              {t("restore.progress", { done: job.filesDone, total: job.filesTotal, bytes: i18n.size(job.bytesDone), totalBytes: i18n.size(job.bytesTotal) })}
              · {i18n.size(speed(job))}/s
            {/if}
          </p>
          {#if job.current}<p class="current" title={job.current}>{job.current}</p>{/if}
        {:else if job.state === "queued"}
          <p class="status">{t("restore.queued")}</p>
        {:else if job.state === "failed"}
          <p class="status error">{t("restore.failed", { error: job.error ?? "" })}</p>
        {:else if s}
          <p class="status">
            {job.state === "cancelled"
              ? t("restore.cancelled", { n: s.filesRestored })
              : t("restore.done", { n: s.filesRestored, size: i18n.size(s.bytesRestored) })}
          </p>
          {#if s.renamedCount || s.skippedLinksCount || s.skippedExistingCount || s.errorsCount}
            <div class="badges">
              {#if s.errorsCount}<span class="badge error">{t("restore.errors", { n: s.errorsCount })}</span>{/if}
              {#if s.renamedCount}<span class="badge">{t("restore.renamed", { n: s.renamedCount })}</span>{/if}
              {#if s.skippedLinksCount}<span class="badge">{t("restore.links", { n: s.skippedLinksCount })}</span>{/if}
              {#if s.skippedExistingCount}<span class="badge">{t("restore.existing", { n: s.skippedExistingCount })}</span>{/if}
              <button class="link" onclick={() => toggle(job.id)} aria-expanded={expanded.has(job.id)}>{t("restore.details")}</button>
            </div>
            {#if expanded.has(job.id)}
              <ul class="details">
                {#each s.errors as [path, msg] (path)}<li class="error"><code>{path}</code> — {msg}</li>{/each}
                {#each s.renamed as [from, to] (from)}<li><code>{from}</code> → <code>{to}</code></li>{/each}
                {#each s.skippedLinks as l (l)}<li><code>{l}</code></li>{/each}
                {#each s.skippedExisting as l (l)}<li><code>{l}</code></li>{/each}
                {#if s.errorsCount + s.renamedCount + s.skippedLinksCount + s.skippedExistingCount > s.errors.length + s.renamed.length + s.skippedLinks.length + s.skippedExisting.length}
                  <li class="more">
                    {t("restore.more", {
                      n:
                        s.errorsCount + s.renamedCount + s.skippedLinksCount + s.skippedExistingCount -
                        (s.errors.length + s.renamed.length + s.skippedLinks.length + s.skippedExisting.length),
                    })}
                  </li>
                {/if}
              </ul>
            {/if}
          {/if}
        {/if}
      </article>
    {:else}
      <p class="empty">{t("restore.empty")}</p>
    {/each}
  </div>
</div>

<style>
  .panel {
    position: fixed;
    right: 14px;
    bottom: 44px;
    z-index: 30;
    width: min(460px, calc(100vw - 28px));
    max-height: min(60vh, 560px);
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    animation: up 0.14s var(--ease);
    user-select: text;
  }

  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 10px 8px 16px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    flex: 1;
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .jobs {
    overflow-y: auto;
    padding: 6px;
  }

  .job {
    padding: 10px 10px 10px 8px;
    border-radius: var(--radius-sm);
  }

  .job + .job {
    border-top: 1px solid var(--border);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .state-icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--surface-3);
    color: var(--text-2);
    flex: none;
  }

  .state-running .state-icon {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .state-done .state-icon {
    background: color-mix(in srgb, var(--success) 15%, transparent);
    color: var(--success);
  }

  .state-failed .state-icon {
    background: var(--danger-soft);
    color: var(--danger);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .text strong,
  .text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .text strong {
    font-weight: 600;
    font-size: 13.5px;
  }

  .text small {
    color: var(--text-3);
    font-size: 12px;
  }

  .btn.small {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
    gap: 6px;
    flex: none;
  }

  .bar {
    position: relative;
    height: 4px;
    margin: 10px 0 0 38px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }

  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 2px;
    background: var(--accent);
    transition: width 0.25s;
  }

  .bar.indeterminate span {
    width: 30% !important;
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

  .status,
  .current {
    margin: 6px 0 0 38px;
    font-size: 12.5px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }

  .current {
    margin-top: 2px;
    color: var(--text-3);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status.error {
    color: var(--danger);
  }

  .badges {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0 38px;
  }

  .badge {
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--surface-3);
    color: var(--text-2);
    font-size: 11.5px;
  }

  .badge.error {
    background: var(--danger-soft);
    color: var(--danger);
  }

  .link {
    border: none;
    background: none;
    color: var(--accent);
    font-size: 12.5px;
    padding: 2px 4px;
  }

  .link:disabled {
    color: var(--text-3);
    cursor: default;
  }

  .link:hover:not(:disabled) {
    text-decoration: underline;
  }

  .details {
    margin: 8px 0 0 38px;
    padding: 8px 10px;
    list-style: none;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    max-height: 160px;
    overflow: auto;
    font-size: 12px;
    color: var(--text-2);
  }

  .details li {
    padding: 1px 0;
    word-break: break-all;
  }

  .details li.error {
    color: var(--danger);
  }

  .details code {
    font-family: var(--mono);
    font-size: 11.5px;
  }

  .more,
  .empty {
    color: var(--text-3);
  }

  .empty {
    margin: 0;
    padding: 18px;
    text-align: center;
    font-size: 13px;
  }
</style>
