// Translations (English, Spanish, German) and locale-aware formatting.
// All dates are shown in the user's local time zone via Intl; UTC never reaches the screen.

import type { Language } from "./api";

type Plural = { one: string; other: string };
type Dict = Record<string, string | Plural>;

const en: Dict = {
  "view.label": "View",
  "view.details": "Details",
  "view.list": "List",
  "view.icons": "Large icons",
  "view.tiles": "Tiles",
  "col.mode": "Permissions",
  "col.owner": "Owner",
  "col.items": "Files",
  "col.reset": "Reset columns",
  "col.resize": "Resize column",
  "menu.open": "Open",
  "menu.copyPath": "Copy path",
  "menu.selectAll": "Select all",
  "menu.columns": "Columns",
  "connect.title": "Connect to a Kopia server",
  "connect.subtitle": "Rewind only reads from the server. Restored files are written to this computer.",
  "connect.server": "Server address",
  "connect.user": "User",
  "connect.userHint": "The user the Kopia server was started with (--server-username)",
  "connect.checkCert": "Check certificate",
  "connect.checking": "Checking…",
  "connect.certTitle": "Server certificate",
  "connect.certHelp":
    "Compare this fingerprint with the line SERVER CERT SHA256 that the Kopia server prints when it starts. Continue only if they match.",
  "connect.certCompare": "Expected fingerprint (optional, to compare automatically)",
  "connect.certMatch": "The fingerprints match",
  "connect.certNoMatch": "The fingerprints do NOT match",
  "connect.trust": "I have checked it and trust this certificate",
  "connect.trusted": "Trusted certificate",
  "connect.password": "Password",
  "connect.remember": "Remember the password in this computer's credential store",
  "connect.connect": "Connect",
  "connect.connecting": "Connecting…",
  "connect.otherServer": "Use another server",
  "connect.restart": "Start again",
  "mismatch.title": "The server certificate has changed",
  "mismatch.body":
    "The server presented a different certificate from the one you trusted. The certificate may have been renewed, or someone may be intercepting the connection. Rewind will not connect.",
  "mismatch.trusted": "Trusted",
  "mismatch.presented": "Presented now",
  "mismatch.reset": "Check the new certificate…",
  "err.invalid_server": "Enter an address like https://server:51515.",
  "err.invalid_fingerprint": "The fingerprint must have 64 hexadecimal characters.",
  "err.certificate_mismatch": "The server certificate does not match the trusted fingerprint.",
  "err.connection": "Cannot reach the server.",
  "err.access_denied": "Wrong user name or password.",
  "err.not_ui_user":
    "This user may not use the Kopia web interface. It must be the user the server was started with (--server-username).",
  "err.session": "The server rejected the session. Try again.",
  "err.server_not_connected": "The Kopia server has no repository open.",
  "err.not_found": "Not found.",
  "err.incompatible": "The server answered in an unexpected way. It may be an untested Kopia version.",
  "err.credential_store": "The system credential store could not be used.",
  "err.password_required": "Enter the password.",
  "err.saved_password_rejected": "The saved password was rejected. Enter it again.",
  "err.not_connected_app": "Not connected.",
  "err.other": "Unexpected error.",
  "app.source": "Backup source",
  "app.group": "Group identical",
  "app.groupHint": "Show consecutive snapshots with identical content as one point",
  "app.settings": "Settings",
  "app.theme": "Theme",
  "app.language": "Language",
  "app.disconnect": "Disconnect",
  "app.forget": "Forget saved password",
  "app.forgotten": "Saved password deleted.",
  "app.server": "Server",
  "app.readonly": "read-only",
  "theme.system": "System",
  "theme.light": "Light",
  "theme.dark": "Dark",
  "lang.auto": "System language",
  "banner.notReadonly":
    "This server can modify the repository (it was not started with --readonly). Rewind only reads, but check that this is not your administration server.",
  "banner.untested": "Kopia {version} has not been tested with Rewind (tested: {tested}). Some things may not work.",
  "timeline.label": "Snapshot",
  "timeline.prev": "Earlier snapshot",
  "timeline.next": "Later snapshot",
  "timeline.unchangedUntil": "unchanged until {time}",
  "timeline.identical": { one: "{n} identical snapshot", other: "{n} identical snapshots" },
  "timeline.count": { one: "{n} snapshot", other: "{n} snapshots" },
  "timeline.incomplete": "incomplete",
  "timeline.none": "This source has no snapshots yet.",
  "timeline.latest": "Latest",
  "timeline.earliest": "Earliest",
  "nav.back": "Back",
  "nav.forward": "Forward",
  "nav.up": "Up one level",
  "nav.search": "Search in this folder",
  "nav.clearSearch": "Clear search",
  "list.name": "Name",
  "list.modified": "Date modified",
  "list.type": "Type",
  "list.size": "Size",
  "list.folder": "Folder",
  "list.link": "Symbolic link",
  "list.unknown": "Unknown",
  "list.empty": "This folder is empty.",
  "list.noMatch": "No items match “{q}”.",
  "list.loading": "Loading…",
  "list.items": { one: "{n} item", other: "{n} items" },
  "list.selected": { one: "{n} selected ({size})", other: "{n} selected ({size})" },
  "list.snapshotOf": "Snapshot of {time}",
  "list.files": { one: "{n} file", other: "{n} files" },
  "notice.missing": "“{name}” does not exist in this snapshot. Showing “{shown}”.",
  "common.retry": "Try again",
  "common.close": "Close",
  "type.pdf": "PDF document",
  "type.doc": "Document",
  "type.sheet": "Spreadsheet",
  "type.slides": "Presentation",
  "type.image": "Image",
  "type.video": "Video",
  "type.audio": "Audio",
  "type.archive": "Archive",
  "type.text": "Text document",
  "type.code": "Source code",
  "type.disk": "Disk image",
  "type.app": "Program",
  "type.ext": "{ext} file",
  "type.file": "File",
};

const es: Dict = {
  "view.label": "Vista",
  "view.details": "Detalles",
  "view.list": "Lista",
  "view.icons": "Iconos grandes",
  "view.tiles": "Mosaico",
  "col.mode": "Permisos",
  "col.owner": "Propietario",
  "col.items": "Archivos",
  "col.reset": "Restablecer columnas",
  "col.resize": "Cambiar ancho de la columna",
  "menu.open": "Abrir",
  "menu.copyPath": "Copiar ruta",
  "menu.selectAll": "Seleccionar todo",
  "menu.columns": "Columnas",
  "connect.title": "Conectar con un servidor Kopia",
  "connect.subtitle": "Rewind solo lee del servidor. Los archivos restaurados se guardan en este equipo.",
  "connect.server": "Dirección del servidor",
  "connect.user": "Usuario",
  "connect.userHint": "El usuario con el que se inició el servidor Kopia (--server-username)",
  "connect.checkCert": "Comprobar certificado",
  "connect.checking": "Comprobando…",
  "connect.certTitle": "Certificado del servidor",
  "connect.certHelp":
    "Compara esta huella con la línea SERVER CERT SHA256 que muestra el servidor Kopia al arrancar. Continúa solo si coinciden.",
  "connect.certCompare": "Huella esperada (opcional, para compararla automáticamente)",
  "connect.certMatch": "Las huellas coinciden",
  "connect.certNoMatch": "Las huellas NO coinciden",
  "connect.trust": "La he comprobado y confío en este certificado",
  "connect.trusted": "Certificado de confianza",
  "connect.password": "Contraseña",
  "connect.remember": "Recordar la contraseña en el almacén de credenciales de este equipo",
  "connect.connect": "Conectar",
  "connect.connecting": "Conectando…",
  "connect.otherServer": "Usar otro servidor",
  "connect.restart": "Empezar de nuevo",
  "mismatch.title": "El certificado del servidor ha cambiado",
  "mismatch.body":
    "El servidor presenta un certificado distinto del que marcaste como de confianza. Puede que se haya renovado o que alguien esté interceptando la conexión. Rewind no se conectará.",
  "mismatch.trusted": "De confianza",
  "mismatch.presented": "Presentado ahora",
  "mismatch.reset": "Comprobar el certificado nuevo…",
  "err.invalid_server": "Escribe una dirección como https://servidor:51515.",
  "err.invalid_fingerprint": "La huella debe tener 64 caracteres hexadecimales.",
  "err.certificate_mismatch": "El certificado del servidor no coincide con la huella de confianza.",
  "err.connection": "No se puede contactar con el servidor.",
  "err.access_denied": "Usuario o contraseña incorrectos.",
  "err.not_ui_user":
    "Este usuario no puede usar la interfaz web de Kopia. Debe ser el usuario con el que se inició el servidor (--server-username).",
  "err.session": "El servidor rechazó la sesión. Inténtalo de nuevo.",
  "err.server_not_connected": "El servidor Kopia no tiene ningún repositorio abierto.",
  "err.not_found": "No encontrado.",
  "err.incompatible": "El servidor respondió de forma inesperada. Puede que sea una versión de Kopia no probada.",
  "err.credential_store": "No se pudo usar el almacén de credenciales del sistema.",
  "err.password_required": "Escribe la contraseña.",
  "err.saved_password_rejected": "La contraseña guardada no es válida. Escríbela de nuevo.",
  "err.not_connected_app": "No conectado.",
  "err.other": "Error inesperado.",
  "app.source": "Origen de la copia",
  "app.group": "Agrupar idénticas",
  "app.groupHint": "Mostrar como un solo punto las copias seguidas con el mismo contenido",
  "app.settings": "Ajustes",
  "app.theme": "Tema",
  "app.language": "Idioma",
  "app.disconnect": "Desconectar",
  "app.forget": "Olvidar la contraseña guardada",
  "app.forgotten": "Contraseña guardada eliminada.",
  "app.server": "Servidor",
  "app.readonly": "solo lectura",
  "theme.system": "Sistema",
  "theme.light": "Claro",
  "theme.dark": "Oscuro",
  "lang.auto": "Idioma del sistema",
  "banner.notReadonly":
    "Este servidor puede modificar el repositorio (no se inició con --readonly). Rewind solo lee, pero comprueba que no sea tu servidor de administración.",
  "banner.untested": "Kopia {version} no se ha probado con Rewind (probadas: {tested}). Puede que algo no funcione.",
  "timeline.label": "Copia",
  "timeline.prev": "Copia anterior",
  "timeline.next": "Copia siguiente",
  "timeline.unchangedUntil": "sin cambios hasta {time}",
  "timeline.identical": { one: "{n} copia idéntica", other: "{n} copias idénticas" },
  "timeline.count": { one: "{n} copia", other: "{n} copias" },
  "timeline.incomplete": "incompleta",
  "timeline.none": "Este origen todavía no tiene copias.",
  "timeline.latest": "Última",
  "timeline.earliest": "Primera",
  "nav.back": "Atrás",
  "nav.forward": "Adelante",
  "nav.up": "Subir un nivel",
  "nav.search": "Buscar en esta carpeta",
  "nav.clearSearch": "Borrar búsqueda",
  "list.name": "Nombre",
  "list.modified": "Fecha de modificación",
  "list.type": "Tipo",
  "list.size": "Tamaño",
  "list.folder": "Carpeta",
  "list.link": "Enlace simbólico",
  "list.unknown": "Desconocido",
  "list.empty": "Esta carpeta está vacía.",
  "list.noMatch": "Ningún elemento coincide con «{q}».",
  "list.loading": "Cargando…",
  "list.items": { one: "{n} elemento", other: "{n} elementos" },
  "list.selected": { one: "{n} seleccionado ({size})", other: "{n} seleccionados ({size})" },
  "list.snapshotOf": "Copia del {time}",
  "list.files": { one: "{n} archivo", other: "{n} archivos" },
  "notice.missing": "«{name}» no existe en esta copia. Se muestra «{shown}».",
  "common.retry": "Reintentar",
  "common.close": "Cerrar",
  "type.pdf": "Documento PDF",
  "type.doc": "Documento",
  "type.sheet": "Hoja de cálculo",
  "type.slides": "Presentación",
  "type.image": "Imagen",
  "type.video": "Vídeo",
  "type.audio": "Audio",
  "type.archive": "Archivo comprimido",
  "type.text": "Documento de texto",
  "type.code": "Código fuente",
  "type.disk": "Imagen de disco",
  "type.app": "Programa",
  "type.ext": "Archivo {ext}",
  "type.file": "Archivo",
};

const de: Dict = {
  "view.label": "Ansicht",
  "view.details": "Details",
  "view.list": "Liste",
  "view.icons": "Große Symbole",
  "view.tiles": "Kacheln",
  "col.mode": "Berechtigungen",
  "col.owner": "Besitzer",
  "col.items": "Dateien",
  "col.reset": "Spalten zurücksetzen",
  "col.resize": "Spaltenbreite ändern",
  "menu.open": "Öffnen",
  "menu.copyPath": "Pfad kopieren",
  "menu.selectAll": "Alles auswählen",
  "menu.columns": "Spalten",
  "connect.title": "Mit einem Kopia-Server verbinden",
  "connect.subtitle": "Rewind liest nur vom Server. Wiederhergestellte Dateien werden auf diesem Computer gespeichert.",
  "connect.server": "Serveradresse",
  "connect.user": "Benutzer",
  "connect.userHint": "Der Benutzer, mit dem der Kopia-Server gestartet wurde (--server-username)",
  "connect.checkCert": "Zertifikat prüfen",
  "connect.checking": "Wird geprüft…",
  "connect.certTitle": "Serverzertifikat",
  "connect.certHelp":
    "Vergleiche diesen Fingerabdruck mit der Zeile SERVER CERT SHA256, die der Kopia-Server beim Start ausgibt. Fahre nur fort, wenn sie übereinstimmen.",
  "connect.certCompare": "Erwarteter Fingerabdruck (optional, zum automatischen Vergleich)",
  "connect.certMatch": "Die Fingerabdrücke stimmen überein",
  "connect.certNoMatch": "Die Fingerabdrücke stimmen NICHT überein",
  "connect.trust": "Ich habe es geprüft und vertraue diesem Zertifikat",
  "connect.trusted": "Vertrauenswürdiges Zertifikat",
  "connect.password": "Passwort",
  "connect.remember": "Passwort im Anmeldeinformationsspeicher dieses Computers speichern",
  "connect.connect": "Verbinden",
  "connect.connecting": "Verbinde…",
  "connect.otherServer": "Anderen Server verwenden",
  "connect.restart": "Neu beginnen",
  "mismatch.title": "Das Serverzertifikat hat sich geändert",
  "mismatch.body":
    "Der Server zeigt ein anderes Zertifikat als das, dem du vertraut hast. Es wurde vielleicht erneuert, oder jemand fängt die Verbindung ab. Rewind verbindet sich nicht.",
  "mismatch.trusted": "Vertraut",
  "mismatch.presented": "Jetzt vorgelegt",
  "mismatch.reset": "Neues Zertifikat prüfen…",
  "err.invalid_server": "Gib eine Adresse wie https://server:51515 ein.",
  "err.invalid_fingerprint": "Der Fingerabdruck muss aus 64 Hexadezimalzeichen bestehen.",
  "err.certificate_mismatch": "Das Serverzertifikat passt nicht zum vertrauenswürdigen Fingerabdruck.",
  "err.connection": "Der Server ist nicht erreichbar.",
  "err.access_denied": "Falscher Benutzername oder falsches Passwort.",
  "err.not_ui_user":
    "Dieser Benutzer darf die Kopia-Weboberfläche nicht verwenden. Es muss der Benutzer sein, mit dem der Server gestartet wurde (--server-username).",
  "err.session": "Der Server hat die Sitzung abgelehnt. Versuche es erneut.",
  "err.server_not_connected": "Der Kopia-Server hat kein Repository geöffnet.",
  "err.not_found": "Nicht gefunden.",
  "err.incompatible":
    "Der Server hat unerwartet geantwortet. Vielleicht ist es eine nicht getestete Kopia-Version.",
  "err.credential_store": "Der Anmeldeinformationsspeicher des Systems konnte nicht verwendet werden.",
  "err.password_required": "Gib das Passwort ein.",
  "err.saved_password_rejected": "Das gespeicherte Passwort wurde abgelehnt. Gib es erneut ein.",
  "err.not_connected_app": "Nicht verbunden.",
  "err.other": "Unerwarteter Fehler.",
  "app.source": "Sicherungsquelle",
  "app.group": "Identische gruppieren",
  "app.groupHint": "Aufeinanderfolgende Sicherungen mit gleichem Inhalt als einen Punkt anzeigen",
  "app.settings": "Einstellungen",
  "app.theme": "Design",
  "app.language": "Sprache",
  "app.disconnect": "Trennen",
  "app.forget": "Gespeichertes Passwort vergessen",
  "app.forgotten": "Gespeichertes Passwort gelöscht.",
  "app.server": "Server",
  "app.readonly": "schreibgeschützt",
  "theme.system": "System",
  "theme.light": "Hell",
  "theme.dark": "Dunkel",
  "lang.auto": "Systemsprache",
  "banner.notReadonly":
    "Dieser Server kann das Repository verändern (er wurde nicht mit --readonly gestartet). Rewind liest nur, aber prüfe, ob das nicht dein Administrationsserver ist.",
  "banner.untested":
    "Kopia {version} wurde nicht mit Rewind getestet (getestet: {tested}). Manches funktioniert vielleicht nicht.",
  "timeline.label": "Sicherung",
  "timeline.prev": "Frühere Sicherung",
  "timeline.next": "Spätere Sicherung",
  "timeline.unchangedUntil": "unverändert bis {time}",
  "timeline.identical": { one: "{n} identische Sicherung", other: "{n} identische Sicherungen" },
  "timeline.count": { one: "{n} Sicherung", other: "{n} Sicherungen" },
  "timeline.incomplete": "unvollständig",
  "timeline.none": "Diese Quelle hat noch keine Sicherungen.",
  "timeline.latest": "Neueste",
  "timeline.earliest": "Älteste",
  "nav.back": "Zurück",
  "nav.forward": "Vorwärts",
  "nav.up": "Eine Ebene höher",
  "nav.search": "In diesem Ordner suchen",
  "nav.clearSearch": "Suche löschen",
  "list.name": "Name",
  "list.modified": "Änderungsdatum",
  "list.type": "Typ",
  "list.size": "Größe",
  "list.folder": "Ordner",
  "list.link": "Symbolischer Link",
  "list.unknown": "Unbekannt",
  "list.empty": "Dieser Ordner ist leer.",
  "list.noMatch": "Keine Elemente passen zu „{q}“.",
  "list.loading": "Wird geladen…",
  "list.items": { one: "{n} Element", other: "{n} Elemente" },
  "list.selected": { one: "{n} ausgewählt ({size})", other: "{n} ausgewählt ({size})" },
  "list.snapshotOf": "Sicherung vom {time}",
  "list.files": { one: "{n} Datei", other: "{n} Dateien" },
  "notice.missing": "„{name}“ existiert in dieser Sicherung nicht. Angezeigt wird „{shown}“.",
  "common.retry": "Erneut versuchen",
  "common.close": "Schließen",
  "type.pdf": "PDF-Dokument",
  "type.doc": "Dokument",
  "type.sheet": "Tabelle",
  "type.slides": "Präsentation",
  "type.image": "Bild",
  "type.video": "Video",
  "type.audio": "Audio",
  "type.archive": "Archiv",
  "type.text": "Textdokument",
  "type.code": "Quellcode",
  "type.disk": "Datenträgerabbild",
  "type.app": "Programm",
  "type.ext": "{ext}-Datei",
  "type.file": "Datei",
};

const dicts: Record<"en" | "es" | "de", Dict> = { en, es, de };

export type Locale = keyof typeof dicts;

export function resolveLocale(pref: Language): Locale {
  if (pref !== "auto") return pref;
  for (const l of navigator.languages ?? [navigator.language]) {
    const base = l.toLowerCase().slice(0, 2);
    if (base === "es" || base === "de" || base === "en") return base;
  }
  return "en";
}

class I18n {
  locale = $state<Locale>("en");

  t(key: string, params: Record<string, string | number> = {}): string {
    const entry = dicts[this.locale][key] ?? en[key] ?? key;
    let text: string;
    if (typeof entry === "string") {
      text = entry;
    } else {
      const n = typeof params.n === "number" ? params.n : 0;
      text = new Intl.PluralRules(this.locale).select(n) === "one" ? entry.one : entry.other;
    }
    return text.replace(/\{(\w+)\}/g, (_, k: string) => {
      const v = params[k];
      if (v === undefined) return `{${k}}`;
      return typeof v === "number" ? this.number(v) : v;
    });
  }

  number(n: number): string {
    return new Intl.NumberFormat(this.locale).format(n);
  }

  /** e.g. "25 Nov 2026, 19:00" in local time. */
  dateTime(d: Date | number | string): string {
    return new Intl.DateTimeFormat(this.locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(d));
  }

  date(d: Date | number | string, opts: Intl.DateTimeFormatOptions): string {
    return new Intl.DateTimeFormat(this.locale, opts).format(new Date(d));
  }

  time(d: Date | number | string): string {
    return new Intl.DateTimeFormat(this.locale, { hour: "numeric", minute: "2-digit" }).format(new Date(d));
  }

  relative(d: Date | number | string): string {
    const diff = (new Date(d).getTime() - Date.now()) / 1000;
    const rtf = new Intl.RelativeTimeFormat(this.locale, { numeric: "auto" });
    const abs = Math.abs(diff);
    if (abs < 60) return rtf.format(Math.round(diff), "second");
    if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
    if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
    if (abs < 86400 * 30) return rtf.format(Math.round(diff / 86400), "day");
    if (abs < 86400 * 365) return rtf.format(Math.round(diff / (86400 * 30)), "month");
    return rtf.format(Math.round(diff / (86400 * 365)), "year");
  }

  size(bytes: number): string {
    if (bytes < 1024) return `${this.number(bytes)} B`;
    const units = ["KB", "MB", "GB", "TB", "PB"];
    let v = bytes / 1024;
    let u = 0;
    while (v >= 1024 && u < units.length - 1) {
      v /= 1024;
      u++;
    }
    const nf = new Intl.NumberFormat(this.locale, { maximumFractionDigits: v < 10 ? 1 : 0 });
    return `${nf.format(v)} ${units[u]}`;
  }
}

export const i18n = new I18n();
export const t = (key: string, params?: Record<string, string | number>) => i18n.t(key, params);
