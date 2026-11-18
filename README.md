# timesheet

CLI para registrar horas en iEngage (CoForge) desde la terminal.

Un solo binario estático, sin Node ni Playwright. El login se hace en tu
navegador: tú escribes la contraseña y el 2FA en la web real de iEngage y el CLI
captura la sesión automáticamente.

## Instalación

Linux x86_64 y `curl`:

```bash
curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh
```

El instalador descarga el último release, verifica su checksum y deja
`timesheet` en `~/.local/bin`. La configuración vive en `~/.config/timesheet`.

Variables útiles: `TIMESHEET_VERSION` (tag), `TIMESHEET_BIN_DIR`,
`TIMESHEET_HOME`, `TIMESHEET_BROWSER` (ruta del navegador),
`TIMESHEET_DATA_DIR` (caché del navegador) y `TIMESHEET_BASE_URL` (para pruebas
locales).

Desinstalar:

```bash
curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh -s -- --uninstall
```

Añade `--purge` para borrar también `~/.config/timesheet`.

## Uso

```bash
timesheet                      # estado del periodo actual y pendientes
timesheet list                 # periodos recientes
timesheet list --all -n 10     # incluye enviados y aprobados

timesheet fill                 # selector de periodos pendientes
timesheet fill 2026-11-01      # directo, sin preguntar
timesheet fill --date 2026-11-17
timesheet fill --dry-run       # solo muestra

timesheet submit               # selector de periodos por enviar
timesheet login                # abre iEngage y guarda la sesión
timesheet init                 # configuración inicial

timesheet -v                   # detalle día a día
```

`periodo` acepta `current`, `2026-11-01` o `01-Nov-2026`.

## Login

`timesheet login` abre iEngage en tu navegador:

1. Usa Chrome, Chromium o Edge si están instalados.
2. Inicias sesión como siempre, con contraseña y 2FA/SSO.
3. El CLI captura la sesión, la valida y cierra la ventana.

Tu contraseña nunca pasa por `timesheet`: se escribe directamente en la web de
iEngage. El navegador usa un perfil temporal aislado y no toca tu perfil normal.

Si no hay navegador instalado, la primera vez ofrece descargar Chrome for
Testing (~200 MB de descarga, ~400 MB en disco) con checksum verificado. Se
guarda en `~/.local/share/timesheet/chrome` y se reutiliza en los siguientes
logins.

Opciones:

```bash
timesheet login --timeout 15      # minutos de espera (por defecto 10)
timesheet login --browser /ruta   # usar un navegador concreto
```

Si ejecutas cualquier comando sin sesión (o con la sesión vencida), el CLI
**abre el navegador automáticamente**, espera el login y continúa con el comando.
Para scripts o CI puedes desactivarlo:

```bash
timesheet --no-login status
TIMESHEET_NO_AUTO_LOGIN=1 timesheet status
```

Para servidores sin entorno gráfico o CI queda el respaldo avanzado
`timesheet login --cookie ARCHIVO`. No es necesario en el uso normal.

## Solución de problemas

- **No hay entorno gráfico:** usa `timesheet login --cookie ARCHIVO`.
- **Chromium es Snap/Flatpak o la política bloquea DevTools:** el CLI puede
  ofrecer Chrome for Testing; si tampoco funciona, usa `--cookie`.
- **El navegador no arranca:** el mensaje incluye las últimas líneas de error.
  En Ubuntu/Debian suele faltar alguna librería (`libnss3`, `libgbm1`, etc.).
- **La descarga del navegador está bloqueada por el proxy:** instala
  Chrome/Chromium desde el gestor de paquetes o usa `--cookie`.

## Reglas

- Periodo **actual**: rellena los laborables hasta hoy.
- Periodo **pasado**: rellena todos sus laborables.
- Periodo **futuro**: error.
- `fill` envía a aprobación cuando el periodo queda completo y toca
  (periodo pasado, o último día laborable del actual).
- Sin `--dry-run` escribe; `-f` sobrescribe horas ya registradas.

## Estado

| salida | significado |
|---|---|
| `en curso` | periodo actual, aún quedan días |
| `atrasado` | periodo pasado con días sin horas |
| `falta submit` | completo pero sin enviar |
| `enviado` | esperando aprobación |
| `aprobado` | cerrado |

## Configuración

`~/.config/timesheet/`

| archivo | contenido |
|---|---|
| `config.json` | empCode, projectCode, horas por día |
| `cookie.txt` | sesión de iEngage |
| `timesheet.log` | detalle de cada ejecución |

Si se descargó el navegador de respaldo, vive en
`~/.local/share/timesheet/chrome/` (se borra con `install.sh --uninstall`).

## Desarrollo

```bash
cargo run -- list        # ejecutar desde el repo
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Binario estático para publicar (Ubuntu/Debian: `sudo apt install musl-tools`):

```bash
rustup target add x86_64-unknown-linux-musl
CC_x86_64_unknown_linux_musl=musl-gcc \
  cargo build --release --target x86_64-unknown-linux-musl
```

`install.sh` acepta `TIMESHEET_BASE_URL` para probar sin publicar:

```bash
mkdir -p dist
cp target/x86_64-unknown-linux-musl/release/timesheet dist/timesheet-linux-x86_64
cd dist
sha256sum timesheet-linux-x86_64 > timesheet-linux-x86_64.sha256
python3 -m http.server 8099
```

```bash
TIMESHEET_BASE_URL=http://127.0.0.1:8099 sh install.sh
```

## Release

El workflow `.github/workflows/release.yml` compila el binario estático y lo
publica junto a su checksum al empujar un tag:

```bash
git tag v2.1.0
git push origin v2.1.0
```
