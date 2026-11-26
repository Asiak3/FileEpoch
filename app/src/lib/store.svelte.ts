// Application state and actions.

import {
  loadApi,
  sourceKey,
  toApiError,
  type Api,
  type ApiError,
  type ColumnId,
  type ColumnSetting,
  type SavedColumn,
  type ConnectRequest,
  type ConnectResult,
  type Entry,
  type RestoreJob,
  type RestoreRequest,
  type Settings,
  type Snapshot,
  type Source,
  type ViewMode,
} from "./api";
import { i18n, resolveLocale, t } from "./i18n.svelte";
import { buildPoints, indexNearest, type TimePoint } from "./timeline";

export type SortKey = "name" | ColumnId;

/** Optional columns of the details view, in display order. */
export const DEFAULT_COLUMNS: ColumnSetting[] = [
  { id: "mtime", visible: true, width: 180 },
  { id: "type", visible: true, width: 160 },
  { id: "size", visible: true, width: 100 },
  { id: "items", visible: false, width: 90 },
  { id: "mode", visible: false, width: 110 },
  { id: "owner", visible: false, width: 110 },
];

/** How often the list of versions is reloaded while Rewind is open. */
const REFRESH_MS = 5 * 60_000;
export const MIN_COLUMN = 60;
export const MAX_COLUMN = 520;
/** The name column: narrowest width, and widest after resizing. */
export const NAME_MIN = 200;
export const NAME_MAX = 1600;
type Screen = "loading" | "connect" | "mismatch" | "main";

function normalize(s: string): string {
  return s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

class AppStore {
  api!: Api;
  isMock = false;

  screen = $state<Screen>("loading");
  settings = $state<Settings>({
    server: "",
    user: "",
    fingerprint: "",
    language: "auto",
    theme: "system",
    groupIdentical: true,
    lastSource: "",
    viewMode: "details",
    columns: [],
    restoreNaming: "suffix",
    suffixDate: "snapshot",
    conflict: "rename",
    verify: true,
    lastRestoreDir: "",
    thumbnails: true,
    tray: false,
  });

  /** Saved column settings merged over the defaults (new columns appear, unknown ones vanish). */
  columns = $derived<ColumnSetting[]>(
    DEFAULT_COLUMNS.map((d) => {
      const saved = this.settings.columns?.find((c) => c.id === d.id);
      return saved ? { ...d, visible: saved.visible, width: Math.min(MAX_COLUMN, Math.max(MIN_COLUMN, saved.width)) } : d;
    }),
  );
  visibleColumns = $derived(this.columns.filter((c) => c.visible));
  /** Width of the name column, or null while it fills the free space (until the user resizes it). */
  nameWidth = $derived.by(() => {
    const w = this.settings.columns?.find((c) => c.id === "name")?.width;
    return w ? Math.min(NAME_MAX, Math.max(NAME_MIN, w)) : null;
  });
  /** Error shown on the connect screen. */
  connectError = $state<ApiError | null>(null);
  /** Certificate presented by the server when it did not match the trusted one. */
  presentedFingerprint = $state("");
  connection = $state<ConnectResult | null>(null);
  systemDark = $state(false);

  sources = $state<Source[]>([]);
  source = $state<Source | null>(null);
  snapshots = $state.raw<Snapshot[]>([]);
  snapshotsLoading = $state(false);
  points = $derived<TimePoint[]>(buildPoints(this.snapshots, this.settings.groupIdentical));
  pointIndex = $state(-1);
  point = $derived<TimePoint | null>(this.points[this.pointIndex] ?? null);

  /** Folder the user navigated to (names below the snapshot root). Moving in time keeps it. */
  wantedPath = $state<string[]>([]);
  /** How much of wantedPath exists in the shown snapshot. */
  shownDepth = $state(0);
  shownPath = $derived(this.wantedPath.slice(0, this.shownDepth));
  entries = $state.raw<Entry[]>([]);
  listing = $state(false);
  listError = $state<ApiError | null>(null);
  private back: string[][] = [];
  private forward: string[][] = [];
  canBack = $state(false);
  canForward = $state(false);

  search = $state("");
  sortKey = $state<SortKey>("name");
  sortAsc = $state(true);
  selection = $state.raw<Set<string>>(new Set());
  focusName = $state<string | null>(null);

  /** Restore jobs, newest first. */
  jobs = $state<RestoreJob[]>([]);
  activeJobs = $derived(this.jobs.filter((j) => j.state === "queued" || j.state === "running"));
  restorePanelOpen = $state(false);
  /** Modal confirmation (overwrite). */
  confirmState = $state<{ title: string; body: string; items: string[]; yes: string; resolve: (ok: boolean) => void } | null>(
    null,
  );
  /** Short message at the bottom of the window. */
  toast = $state<string | null>(null);
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  private seq = 0;
  private loadTimer: ReturnType<typeof setTimeout> | undefined;
  /** Starts with the operating system (read from the system). */
  autostart = $state(false);
  private trayKey = "";
  private lastRefresh = 0;

  /**
   * Sort keys, computed once per folder: normalized name (accents and case removed) for search,
   * a "natural" key with zero-padded numbers (foto2 before foto10) and the extension for "Type".
   * Comparing these plain strings is ~7× faster than Intl.Collator on 100k entries.
   */
  private keyed = $derived(
    this.entries.map((e) => {
      const n = normalize(e.name);
      const dot = n.lastIndexOf(".");
      return {
        e,
        n,
        k: n.replace(/\d+/g, (d) => d.padStart(20, "0")),
        t: e.kind === "d" || dot <= 0 ? "" : n.slice(dot + 1),
        d: e.kind === "d" ? 0 : 1,
      };
    }),
  );

  /** Sorted once per folder/sort order; folders always first. */
  private sorted = $derived.by(() => {
    const sign = this.sortAsc ? 1 : -1;
    const key = this.sortKey;
    const str = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
    return this.keyed.slice().sort((a, b) => {
      if (a.d !== b.d) return a.d - b.d;
      let c = 0;
      if (key === "mtime") c = (a.e.mtime ?? 0) - (b.e.mtime ?? 0);
      else if (key === "size") c = a.e.size - b.e.size;
      else if (key === "type") c = str(a.t, b.t);
      else if (key === "items") c = (a.e.files ?? -1) - (b.e.files ?? -1);
      else if (key === "mode") c = str(a.e.mode ?? "", b.e.mode ?? "");
      else if (key === "owner") c = (a.e.uid ?? 0) - (b.e.uid ?? 0) || (a.e.gid ?? 0) - (b.e.gid ?? 0);
      return sign * (c || str(a.k, b.k));
    });
  });

  /** Sorted entries matching the search (typing only filters, it never re-sorts). */
  visible = $derived.by(() => {
    const q = normalize(this.search.trim());
    const list = q ? this.sorted.filter((x) => x.n.includes(q)) : this.sorted;
    return list.map((x) => x.e);
  });

  selectedSize = $derived.by(() => {
    let total = 0;
    for (const e of this.entries) if (this.selection.has(e.name)) total += e.size;
    return total;
  });

  resolvedTheme = $derived<"light" | "dark">(
    this.settings.theme === "system" ? (this.systemDark ? "dark" : "light") : this.settings.theme,
  );

  async init() {
    this.api = await loadApi();
    this.isMock = !("__TAURI_INTERNALS__" in window);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    this.systemDark = mq.matches;
    mq.addEventListener("change", (e) => (this.systemDark = e.matches));

    this.settings = await this.api.loadSettings();
    this.applyPreferences();
    this.autostart = await this.api.getAutostart().catch(() => false);
    await this.api.onQuitRequested(() => void this.confirmQuit());
    // New versions appear while Rewind stays open (e.g. in the tray): check every few minutes
    // and when the window comes back.
    setInterval(() => void this.refreshSnapshots(), REFRESH_MS);
    window.addEventListener("focus", () => {
      if (Date.now() - this.lastRefresh > 60_000) void this.refreshSnapshots();
    });
    await this.api.onRestoreProgress((job) => this.upsertJob(job));
    this.jobs = (await this.api.listRestores()).reverse();
    const s = this.settings;
    if (s.server && s.user && s.fingerprint) {
      await this.connect({ server: s.server, user: s.user, fingerprint: s.fingerprint, password: null, remember: false });
    } else {
      this.screen = "connect";
    }
  }

  applyPreferences() {
    i18n.locale = resolveLocale(this.settings.language);
    document.documentElement.lang = i18n.locale;
    document.documentElement.dataset.theme = this.resolvedTheme;
    void this.api.setWindowTheme(this.settings.theme === "system" ? null : this.settings.theme).catch(() => {});
    const trayKey = `${this.settings.tray}:${i18n.locale}`;
    if (trayKey !== this.trayKey) {
      this.trayKey = trayKey;
      void this.api.configureTray(this.settings.tray, i18n.locale).catch(() => {});
    }
  }

  async setAutostart(enabled: boolean) {
    try {
      this.autostart = await this.api.setAutostart(enabled);
    } catch (e) {
      this.showToast(toApiError(e).message);
    }
  }

  /** Closing or quitting while restores run. */
  async confirmQuit() {
    if (this.confirmState) return;
    const ok = await this.confirm({ title: t("quit.title"), body: t("quit.body"), items: [], yes: t("quit.yes") });
    if (ok) await this.api.quitApp();
  }

  async saveSettings(patch: Partial<Settings>) {
    this.settings = { ...this.settings, ...patch };
    this.applyPreferences();
    try {
      await this.api.saveSettings(this.settings);
    } catch {
      /* settings are a convenience; failure to save is not fatal */
    }
  }

  /** Returns true on success. Errors are stored in connectError / screen. */
  async connect(request: ConnectRequest): Promise<boolean> {
    this.connectError = null;
    try {
      this.connection = await this.api.connect(request);
    } catch (e) {
      const err = toApiError(e);
      if (err.code === "certificate_mismatch" && request.fingerprint === this.settings.fingerprint && this.settings.fingerprint) {
        this.presentedFingerprint = err.presented ?? "";
        this.screen = "mismatch";
      } else {
        this.connectError = err.code === "password_required" ? null : err;
        this.screen = "connect";
      }
      return false;
    }
    await this.saveSettings({ server: request.server, user: request.user, fingerprint: request.fingerprint });
    this.screen = "main";
    await this.loadSources();
    return true;
  }

  /** After a certificate change: forget the trusted fingerprint and go back to the connect form. */
  async retrustCertificate() {
    await this.saveSettings({ fingerprint: "" });
    this.presentedFingerprint = "";
    this.screen = "connect";
  }

  async disconnect() {
    await this.api.disconnect();
    this.connection = null;
    this.sources = [];
    this.source = null;
    this.snapshots = [];
    this.entries = [];
    this.screen = "connect";
  }

  async forgetPassword(): Promise<boolean> {
    return this.api.forgetPassword(this.settings.server, this.settings.user);
  }

  async loadSources() {
    try {
      this.sources = await this.api.listSources();
    } catch (e) {
      this.listError = toApiError(e);
      return;
    }
    const last = this.sources.find((s) => sourceKey(s) === this.settings.lastSource);
    const first = last ?? this.sources[0];
    if (first) await this.selectSource(first);
  }

  async selectSource(source: Source) {
    this.source = source;
    void this.saveSettings({ lastSource: sourceKey(source) });
    this.snapshots = [];
    this.entries = [];
    this.pointIndex = -1;
    this.wantedPath = [];
    this.shownDepth = 0;
    this.back = [];
    this.forward = [];
    this.updateHistoryFlags();
    this.search = "";
    this.selection = new Set();
    this.snapshotsLoading = true;
    this.listError = null;
    try {
      this.snapshots = await this.api.listSnapshots(source);
    } catch (e) {
      this.listError = toApiError(e);
    } finally {
      this.snapshotsLoading = false;
    }
    this.pointIndex = this.points.length - 1;
    // Development only (mock backend): start in ?path=a/b and at ?point=<index>.
    if (this.isMock) {
      const q = new URLSearchParams(location.search);
      if (q.get("path")) this.wantedPath = q.get("path")!.split("/").filter(Boolean);
      if (q.get("point")) this.pointIndex = Math.min(this.points.length - 1, Number(q.get("point")));
    }
    await this.loadListing();
  }

  /**
   * Reloads the versions of the current source. If the newest one was selected, the new newest
   * is selected; otherwise the selection stays on the same time.
   */
  async refreshSnapshots() {
    const source = this.source;
    if (this.screen !== "main" || !source || this.snapshotsLoading) return;
    this.lastRefresh = Date.now();
    let fresh: Snapshot[];
    try {
      fresh = await this.api.listSnapshots(source);
    } catch {
      return; // a passing network problem: try again next time
    }
    if (this.source !== source) return;
    const same = fresh.length === this.snapshots.length && fresh.every((s, i) => s.id === this.snapshots[i].id);
    if (same) return;
    const wasLatest = this.pointIndex === this.points.length - 1;
    const time = this.point?.time ?? Date.now();
    const root = this.point?.snapshot.rootId;
    this.snapshots = fresh;
    this.pointIndex = wasLatest ? this.points.length - 1 : indexNearest(this.points, time);
    if (this.point?.snapshot.rootId !== root) await this.loadListing();
  }

  /** Select a point on the timeline. Rapid changes (scrubbing) are coalesced. */
  setPoint(index: number, immediate = false) {
    if (index < 0 || index >= this.points.length || index === this.pointIndex) return;
    this.pointIndex = index;
    clearTimeout(this.loadTimer);
    if (immediate) void this.loadListing();
    else this.loadTimer = setTimeout(() => void this.loadListing(), 140);
  }

  setGrouping(group: boolean) {
    const time = this.point?.time ?? Date.now();
    void this.saveSettings({ groupIdentical: group });
    this.pointIndex = indexNearest(this.points, time);
  }

  async loadListing() {
    const point = this.point;
    if (!point) {
      this.entries = [];
      return;
    }
    const seq = ++this.seq;
    this.listing = true;
    this.listError = null;
    try {
      const r = await this.api.openPath(point.snapshot.rootId, this.wantedPath);
      if (seq !== this.seq) return; // a newer request superseded this one
      this.entries = r.entries;
      this.shownDepth = r.depth;
      const names = new Set(r.entries.map((e) => e.name));
      this.selection = new Set([...this.selection].filter((n) => names.has(n)));
      if (this.focusName && !names.has(this.focusName)) this.focusName = null;
    } catch (e) {
      if (seq !== this.seq) return;
      this.listError = toApiError(e);
      this.entries = [];
    } finally {
      if (seq === this.seq) this.listing = false;
    }
  }

  /** Manual navigation: replaces the wanted path. */
  navigate(path: string[], record = true) {
    if (record) {
      this.back.push(this.shownPath);
      this.forward = [];
    }
    this.wantedPath = path;
    this.search = "";
    this.selection = new Set();
    this.focusName = null;
    this.updateHistoryFlags();
    void this.loadListing();
  }

  open(entry: Entry) {
    if (entry.kind === "d") this.navigate([...this.shownPath, entry.name]);
  }

  up() {
    if (this.shownPath.length > 0) {
      const child = this.shownPath[this.shownPath.length - 1];
      this.navigate(this.shownPath.slice(0, -1));
      this.focusName = child;
      this.selection = new Set([child]);
    }
  }

  goBack() {
    const p = this.back.pop();
    if (!p) return;
    this.forward.push(this.shownPath);
    this.navigate(p, false);
  }

  goForward() {
    const p = this.forward.pop();
    if (!p) return;
    this.back.push(this.shownPath);
    this.navigate(p, false);
  }

  private updateHistoryFlags() {
    this.canBack = this.back.length > 0;
    this.canForward = this.forward.length > 0;
  }

  setViewMode(mode: ViewMode) {
    void this.saveSettings({ viewMode: mode });
  }

  setColumn(id: ColumnId, patch: Partial<ColumnSetting>) {
    const columns = this.columns.map((c) => (c.id === id ? { ...c, ...patch } : c));
    if (patch.visible === false && this.sortKey === id) this.sortKey = "name";
    void this.saveSettings({ columns: this.withName(columns, this.nameWidth) });
  }

  /** null: the name column fills the free space again. */
  setNameWidth(width: number | null) {
    void this.saveSettings({ columns: this.withName(this.columns, width) });
  }

  private withName(columns: ColumnSetting[], nameWidth: number | null): SavedColumn[] {
    return nameWidth === null ? columns : [...columns, { id: "name", visible: true, width: Math.round(nameWidth) }];
  }

  resetColumns() {
    void this.saveSettings({ columns: [] });
  }

  // --- restore -------------------------------------------------------------------------------

  private upsertJob(job: RestoreJob) {
    const i = this.jobs.findIndex((j) => j.id === job.id);
    this.jobs = i < 0 ? [job, ...this.jobs] : this.jobs.map((j) => (j.id === job.id ? job : j));
  }

  showToast(message: string) {
    clearTimeout(this.toastTimer);
    this.toast = message;
    this.toastTimer = setTimeout(() => (this.toast = null), 6000);
  }

  confirm(options: { title: string; body: string; items: string[]; yes: string }): Promise<boolean> {
    return new Promise((resolve) => {
      this.confirmState = {
        ...options,
        resolve: (ok) => {
          this.confirmState = null;
          resolve(ok);
        },
      };
    });
  }

  /**
   * Restores `entries` of the shown folder, or the shown folder itself when null.
   * The snapshot and folder are captured now: moving the timeline afterwards does not change
   * what is restored. Asks for the destination every time.
   */
  async restore(entries: Entry[] | null) {
    const point = this.point;
    const source = this.source;
    if (!point || !source) return;
    const base = {
      source,
      snapshotRoot: point.snapshot.rootId,
      snapshotTime: point.snapshot.startTime,
      path: [...this.shownPath],
      names: entries ? entries.map((e) => e.name) : [],
    };
    try {
      const dest = await this.api.chooseDestination(this.settings.lastRestoreDir || null, t("restore.pickTitle"));
      if (!dest) return;
      void this.saveSettings({ lastRestoreDir: dest });
      const s = this.settings;
      const request: RestoreRequest = {
        ...base,
        dest,
        naming: s.restoreNaming,
        suffixDate: s.suffixDate,
        label: t(s.suffixDate === "modified" ? "restore.word.modified" : "restore.word.snapshot"),
        conflict: s.conflict,
        overwriteConfirmed: false,
        verify: s.verify,
      };
      if (s.conflict === "overwrite") {
        const existing = await this.api.restoreConflicts(request);
        if (existing.length === 0) {
          request.conflict = "skip"; // nothing would be overwritten
        } else {
          const ok = await this.confirm({
            title: t("restore.confirmTitle"),
            body: t("restore.confirmBody"),
            items: existing,
            yes: t("restore.confirmYes"),
          });
          if (!ok) return;
          request.overwriteConfirmed = true;
        }
      }
      await this.api.startRestore(request);
      this.restorePanelOpen = true;
    } catch (e) {
      const err = toApiError(e);
      this.showToast(`${t(`err.${err.code}`)} ${err.code === "other" ? err.message : ""}`.trim());
    }
  }

  async cancelRestore(id: number) {
    await this.api.cancelRestore(id);
  }

  async clearRestores() {
    await this.api.clearRestores();
    this.jobs = this.jobs.filter((j) => j.state === "queued" || j.state === "running");
  }

  sortBy(key: SortKey) {
    if (this.sortKey === key) this.sortAsc = !this.sortAsc;
    else {
      this.sortKey = key;
      this.sortAsc = key === "name" || key === "type" || key === "mode" || key === "owner";
    }
  }
}

export const app = new AppStore();
