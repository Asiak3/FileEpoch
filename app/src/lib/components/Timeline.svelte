<script lang="ts">
  import { app } from "../store.svelte";
  import { i18n, t } from "../i18n.svelte";
  import Icon from "./Icon.svelte";

  const PAD = 40; // space before the first and after the last tick
  const GAP = 9; // minimum distance between ticks

  let wrap: HTMLDivElement | undefined = $state();
  let width = $state(0);
  let scrollLeft = $state(0);
  let hover = $state<number | null>(null);
  let dragging = $state(false);

  let points = $derived(app.points);
  let n = $derived(points.length);
  let index = $derived(app.pointIndex);

  /**
   * Tick positions (0 = first tick). Proportional to time so gaps are visible, but ticks closer
   * than GAP are pushed apart and the time scale is shrunk until everything fits the width.
   * Only when even evenly spaced ticks do not fit does the track become wider than the view.
   */
  function layout(times: number[], avail: number): number[] {
    const count = times.length;
    if (count === 0) return [];
    if (count === 1) return [0];
    const t0 = times[0];
    const span = times[count - 1] - t0 || 1;
    const place = (scale: number) => {
      const xs = new Array<number>(count);
      for (let i = 0; i < count; i++) {
        const raw = ((times[i] - t0) / span) * avail * scale;
        xs[i] = i === 0 ? 0 : Math.max(raw, xs[i - 1] + GAP);
      }
      return xs;
    };
    if ((count - 1) * GAP >= avail) return place(0);
    let xs = place(1);
    if (xs[count - 1] <= avail + 0.5) return xs;
    let lo = 0;
    let hi = 1;
    for (let k = 0; k < 28; k++) {
      const mid = (lo + hi) / 2;
      if (place(mid)[count - 1] > avail) hi = mid;
      else lo = mid;
    }
    return place(lo);
  }

  let positions = $derived(layout(points.map((p) => p.time), Math.max(0, width - 2 * PAD)));
  let span = $derived(n ? positions[n - 1] : 0);
  let trackWidth = $derived(Math.max(width, span + 2 * PAD));
  let offset = $derived(n === 1 ? width / 2 : PAD);

  function x(i: number): number {
    return offset + (positions[i] ?? 0);
  }

  function localDayKey(ms: number): string {
    const d = new Date(ms);
    return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  }

  // Ticks: first snapshot of each local day is "major".
  let ticks = $derived.by(() => {
    let prevDay = "";
    return points.map((p, i) => {
      const day = localDayKey(p.time);
      const major = day !== prevDay;
      prevDay = day;
      return { i, x: x(i), major, dim: !p.changed };
    });
  });

  // Labels under the ticks: days (or hours when everything is on one day), without overlapping.
  let labels = $derived.by(() => {
    const out: { x: number; text: string; strong: boolean }[] = [];
    if (n === 0) return out;
    const oneDay = localDayKey(points[0].time) === localDayKey(points[n - 1].time);
    const thisYear = new Date().getFullYear();
    let lastX = -Infinity;
    let prevKey = "";
    let prevMonth = -1;
    for (let i = 0; i < n; i++) {
      const d = new Date(points[i].time);
      const key = oneDay ? `${d.getHours()}` : localDayKey(points[i].time);
      if (key === prevKey) continue;
      prevKey = key;
      const xi = x(i);
      const monthChange = d.getMonth() !== prevMonth;
      if (xi - lastX < 74) continue;
      prevMonth = d.getMonth();
      const text = oneDay
        ? i18n.time(d)
        : i18n.date(d, {
            day: "numeric",
            month: "short",
            ...(d.getFullYear() !== thisYear ? { year: "numeric" } : {}),
          });
      out.push({ x: xi, text, strong: monthChange && !oneDay });
      lastX = xi;
    }
    return out;
  });

  function indexAt(clientX: number): number {
    if (!wrap || n === 0) return -1;
    if (n === 1) return 0;
    const rect = wrap.getBoundingClientRect();
    const px = clientX - rect.left + wrap.scrollLeft - offset;
    // Binary search for the nearest tick.
    let lo = 0;
    let hi = n - 1;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (positions[mid] < px) lo = mid;
      else hi = mid;
    }
    return Math.abs(positions[lo] - px) <= Math.abs(positions[hi] - px) ? lo : hi;
  }

  function select(i: number, immediate = false) {
    if (i >= 0) app.setPoint(i, immediate);
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0 || n === 0) return;
    dragging = true;
    wrap?.setPointerCapture(e.pointerId);
    wrap?.focus();
    select(indexAt(e.clientX));
  }

  function onpointermove(e: PointerEvent) {
    const i = indexAt(e.clientX);
    if (dragging) select(i);
    else hover = i;
  }

  function onpointerup(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    wrap?.releasePointerCapture(e.pointerId);
  }

  function onkeydown(e: KeyboardEvent) {
    if (n === 0) return;
    const keys: Record<string, number> = {
      ArrowLeft: index - 1,
      ArrowDown: index - 1,
      ArrowRight: index + 1,
      ArrowUp: index + 1,
      PageDown: index - 10,
      PageUp: index + 10,
      Home: 0,
      End: n - 1,
    };
    if (e.key in keys) {
      e.preventDefault();
      select(Math.max(0, Math.min(n - 1, keys[e.key])));
    }
  }

  // Vertical wheel scrolls the timeline horizontally.
  $effect(() => {
    const el = wrap;
    if (!el) return;
    const onwheel = (e: WheelEvent) => {
      if (Math.abs(e.deltaY) > Math.abs(e.deltaX) && el.scrollWidth > el.clientWidth) {
        e.preventDefault();
        el.scrollLeft += e.deltaY;
      }
    };
    el.addEventListener("wheel", onwheel, { passive: false });
    return () => el.removeEventListener("wheel", onwheel);
  });

  // Keep the selected point in view.
  $effect(() => {
    const el = wrap;
    const i = index;
    if (!el || i < 0 || width === 0) return;
    const xi = x(i);
    const margin = 80;
    if (xi < el.scrollLeft + margin || xi > el.scrollLeft + width - margin) {
      const target = xi - width / 2;
      // Jump for large moves (first display, Home/End); glide for small steps.
      const smooth = !dragging && Math.abs(target - el.scrollLeft) < width;
      el.scrollTo({ left: target, behavior: smooth ? "smooth" : "auto" });
    }
  });

  let point = $derived(app.point);
  let bubbleLeft = $derived(index >= 0 ? Math.max(150, Math.min(width - 150, x(index) - scrollLeft)) : 0);
  let hoverPoint = $derived(hover !== null && hover !== index ? points[hover] : null);
  let hoverLeft = $derived(hover !== null ? x(hover) - scrollLeft : 0);

  /** "434,6 GB · 215.266 archivos": the total size of the files in that version. */
  function sizeText(i: number): string {
    const s = points[i]?.snapshot;
    return s && known(i) ? t("version.size", { n: s.files, size: i18n.size(s.size) }) : "";
  }

  /** Incomplete versions (or ones without a summary) have no meaningful total. */
  function known(i: number): boolean {
    const s = points[i]?.snapshot;
    return !!s && !s.incomplete && (s.size > 0 || s.files > 0);
  }

  /** Change of the total size since the previous point ("+1,2 GB"), or "" when none. */
  function deltaText(i: number): string {
    if (i <= 0 || !known(i) || !known(i - 1)) return "";
    const d = points[i].snapshot.size - points[i - 1].snapshot.size;
    if (d === 0) return "";
    return `${d > 0 ? "+" : "−"}${i18n.size(Math.abs(d))}`;
  }

  function valueText(): string {
    if (!point) return "";
    const delta = deltaText(index);
    return [i18n.dateTime(point.time), sizeText(index), delta ? t("version.delta", { delta }) : ""].filter(Boolean).join(" · ");
  }

  let distinct = $derived(new Set(app.snapshots.map((s) => s.rootId)).size);
</script>

<section class="timeline" aria-label={t("timeline.label")}>
  <div class="head">
    <div class="title">
      <Icon name="clock" size={16} />
      {#if app.snapshotsLoading}
        <span>{t("list.loading")}</span>
      {:else}
        <span>{t("timeline.count", { n: app.snapshots.length })}</span>
        {#if app.snapshots.length > 1 && distinct < app.snapshots.length}
          <span class="muted">· {t("timeline.identical", { n: app.snapshots.length - distinct })}</span>
        {/if}
        <button
          class="refresh"
          class:spinning={app.refreshing}
          onclick={() => app.refreshSnapshots()}
          title={app.refreshedAt ? t("timeline.refreshed", { time: i18n.time(app.refreshedAt) }) : t("timeline.refresh")}
          aria-label={t("timeline.refresh")}
        >
          <Icon name="refresh" size={14} />
        </button>
      {/if}
    </div>
    <label class="switch" title={t("app.groupHint")}>
      <input
        type="checkbox"
        role="switch"
        checked={app.settings.groupIdentical}
        onchange={(e) => app.setGrouping(e.currentTarget.checked)}
      />
      <span class="knob" aria-hidden="true"></span>
      <span>{t("app.group")}</span>
    </label>
  </div>

  <div class="bar">
    <button class="icon-btn nav" onclick={() => select(index - 1, true)} disabled={index <= 0} title={t("timeline.prev")} aria-label={t("timeline.prev")}>
      <Icon name="chevron-left" />
    </button>

    <div class="viewport">
      {#if point}
        <div class="bubble" style:left="{bubbleLeft}px">
          <!-- The text is only for the eye (the slider announces valueText); the button is real. -->
          <strong aria-hidden="true">{i18n.dateTime(point.time)}</strong>
          <span aria-hidden="true">
            {i18n.relative(point.time)}
            {#if point.count > 1}
              · {t("timeline.unchangedUntil", { time: i18n.dateTime(point.last.startTime) })}
            {/if}
            {#if point.snapshot.incomplete}
              · <em>{t("timeline.incomplete")}</em>
            {/if}
          </span>
          {#if sizeText(index) || app.canShowChanges}
            <span class="size">
              {#if sizeText(index)}<span aria-hidden="true">{sizeText(index)}</span>{/if}
              {#if app.canShowChanges}
                <button
                  class="delta changes"
                  onclick={() => app.showChanges()}
                  onpointerdown={(e) => e.stopPropagation()}
                  title={t("diff.linkHint")}
                  aria-label={deltaText(index) ? `${t("diff.link")} (${deltaText(index)})` : t("diff.link")}
                >
                  {deltaText(index) ? `(${deltaText(index)})` : t("diff.link")}
                </button>
              {/if}
            </span>
          {/if}
        </div>
      {/if}
      {#if hoverPoint && !dragging}
        <div class="hover-tip" style:left="{hoverLeft}px" aria-hidden="true">{i18n.dateTime(hoverPoint.time)}{known(hover ?? -1) ? ` · ${i18n.size(hoverPoint.snapshot.size)}` : ""}</div>
      {/if}

      <div
        class="track-wrap"
        class:dragging
        bind:this={wrap}
        bind:clientWidth={width}
        onscroll={() => (scrollLeft = wrap?.scrollLeft ?? 0)}
        {onpointerdown}
        {onpointermove}
        {onpointerup}
        onpointercancel={onpointerup}
        onpointerleave={() => (hover = null)}
        {onkeydown}
        tabindex="0"
        role="slider"
        aria-label={t("timeline.label")}
        aria-valuemin={0}
        aria-valuemax={Math.max(0, n - 1)}
        aria-valuenow={Math.max(0, index)}
        aria-valuetext={valueText()}
        aria-disabled={n === 0}
      >
        <div class="track" style:width="{trackWidth}px">
          <div class="rail" style:left="{n ? x(0) : 0}px" style:width="{span}px"></div>
          {#each ticks as tk (tk.i)}
            <div class="tick" class:major={tk.major} class:dim={tk.dim} class:hovered={tk.i === hover} style:left="{tk.x}px"></div>
          {/each}
          {#if index >= 0}
            <div class="thumb" style:left="{x(index)}px"></div>
          {/if}
          {#each labels as l (l.x)}
            <div class="label" class:strong={l.strong} style:left="{l.x}px">{l.text}</div>
          {/each}
        </div>
      </div>
      {#if n === 0 && !app.snapshotsLoading}
        <div class="empty">{t("timeline.none")}</div>
      {/if}
    </div>

    <button class="icon-btn nav" onclick={() => select(index + 1, true)} disabled={index >= n - 1} title={t("timeline.next")} aria-label={t("timeline.next")}>
      <Icon name="chevron-right" />
    </button>
  </div>
</section>

<style>
  .timeline {
    padding: 10px 14px 6px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 6px 4px;
    color: var(--text-2);
    font-size: 13px;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .refresh {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-left: 2px;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: var(--text-3);
  }

  .refresh:hover {
    background: var(--hover);
    color: var(--text);
  }

  .refresh.spinning :global(svg) {
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .muted {
    color: var(--text-3);
  }

  .switch {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    cursor: pointer;
    position: relative;
  }

  .switch input {
    position: absolute;
    opacity: 0;
    width: 36px;
    height: 18px;
    margin: 0;
    cursor: pointer;
  }

  .knob {
    width: 36px;
    height: 18px;
    border-radius: 9px;
    border: 1px solid var(--text-3);
    position: relative;
    transition: background 0.15s var(--ease);
  }

  .knob::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text-2);
    transition: transform 0.15s var(--ease);
  }

  .switch input:checked + .knob {
    background: var(--accent);
    border-color: var(--accent);
  }

  .switch input:checked + .knob::after {
    transform: translateX(18px);
    background: var(--accent-fg);
  }

  .switch input:focus-visible + .knob {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }

  .bar {
    display: flex;
    align-items: flex-end;
    gap: 4px;
  }

  .nav {
    margin-bottom: 28px;
  }

  .viewport {
    position: relative;
    flex: 1;
    min-width: 0;
    padding-top: 66px;
  }

  .bubble {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 5px 12px 6px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    white-space: nowrap;
    font-size: 12px;
    color: var(--text-2);
    pointer-events: none;
    transition: left 0.12s var(--ease);
    z-index: 2;
  }

  .bubble strong {
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .bubble .size {
    font-variant-numeric: tabular-nums;
  }

  .bubble .delta {
    margin-left: 5px;
    color: var(--text-3);
  }

  /* The only part of the bubble that takes the mouse: it opens the list of changes. */
  .bubble button.changes {
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    pointer-events: auto;
  }

  .bubble button.changes:hover {
    filter: brightness(1.15);
  }

  .bubble em {
    color: var(--danger);
    font-style: normal;
  }

  .hover-tip {
    position: absolute;
    top: 68px;
    transform: translate(-50%, -100%);
    padding: 2px 7px;
    border-radius: 4px;
    background: var(--text);
    color: var(--bg);
    font-size: 11px;
    white-space: nowrap;
    pointer-events: none;
    z-index: 3;
  }

  .track-wrap {
    position: relative;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    cursor: pointer;
    touch-action: pan-y;
    border-radius: var(--radius-sm);
  }

  .track-wrap::-webkit-scrollbar {
    display: none;
  }

  .track-wrap.dragging {
    cursor: grabbing;
  }

  .track {
    position: relative;
    height: 62px;
  }

  .rail {
    position: absolute;
    top: 21px;
    height: 2px;
    border-radius: 1px;
    background: var(--border);
  }

  .tick {
    position: absolute;
    top: 14px;
    width: 2px;
    height: 16px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--tick);
    transition: background 0.1s;
  }

  .tick.major {
    top: 10px;
    height: 24px;
    background: var(--tick-major);
  }

  .tick.dim {
    opacity: 0.45;
  }

  .tick.hovered {
    background: var(--accent);
    opacity: 1;
  }

  .thumb {
    position: absolute;
    top: 5px;
    width: 8px;
    height: 34px;
    margin-left: -4px;
    border-radius: 4px;
    background: var(--accent);
    box-shadow:
      0 0 0 3px var(--bg-elev),
      0 2px 8px var(--accent-soft);
    transition: left 0.12s var(--ease);
  }

  .track-wrap:focus-visible {
    outline-offset: -2px;
  }

  .label {
    position: absolute;
    top: 40px;
    transform: translateX(-50%);
    font-size: 11.5px;
    color: var(--text-3);
    white-space: nowrap;
    pointer-events: none;
  }

  .label.strong {
    color: var(--text-2);
    font-weight: 600;
  }

  /* --- "glass" style (default): frosted, rounded surfaces in the spirit of macOS ----------- */
  :global(.glass) .timeline {
    --g-surface: rgba(255, 255, 255, 0.52);
    --g-strong: rgba(255, 255, 255, 0.78);
    --g-edge: rgba(255, 255, 255, 0.75);
    --g-line: rgba(60, 60, 67, 0.16);
    --g-tick: rgba(60, 60, 67, 0.28);
    --g-tick-major: rgba(60, 60, 67, 0.55);
    --g-shadow: 0 8px 28px rgba(15, 23, 42, 0.12), 0 1px 3px rgba(15, 23, 42, 0.07);
  }

  :global([data-theme="dark"] .glass) .timeline {
    --g-surface: rgba(255, 255, 255, 0.07);
    --g-strong: rgba(48, 52, 62, 0.62);
    --g-edge: rgba(255, 255, 255, 0.14);
    --g-line: rgba(235, 235, 245, 0.14);
    --g-tick: rgba(235, 235, 245, 0.26);
    --g-tick-major: rgba(235, 235, 245, 0.5);
    --g-shadow: 0 10px 30px rgba(0, 0, 0, 0.45), 0 1px 3px rgba(0, 0, 0, 0.3);
  }

  :global(.glass) .track-wrap {
    border-radius: 16px;
    background: var(--g-surface);
    border: 1px solid var(--g-edge);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.35),
      0 1px 2px rgba(0, 0, 0, 0.04);
    backdrop-filter: blur(22px) saturate(180%);
  }

  :global(.glass) .rail {
    height: 3px;
    border-radius: 2px;
    background: var(--g-line);
  }

  :global(.glass) .tick {
    width: 2px;
    border-radius: 1px;
    background: var(--g-tick);
  }

  :global(.glass) .tick.major {
    background: var(--g-tick-major);
  }

  :global(.glass) .tick.hovered {
    background: var(--accent);
  }

  /* A white capsule knob, like a macOS slider. */
  :global(.glass) .thumb {
    top: 6px;
    width: 14px;
    height: 32px;
    margin-left: -7px;
    border-radius: 7px;
    background: linear-gradient(#ffffff, #eef1f6);
    box-shadow:
      0 0 0 0.5px rgba(0, 0, 0, 0.18),
      0 2px 6px rgba(0, 0, 0, 0.22),
      0 6px 16px rgba(15, 23, 42, 0.14);
  }

  :global(.glass) .thumb::after {
    content: "";
    position: absolute;
    left: 5px;
    right: 5px;
    top: 9px;
    bottom: 9px;
    border-radius: 2px;
    background: var(--accent);
  }

  :global(.glass) .bubble {
    padding: 7px 14px 8px;
    border-radius: 14px;
    background: var(--g-strong);
    border: 1px solid var(--g-edge);
    box-shadow: var(--g-shadow);
    backdrop-filter: blur(24px) saturate(180%);
  }

  :global(.glass) .hover-tip {
    padding: 3px 9px;
    border-radius: 8px;
    background: rgba(28, 28, 32, 0.72);
    color: #fff;
    backdrop-filter: blur(16px);
  }

  :global(.glass) .nav {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--g-surface);
    border: 1px solid var(--g-edge);
    backdrop-filter: blur(16px) saturate(180%);
  }

  :global(.glass) .knob {
    width: 38px;
    height: 22px;
    border-radius: 11px;
    border: none;
    background: rgba(120, 120, 128, 0.32);
  }

  :global(.glass) .knob::after {
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  }

  :global(.glass) .switch input {
    width: 38px;
    height: 22px;
  }

  :global(.glass) .switch input:checked + .knob {
    background: var(--accent);
  }

  :global(.glass) .switch input:checked + .knob::after {
    transform: translateX(16px);
    background: #fff;
  }

  .empty {
    position: absolute;
    inset: 50px 0 0;
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
</style>
