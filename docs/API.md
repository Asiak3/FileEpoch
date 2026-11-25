# API del servidor web de Kopia (descubrimiento, Fase 0)

> Kopia **no documenta una API pública estable**. Todo lo de este documento se ha obtenido
> leyendo el código fuente de **Kopia v0.23.1** (`internal/server/*.go`, `internal/serverapi`,
> `snapshot/`, `fs/`) y de la interfaz web **kopia/htmlui** (commit `e9f6ace`, 2026-06-08, el que
> corresponde a `htmluibuild v0.0.1-0.20260608231842-7bf9fcc0831d` embebido en v0.23.1).
> Puede cambiar en cualquier versión de Kopia; por eso la app debe comprobar la versión (§3).

Marcadores de estado usados en la tabla de endpoints:

- **Código** = verificado leyendo el código de v0.23.1.
- **Vivo** = verificado contra un servidor real 0.23.1.
- **Pendiente** = no verificado todavía contra un servidor real.

**Todos los ejemplos JSON de este documento se han construido a partir de los tipos Go** (nombres
de campo y formatos exactos según las etiquetas `json:"…"`); **ninguno es una respuesta grabada**
de un servidor real. Se sustituirán por fixtures anonimizados cuando se verifique en vivo (§8).

Valores de ejemplo usados en este documento (marcadores de posición, no datos reales):
`https://kopia.example:51515`, usuario `ui-user`, host `host1`, usuario de origen `user1`,
ruta de origen `/data/docs`.

---

## 1. Resumen

| Necesidad                         | Endpoint                                   | CSRF | Estado                 |
|-----------------------------------|--------------------------------------------|------|------------------------|
| Obtener token CSRF + versión      | `GET /`                                    | no   | Código                 |
| Estado del repositorio (readonly) | `GET /api/v1/repo/status`                  | sí   | Código; 401 sin credenciales: Vivo |
| Listar fuentes (sources)          | `GET /api/v1/sources`                      | sí   | Código                 |
| Listar snapshots de una fuente    | `GET /api/v1/snapshots?host=&userName=&path=[&all=1]` | sí | Código        |
| Listar un directorio              | `GET /api/v1/objects/{oid}` (oid de directorio) | **no** | Código           |
| Descargar un archivo              | `GET /api/v1/objects/{oid}` (oid de archivo)    | **no** | Código           |
| Descargar una carpeta             | *no existe endpoint*: recorrido recursivo + descarga de cada archivo | — | Código |

Lista blanca propuesta para la app (método **y** ruta, nada más):

```
GET  /
GET  /api/v1/repo/status
GET  /api/v1/sources
GET  /api/v1/snapshots
GET  /api/v1/objects/{oid}
```

Cualquier otra combinación de método/ruta debe rechazarse en el propio cliente, antes de salir
a la red. Ver §7 para el motivo: `--readonly` **no** hace seguros el resto de endpoints.

---

## 2. Autenticación

### 2.1 Modelo real (corrige la hipótesis inicial)

No hay "login con sesión". El servidor exige **HTTP Basic en cada petición**
(`Server.isAuthenticated`, `internal/server/server.go`):

1. Si la petición no trae `Authorization: Basic …` → `401`, cuerpo `Missing credentials.`,
   cabecera `WWW-Authenticate: Basic realm="Kopia"`.
2. Si trae Basic **y** una cookie `Kopia-Auth` válida para ese usuario → aceptada sin comprobar la
   contraseña (es solo una caché para evitar el hash de la contraseña).
3. Si no, se valida usuario/contraseña. Si falla → `401`, cuerpo `Access denied.` (y el servidor
   registra el intento fallido en su log).
4. Si es válida, el servidor responde con `Set-Cookie: Kopia-Auth=<JWT HS256>; Path=/` con
   caducidad de **1 minuto**. Es opcional reenviarla; reduce carga en el servidor.

Después, para casi todas las rutas `/api/v1/*` (las registradas con `handleUI` /
`handleUIPossiblyNotConnected`) se exige además un **token CSRF** (§2.2). Si falta o no coincide →
`401`, cuerpo `Invalid or missing CSRF token.`

**Diagnóstico de la prueba con curl:** un `curl -u usuario …/api/v1/…` con Basic correcto pero sin
cookie de sesión ni cabecera CSRF da exactamente este último 401. No faltaba un "login", faltaba el
par cookie de sesión + token CSRF.

Hay por tanto **tres cuerpos distintos de 401**, y el cliente debe distinguirlos para dar un
mensaje útil:

| Cuerpo (text/plain)               | Significado                             | Acción del cliente                   |
|-----------------------------------|-----------------------------------------|--------------------------------------|
| `Missing credentials.`            | No se envió Basic                       | Error de programación                |
| `Access denied.`                  | Usuario o contraseña incorrectos        | Pedir de nuevo la contraseña         |
| `Invalid or missing CSRF token.`  | Token caducado / servidor reiniciado    | Repetir el arranque CSRF (§2.2) una vez y reintentar |

### 2.2 Arranque CSRF

Código: `ServeStaticFiles` y `patchIndexBytes` en `server.go`, `generateCSRFToken` /
`validateCSRFToken` en `server_authz_checks.go`, `App.jsx` en htmlui.

1. `GET /` con Basic. El servidor:
   - fija `Set-Cookie: Kopia-Session-Cookie=<uuid>; Path=/` (si la petición no traía ya esa cookie);
   - devuelve `index.html` con `<meta name="kopia-csrf-token" content="<token>" />` insertado justo
     antes de `</head>`.
2. En cada llamada a `/api/v1/*` enviar:
   - `Authorization: Basic …`
   - `Cookie: Kopia-Session-Cookie=<uuid>` (y opcionalmente `Kopia-Auth=<jwt>`)
   - `X-Kopia-Csrf-Token: <token>`

El token es `hex(HMAC-SHA256(clave, sessionID))`. La clave (`authCookieSigningKey`) es un UUID
aleatorio generado **en cada arranque del servidor** salvo que se fije con
`--auth-cookie-signing-key`. Consecuencia: tras reiniciar el servidor, todos los tokens y cookies
dejan de valer → el cliente debe repetir el paso 1 al recibir `Invalid or missing CSRF token.`

`GET /api/v1/objects/{oid}` está registrado con `csrfTokenNotRequired`: solo necesita Basic.

### 2.3 Usuario autorizado para la UI

Además de autenticarse, el usuario debe ser **el usuario de UI del servidor**, es decir el valor de
`--server-username` con el que se arrancó (`requireUIUser`). Si no coincide:

- `GET /` → `403`, `UI Access denied. See https://github.com/kopia/kopia/issues/880#…`
- `/api/v1/*` → `403`, JSON `{"code":"ACCESS_DENIED","error":"access is denied"}`
- `/api/v1/objects/{oid}` → `403`, texto `access denied`

Usuarios definidos con `--htpasswd-file` o con `kopia server user add` pueden autenticarse, pero
solo el usuario de UI puede usar estos endpoints.

### 2.4 Formato de errores JSON

Las rutas `handleUI*` devuelven errores como `text/plain` (sic) con cuerpo JSON:

```json
{ "code": "NOT_CONNECTED", "error": "not connected" }
```

Códigos posibles (`serverapi.APIErrorCode`): `INTERNAL`, `ALREADY_CONNECTED`,
`ALREADY_INITIALIZED`, `INVALID_PASSWORD`, `INVALID_TOKEN`, `MALFORMED_REQUEST`, `NOT_CONNECTED`,
`NOT_FOUND`, `NOT_INITIALIZED`, `PATH_NOT_FOUND`, `STORAGE_CONNECTION`, `ACCESS_DENIED`.
HTTP: 400 (petición), 403 (acceso), 404, 500.

---

## 3. Versión del servidor y comprobaciones de seguridad

**No existe endpoint de versión.** La versión se obtiene del mismo `GET /` del arranque CSRF:
`patchIndexBytes` sustituye

- `</title>` → ` v<versión></title>` (p. ej. `<title>KopiaUI v0.23.1</title>`)
- `<p class="version-info">Version ` → `<p class="version-info">Version v<versión> <buildinfo> `

Si el binario no tiene versión (compilación de desarrollo) no se añade nada. Si se usa
`--ui-title-prefix`, el prefijo aparece antes de `KopiaUI`. La app debe extraer `v\d+\.\d+\.\d+`
del `<title>` y, si no está en la lista de versiones probadas, avisar claramente.

`GET /api/v1/repo/status` incluye los campos de `ClientOptions`, entre ellos `readonly`:

```json
{
  "connected": true,
  "configFile": "<ruta en el servidor>",
  "formatVersion": 3,
  "hash": "BLAKE2B-256-128",
  "encryption": "AES256-GCM-HMAC-SHA256",
  "splitter": "DYNAMIC-4M-BUZHASH",
  "storage": "filesystem",
  "supportsContentCompression": true,
  "hostname": "host1",
  "username": "user1",
  "readonly": true,
  "description": "My Repository",
  "enableActions": false
}
```

(Valores ilustrativos, construidos a partir de los tipos Go; no es una respuesta grabada.)

**Requisito para la app:** si `readonly` no es `true`, mostrar una advertencia clara (o negarse a
conectar, según se decida, ver §9). Es una defensa concreta contra apuntar la app por error al
servidor de administración. Si `connected` es `false`, el servidor no tiene repositorio abierto
(`initTaskID` puede indicar que se está abriendo): mostrar "servidor sin repositorio" y reintentar.

### 3.1 Certificado TLS y huella

Kopia compara `hex(SHA-256(certificado DER))` en minúsculas, sin separadores
(`internal/tlsutil/tlsutil.go`, `verifyPeerCertificate`), y la imprime así al arrancar:
`SERVER CERT SHA256: 0a1b2c…` (64 caracteres hex). `openssl x509 -fingerprint -sha256` da el mismo valor en
mayúsculas con `:`. La app debe aceptar ambos formatos (normalizar: quitar `:` y espacios,
pasar a minúsculas) y comprobar que la huella coincide con **cualquiera** de los certificados
presentados, igual que Kopia.

---

## 4. Fuentes (sources)

`GET /api/v1/sources` — CSRF: sí. Filtros opcionales por query: `host`, `userName`, `path`
(coincidencia exacta).

```json
{
  "localUsername": "user1",
  "localHost": "host1",
  "multiUser": true,
  "sources": [
    {
      "source": { "host": "host1", "userName": "user1", "path": "/data/docs" },
      "status": "IDLE",
      "schedule": { "intervalSeconds": 7200 },
      "lastSnapshot": { "id": "…", "source": { … }, "startTime": "…", "endTime": "…",
                        "stats": { … }, "rootEntry": { "name": "docs", "type": "d", "obj": "k…", "summ": { … } } },
      "nextSnapshotTime": "2026-01-01T12:00:00Z"
    }
  ]
}
```

Notas:

- Incluye **todas** las fuentes del repositorio (todas las combinaciones usuario@host:ruta con
  snapshots o con política propia), no solo las del host local. La web de Kopia filtra por
  `localHost`/`localUsername` en el cliente; nuestra app debería mostrar todas, agrupadas por
  `usuario@host`.
- La lista se calcula al arrancar el servidor y en cada *refresh* (por defecto cada 4 h,
  `--refresh-interval`). Una fuente nueva puede tardar en aparecer.
- `status` y `nextSnapshotTime` no son relevantes en un servidor de solo lectura (no programa
  snapshots porque `isLocal()` es falso cuando `readonly`).

---

## 5. Snapshots de una fuente

`GET /api/v1/snapshots?host=<h>&userName=<u>&path=<p>[&all=1]` — CSRF: sí.

- Los tres parámetros deben indicarse; se usan como `SourceInfo` exacto (con `path` vacío se
  pediría otra cosa).
- Orden: **del más antiguo al más reciente** (`snapshot.SortByTime(manifests, false)`: ascendente
  por `startTime`, desempate por `endTime`).
- Sin `all`: el servidor **agrupa** snapshots con el mismo `rootID` (contenido idéntico) y, como
  recorre la lista en orden ascendente, conserva el **más antiguo** de cada grupo y le suma los
  `retention`/`pins` de los demás. Esto es el "agrupar idénticos" de la web. Con
  `all=<cualquier valor no vacío>` devuelve todos.
- Consecuencia para la barra temporal: un grupo de snapshots idénticos se muestra con la fecha del
  primero ("contenido vigente desde…"). Si la app quiere mostrar también "hasta…", debe pedir
  `all=1` y agrupar ella misma por `rootID` (barato: es la misma lista).

```json
{
  "snapshots": [
    {
      "id": "a1b2c3…",
      "description": "",
      "startTime": "2026-01-01T10:00:00.123456789Z",
      "endTime":   "2026-01-01T10:02:13.5Z",
      "incomplete": "checkpoint",
      "summary": { "size": 123456789, "files": 1200, "symlinks": 0, "dirs": 80,
                   "maxTime": "2025-12-31T18:00:00Z", "numFailed": 0 },
      "rootID": "k0123456789abcdef0123456789abcdef",
      "retention": ["latest-1", "hourly-1", "daily-1"],
      "pins": []
    }
  ],
  "unfilteredCount": 240,
  "uniqueCount": 37
}
```

- `incomplete` solo aparece en snapshots incompletos (checkpoints); la app debería marcarlos.
- `rootID` es el OID del directorio raíz del snapshot → entrada a §6.
- **Fechas:** RFC 3339 con nanosegundos, siempre en **UTC** (`fs.UTCTimestamp.MarshalJSON`). La app
  convierte a hora local del usuario para mostrarlas.

---

## 6. Directorios y archivos: `GET /api/v1/objects/{oid}`

Un único endpoint sirve para listar directorios y descargar archivos (`handleObjectGet`,
`api_object_get.go`). CSRF: **no**. Solo está registrado para GET. Ojo: una petición HEAD no da 405, la recoge el
manejador de archivos estáticos (`PathPrefix("/")`), así que no sirve para consultar tamaños; usar
siempre GET (el tamaño ya viene en la entrada del directorio).

### 6.1 Directorio

**Gramática de un OID** (`repo/object/objectid.go`, `repo/content/index/id.go`):

```
OID      = "I"* ["Z"] CONTENT          ("I" y "Z" son excluyentes)
CONTENT  = [prefijo g..z] HEX          (HEX: hash en hexadecimal, longitud par, hasta 64)
```

- `I` (una o varias) = objeto troceado con índice: lo usan **archivos grandes** y también
  directorios grandes.
- `Z` = objeto comprimido.
- Prefijo de contenido `k` = metadatos de directorio; archivos normales no llevan prefijo.

**Regla para el cliente:** saber si algo es archivo o directorio por el campo `type` de su entrada
(o porque es el `rootID` de un snapshot), **nunca** por el prefijo del OID. La validación del
`{oid}` en la lista blanca debe aceptar toda la gramática anterior, p. ej.
`^(I+|Z)?[g-z]?[0-9a-f]{2,64}$`, y nada más (así tampoco se puede inyectar `/`, `?`, `..`).

Si el OID es de directorio la respuesta es `application/json` con el `DirManifest` tal cual está
guardado en el repositorio (no lo genera el servidor; es el formato interno de Kopia):

```json
{
  "stream": "kopia:directory",
  "entries": [
    { "name": "Informe.pdf", "type": "f", "mode": "0644", "size": 3355443,
      "mtime": "2025-11-15T16:50:00Z", "uid": 1000, "gid": 1000, "obj": "8f3e…" },
    { "name": "Fotos", "type": "d", "mode": "0755",
      "mtime": "2025-11-14T10:00:00Z", "uid": 1000, "gid": 1000, "obj": "k9a1b…",
      "summ": { "size": 987654321, "files": 4200, "symlinks": 0, "dirs": 35,
                "maxTime": "2025-11-14T09:59:00Z", "numFailed": 0 } },
    { "name": "actual", "type": "s", "mode": "0777", "size": 12,
      "mtime": "2025-11-01T00:00:00Z", "obj": "c41d…" }
  ],
  "summary": { "size": …, "files": …, "dirs": …, "maxTime": "…", "numFailed": 0 }
}
```

- `type`: `f` archivo, `d` directorio, `s` enlace simbólico (otros tipos posibles en el código:
  vacío = desconocido).
- `mode`: permisos como **cadena octal**; omitido si es 0.
- `size`, `uid`, `gid`, `summ` se omiten si son 0/nulos (`omitempty`): el parser debe tolerar
  ausencias.
- `mtime`: UTC, RFC 3339 nanosegundos.
- **Sin paginación.** Un directorio con 100 000 entradas llega como un único JSON (decenas de MB).
  La "carga perezosa" tiene que ser del lado del cliente: parsear en streaming / en segundo plano,
  virtualizar la lista, y cargar subdirectorios solo al abrirlos.
- No hay endpoint para resolver una ruta (`/a/b/c`) dentro de un snapshot: se navega desde
  `rootID` bajando entrada por entrada. (`POST /api/v1/paths/resolve` existe pero resuelve rutas
  del **sistema de archivos del servidor**, no de un snapshot, y no está en la lista blanca.)

### 6.2 Archivo

Si el OID es de archivo, la respuesta es el contenido, servido con `http.ServeContent`:

- Soporta **Range** (`Range: bytes=…` → 206): permite reanudar descargas grandes.
- `Content-Type` lo deduce Go por la extensión de `fname` o por sniffing; no fiarse de él.
- Query opcional `fname=<nombre>` → añade `Content-Disposition: attachment; filename="<nombre>"`.
  La app no lo necesita (y el nombre no se escapa, así que mejor no usarlo).
- Query opcional `mtime=<RFC3339Nano>` → se usa como `Last-Modified`. No necesario.
- Para enlaces simbólicos (`type: "s"`), el objeto contiene el **texto del destino** del enlace.

### 6.3 Errores

| Caso                          | Respuesta                                 |
|-------------------------------|-------------------------------------------|
| OID mal formado               | 400 `invalid object id`                   |
| OID inexistente               | 404 `object not found`                    |
| Usuario no es el de UI        | 403 `access denied`                       |
| Otro error al abrir el objeto | No se trata en el código (ver §9): probablemente 500 o conexión cortada |

### 6.4 Descarga de carpetas

**No existe** endpoint de descarga de carpeta (zip/tar) que escriba en el cliente. El único zip/tar
es `POST /api/v1/restore`, que **escribe en el disco del servidor** y está prohibido (§7).

Restaurar una carpeta = recorrer recursivamente desde su OID (`GET objects/{oid}` de cada
subdirectorio) y descargar cada archivo con `GET objects/{oid}`. Los metadatos de cada entrada
(`mtime`, `mode`, `size`) permiten restaurar la fecha de modificación y verificar el tamaño.

**Verificación:** los OID de Kopia se derivan de un hash con clave secreta del repositorio, así
que el cliente **no puede** recalcular un hash comparable. Verificación posible: tamaño (`size`)
y fecha (`mtime`). TLS ya protege la integridad en tránsito.

---

## 7. Endpoints prohibidos y por qué `--readonly` no basta

`kopia server start --readonly` abre el repositorio con el almacenamiento envuelto en un
`readonly` wrapper (`repo/open.go`): **impide escribir blobs del repositorio**, pero el servidor
sigue registrando todas las rutas, y varias actúan sobre el **host del servidor** o sobre su
estado, no sobre el repositorio:

| Ruta                                                    | Efecto aun con `--readonly`                              |
|---------------------------------------------------------|----------------------------------------------------------|
| `POST /api/v1/restore`                                  | Crea archivos/zip/tar en el disco del servidor           |
| `POST /api/v1/mounts`, `DELETE /api/v1/mounts/{id}`     | Monta/desmonta snapshots en el servidor                  |
| `POST /api/v1/repo/disconnect`                          | Desconecta el repositorio (servidor fuera de servicio)   |
| `POST /api/v1/repo/connect`, `/repo/create`, `/repo/exists` | Conecta/crea repositorios desde el servidor          |
| `PUT /api/v1/ui-preferences`                            | Escribe un archivo JSON en el servidor                   |
| `PUT /api/v1/repo/throttle`                             | Cambia la limitación del servidor                        |
| `POST /api/v1/tasks/{id}/cancel`                        | Cancela tareas del servidor                              |
| `POST /api/v1/refresh`, `POST /api/v1/repo/sync`        | Fuerza refresh del servidor (inocuo en principio, ver §9)|
| `POST /api/v1/sources`, `/sources/upload`, `/sources/cancel` | Crear fuentes/snapshots (fallan en readonly, pero no se deben intentar) |
| `POST /api/v1/snapshots/delete`, `/snapshots/edit`      | Borrar/editar snapshots                                   |
| `PUT/DELETE /api/v1/policy`, `POST /api/v1/policy/resolve` | Políticas                                             |
| `POST/DELETE /api/v1/notificationProfiles…`, `POST /api/v1/testNotificationProfile` | Notificaciones (envía mensajes) |
| `POST /api/v1/estimate`                                 | Lanza una tarea que recorre el disco del servidor         |
| `/api/v1/control/*`                                     | API de control (otro usuario, `server-control`)          |
| gRPC `/kopia_repository.KopiaRepository/*`              | API de repositorio para clientes kopia                    |

Por eso la lista blanca (§1) es **exacta y solo GET**, y el cliente debe imponerla por
construcción (no hay forma de construir una petición a otra ruta), no por convención.

Tampoco se usan, aunque sean GET de lectura, porque no hacen falta: `/api/v1/policy`,
`/api/v1/policies`, `/api/v1/cli` (revela la ruta del ejecutable y del config del servidor),
`/api/v1/repo/algorithms`, `/api/v1/repo/throttle`, `/api/v1/mounts…`, `/api/v1/current-user`,
`/api/v1/ui-preferences`, `/api/v1/tasks…`, `/api/v1/notificationProfiles…`. Se pueden añadir a la
lista blanca más adelante si se justifica.

---

## 8. Cómo verificarlo en vivo (receta con curl)

Pasos para que **el propietario del servidor** confirme los endpoints. `curl -u usuario` sin
`:contraseña` hace que curl la **pida por teclado**: no queda en el historial ni en archivos.
`--pinnedpubkey` no sirve aquí (fija la clave pública, no el certificado); para pruebas manuales
se usa `-k` comprobando antes la huella con `openssl`.

```bash
S=https://kopia.example:51515
U=ui-user
J=$(mktemp)    # tarro de cookies temporal; borrarlo al terminar

# 0) Huella del certificado (comparar con la que imprime Kopia al arrancar)
echo | openssl s_client -connect kopia.example:51515 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha256

# 1) Arranque: cookie de sesión + token CSRF + versión
curl -sk -u "$U" -c "$J" "$S/" -o index.html
grep -o '<title>[^<]*</title>' index.html
TOKEN=$(grep -o 'kopia-csrf-token" content="[0-9a-f]*' index.html | sed 's/.*content="//')

# 2) Estado (comprobar "readonly": true)
curl -sk -u "$U" -b "$J" -H "X-Kopia-Csrf-Token: $TOKEN" "$S/api/v1/repo/status"

# 3) Fuentes
curl -sk -u "$U" -b "$J" -H "X-Kopia-Csrf-Token: $TOKEN" "$S/api/v1/sources"

# 4) Snapshots de una fuente (codificar la ruta)
curl -sk -u "$U" -b "$J" -H "X-Kopia-Csrf-Token: $TOKEN" -G "$S/api/v1/snapshots" \
  --data-urlencode host=host1 --data-urlencode userName=user1 --data-urlencode path=/data/docs

# 5) Directorio raíz de un snapshot (rootID de la respuesta anterior) — sin CSRF
curl -sk -u "$U" "$S/api/v1/objects/<rootID>"

# 6) Descarga parcial de un archivo (comprueba Range → HTTP 206)
curl -sk -u "$U" -r 0-99 -o /dev/null -w '%{http_code} %{size_download}\n' "$S/api/v1/objects/<obj>"

rm -f "$J" index.html
```

Cada `curl -u "$U"` pide la contraseña de nuevo. Las salidas pueden contener nombres de archivos y
rutas reales: **no** pegarlas en el repositorio sin anonimizarlas.

---

## 9. Dudas abiertas

1. **Verificación en vivo pendiente.** Hasta ahora solo se ha comprobado en vivo el `401` sin
   credenciales (`Missing credentials.` + `WWW-Authenticate: Basic realm="Kopia"`), que el servidor
   habla HTTP/1.1 sobre TLS y el certificado autofirmado. Falta ejecutar §8 con credenciales (o
   recibir el HAR sanitizado) y grabar fixtures anonimizados para la Fase 1.
2. **¿Rechazar o solo avisar si `readonly` ≠ `true`?** Propuesta: rechazar por defecto con opción
   avanzada para permitirlo. Decisión del propietario.
3. **Frescura de los datos.** Las fuentes se recalculan solo en cada *refresh* del servidor
   (`--refresh-interval`, 4 h por defecto). Las listas de snapshots se leen bajo demanda, pero el
   índice del repositorio puede estar en caché. ¿Aparecen los snapshots nuevos (cada 2 h) sin
   llamar a `POST /api/v1/repo/sync`? Hay que comprobarlo en vivo. Si no aparecen, decidir entre
   (a) añadir `POST /api/v1/repo/sync` a la lista blanca (no escribe en el repositorio, pero es un
   POST y dispara trabajo en el servidor) o (b) recomendar un `--refresh-interval` menor.
4. **Errores no tratados en `handleObjectGet`.** Si `OpenObject` falla por algo distinto de "no
   encontrado", el código sigue con `obj == nil`. Esperable un 500 o conexión cortada; el cliente
   debe tratarlo como error recuperable y reintentar una vez.
5. **Directorios enormes.** Sin paginación; hay que medir en vivo el tamaño/tiempo del JSON del
   directorio más grande y decidir si hace falta parseo en streaming.
6. **Límites de concurrencia.** No hay límite explícito en el servidor HTTP para descargas
   paralelas (`--max-concurrency` afecta a gRPC). Proponer por defecto 4 descargas simultáneas,
   configurable.
7. **Nombres de archivo no válidos en Windows** (`:`, `?`, `*`, `\`, nombres reservados como
   `CON`, mayúsculas/minúsculas que colisionan) en snapshots de Linux. Hace falta una política de
   saneado al restaurar. Decisión del propietario.
8. **Enlaces simbólicos al restaurar en Windows:** ¿crearlos (requiere privilegios / modo
   desarrollador), guardarlos como archivo de texto, u omitirlos e informarlo? Propuesta: omitirlos
   y listarlos en el resumen.
9. **Compatibilidad con otras versiones.** Solo se ha leído 0.23.1. Las rutas usadas existen desde
   hace muchas versiones, pero no se ha comprobado ninguna otra.
10. **Idioma de la documentación del repositorio** (este archivo está en español). Para un proyecto
    open source quizá convenga el inglés. Decisión del propietario.
