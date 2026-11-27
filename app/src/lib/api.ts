// Interface between the UI and the Rust backend (src-tauri/src/commands.rs).
// Outside Tauri (plain browser during development) a mock backend is loaded instead.

export type Language = "auto" | "es" | "de" | "en";
export type Theme = "system" | "light" | "dark";

export type ViewMode = "details" | "list" | "icons" | "tiles";
export type ColumnId = "mtime" | "type" | "size" | "mode" | "owner" | "items";

export interface ColumnSetting {
  id: ColumnId;
  visible: boolean;
  width: number;
}

/** As saved: also "name", with the width of the name column once the user has resized it. */
export interface SavedColumn {
  id: ColumnId | "name";
  visible: boolean;
  width: number;
}

export interface Settings {
  server: string;
  user: string;
  fingerprint: string;
  language: Language;
  theme: Theme;
  groupIdentical: boolean;
  lastSource: string;
  viewMode: ViewMode;
  columns: SavedColumn[];
  restoreNaming: "suffix" | "original";
  suffixDate: "snapshot" | "modified";
  conflict: "rename" | "skip" | "overwrite";
  verify: boolean;
  lastRestoreDir: string;
  /** Image thumbnails in the icon views (the images are downloaded for that). */
  thumbnails: boolean;
  /** Keep running in the system tray when the window is closed. */
  tray: boolean;
  /** Look of the timeline. */
  timeline: "glass" | "simple";
  /** Check GitHub for a newer version at start and once a day. */
  autoUpdate: boolean;
}

export interface UpdateInfo {
  version: string;
  current: string;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface Source {
  host: string;
  userName: string;
  path: string;
}

export interface Snapshot {
  id: string;
  /** RFC 3339, UTC. */
  startTime: string;
  endTime: string | null;
  rootId: string;
  incomplete: string | null;
  files: number;
  size: number;
  retention: string[];
  pins: string[];
  description: string;
}

export type EntryKind = "f" | "d" | "s" | "?";

export interface Entry {
  name: string;
  kind: EntryKind;
  size: number;
  /** Milliseconds since the epoch (UTC). */
  mtime: number | null;
  obj: string;
  files?: number;
  /** Octal permissions such as "0644". */
  mode?: string;
  uid?: number;
  gid?: number;
}

export interface Listing {
  /** How many components of the requested path exist in this snapshot. */
  depth: number;
  entries: Entry[];
}

/** One file that changed between two versions, or a whole folder that appeared or disappeared. */
export interface FileChange {
  /** Below the snapshot root, components separated by "/". */
  path: string;
  kind: "added" | "removed" | "modified";
  isDir: boolean;
  sizeBefore: number;
  sizeAfter: number;
  /** 1 for a file, the number of files inside for a folder. */
  files: number;
}

/** Content changes between two versions (dates and permissions alone do not count). */
export interface VersionDiff {
  /** Biggest change first. */
  changes: FileChange[];
  addedFiles: number;
  removedFiles: number;
  modifiedFiles: number;
  /** Newer total minus older total of the changed items. */
  netBytes: number;
  /** The comparison stopped at its limits: the figures only cover part of the changes. */
  truncated: boolean;
}

export interface CertInfo {
  hex: string;
  colon: string;
}

export interface ConnectRequest {
  server: string;
  user: string;
  fingerprint: string;
  /** null: use the password saved in the system credential store. */
  password: string | null;
  remember: boolean;
}

export interface ConnectResult {
  version: string | null;
  testedVersions: string[];
  untestedVersion: boolean;
  readonly: boolean;
  description: string;
}

export type ErrorCode =
  | "invalid_server"
  | "invalid_fingerprint"
  | "certificate_mismatch"
  | "connection"
  | "access_denied"
  | "not_ui_user"
  | "session"
  | "server_not_connected"
  | "not_found"
  | "incompatible"
  | "credential_store"
  | "password_required"
  | "update_check"
  | "update_install"
  | "restore_running"
  | "saved_password_rejected"
  | "not_connected_app"
  | "other";

export interface ApiError {
  code: ErrorCode;
  message: string;
  presented?: string;
}

export interface RestoreRequest {
  source: Source;
  snapshotRoot: string;
  snapshotTime: string;
  /** Folder shown (names below the snapshot root). */
  path: string[];
  /** Selected names in that folder; empty = the folder itself. */
  names: string[];
  dest: string;
  naming: Settings["restoreNaming"];
  suffixDate: Settings["suffixDate"];
  label: string;
  conflict: Settings["conflict"];
  overwriteConfirmed: boolean;
  verify: boolean;
}

export interface RestoreSummary {
  topLevel: string[];
  filesRestored: number;
  bytesRestored: number;
  dirsCreated: number;
  renamed: [string, string][];
  renamedCount: number;
  skippedLinks: string[];
  skippedLinksCount: number;
  skippedExisting: string[];
  skippedExistingCount: number;
  errors: [string, string][];
  errorsCount: number;
}

export interface RestoreJob {
  id: number;
  state: "queued" | "running" | "done" | "cancelled" | "failed";
  phase: "scanning" | "downloading" | "finishing";
  sourcePath: string;
  snapshotTime: string;
  items: string[];
  itemCount: number;
  dest: string;
  dirsScanned: number;
  filesTotal: number;
  filesDone: number;
  bytesTotal: number;
  bytesDone: number;
  current: string;
  summary: RestoreSummary | null;
  error: string | null;
}

export interface Api {
  loadSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<void>;
  probeCertificate(server: string): Promise<CertInfo>;
  connect(request: ConnectRequest): Promise<ConnectResult>;
  disconnect(): Promise<void>;
  forgetPassword(server: string, user: string): Promise<boolean>;
  listSources(): Promise<Source[]>;
  listSnapshots(source: Source): Promise<Snapshot[]>;
  openPath(root: string, path: string[]): Promise<Listing>;
  /** What changed from the version with root `oldRoot` to the one with root `newRoot`. */
  diffSnapshots(oldRoot: string, newRoot: string): Promise<VersionDiff>;
  setWindowTheme(theme: "light" | "dark" | null): Promise<void>;
  chooseDestination(start: string | null, title: string): Promise<string | null>;
  restoreConflicts(request: RestoreRequest): Promise<string[]>;
  startRestore(request: RestoreRequest): Promise<number>;
  listRestores(): Promise<RestoreJob[]>;
  cancelRestore(id: number): Promise<void>;
  clearRestores(): Promise<void>;
  openRestoreFolder(id: number): Promise<void>;
  /** Small data URL of an image file, or null when it cannot be shown. */
  thumbnail(oid: string): Promise<string | null>;
  /** Shows or removes the tray icon; `locale` translates its menu. */
  configureTray(enabled: boolean, locale: string): Promise<void>;
  /** Whether FileEpoch starts with the operating system (read from the system, not from settings). */
  getAutostart(): Promise<boolean>;
  setAutostart(enabled: boolean): Promise<boolean>;
  /** Cancels restores and quits (after the user confirmed). */
  quitApp(): Promise<void>;
  openProjectPage(): Promise<void>;
  /** Opens the license notices of FileEpoch and its components in a separate window. */
  openLicenses(): Promise<void>;
  /** Closing/quitting while restores run: the UI asks first. */
  onQuitRequested(callback: () => void): Promise<() => void>;
  /** Newer version on GitHub, or null when up to date. */
  checkUpdate(): Promise<UpdateInfo | null>;
  /** Downloads and installs the update found by checkUpdate, then restarts. */
  installUpdate(): Promise<void>;
  openReleasePage(version: string): Promise<void>;
  onUpdateProgress(callback: (p: UpdateProgress) => void): Promise<() => void>;
  /** Subscribes to job updates; returns an unsubscribe function. */
  onRestoreProgress(callback: (job: RestoreJob) => void): Promise<() => void>;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function toApiError(e: unknown): ApiError {
  if (isApiError(e)) return e;
  return { code: "other", message: e instanceof Error ? e.message : String(e) };
}

export function sourceKey(s: Source): string {
  return `${s.userName}@${s.host}:${s.path}`;
}

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { listen } = await import("@tauri-apps/api/event");
  return {
    loadSettings: () => invoke("load_settings"),
    saveSettings: (settings) => invoke("save_settings", { settings }),
    probeCertificate: (server) => invoke("probe_certificate", { server }),
    connect: (request) => invoke("connect", { request }),
    disconnect: () => invoke("disconnect"),
    forgetPassword: (server, user) => invoke("forget_password", { server, user }),
    listSources: () => invoke("list_sources"),
    listSnapshots: (source) => invoke("list_snapshots", { source }),
    openPath: (root, path) => invoke("open_path", { root, path }),
    diffSnapshots: (oldRoot, newRoot) => invoke("diff_snapshots", { oldRoot, newRoot }),
    setWindowTheme: (theme) => getCurrentWindow().setTheme(theme),
    chooseDestination: (start, title) => invoke("choose_destination", { start, title }),
    restoreConflicts: (request) => invoke("restore_conflicts", { request }),
    startRestore: (request) => invoke("start_restore", { request }),
    listRestores: () => invoke("list_restores"),
    cancelRestore: (id) => invoke("cancel_restore", { id }),
    clearRestores: () => invoke("clear_restores"),
    openRestoreFolder: (id) => invoke("open_restore_folder", { id }),
    thumbnail: (oid) => invoke("thumbnail", { oid }),
    configureTray: (enabled, locale) => invoke("configure_tray", { enabled, locale }),
    getAutostart: () => invoke("get_autostart"),
    setAutostart: (enabled) => invoke("set_autostart", { enabled }),
    quitApp: () => invoke("quit_app"),
    openProjectPage: () => invoke("open_project_page"),
    openLicenses: () => invoke("open_licenses"),
    onQuitRequested: (callback) => listen("quit-requested", () => callback()),
    checkUpdate: () => invoke("check_update"),
    installUpdate: () => invoke("install_update"),
    openReleasePage: (version) => invoke("open_release_page", { version }),
    onUpdateProgress: (callback) => listen<UpdateProgress>("update-progress", (e) => callback(e.payload)),
    onRestoreProgress: (callback) => listen<RestoreJob>("restore-progress", (e) => callback(e.payload)),
  };
}

export async function loadApi(): Promise<Api> {
  if ("__TAURI_INTERNALS__" in window) return tauriApi();
  const { mockApi } = await import("./mock");
  return mockApi();
}
