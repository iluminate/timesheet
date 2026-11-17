# timesheet

CLI para registrar horas en iEngage (CoForge) desde la terminal.

## Instalación

Linux, Node 20+ y `curl`:

```bash
curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh
```

El instalador baja el último release, verifica el checksum, instala Playwright y
Chromium en `~/.local/share/timesheet`, y deja el lanzador `timesheet` en
`~/.local/bin`. La configuración vive en `~/.config/timesheet`.

Variables útiles: `TIMESHEET_VERSION` (tag), `TIMESHEET_SKIP_BROWSER=1` (sin
Playwright/Chromium) y `TIMESHEET_BIN_DIR` / `TIMESHEET_SHARE_DIR`.

Desinstalar:

```bash
timesheet --version   # comprobar
curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh -s -- --uninstall
```

## Uso

```bash
timesheet                      # estado del periodo actual y pendientes
timesheet list                 # periodos recientes
timesheet list --all -n 10     # incluye enviados y aprobados

timesheet fill                 # selector de periodos pendientes
timesheet fill 2026-11-01      # directo, sin preguntar
timesheet fill --dry-run       # solo muestra

timesheet submit               # selector de periodos por enviar
timesheet login                # renovar sesión (2FA)
timesheet init                 # configuración inicial
```

`periodo` acepta `current`, `2026-11-01` o `01-Nov-2026`.

`-v` muestra el detalle: empleado, estado y tabla día a día.

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
| `state.json` | historial de guardados y envíos |
| `timesheet.log` | detalle de cada ejecución |

## Desarrollo

```bash
npm install
npm run dev -- list            # ejecutar desde el repo
npm run build                  # dist/timesheet.mjs + sha256
```

`install.sh` acepta `TIMESHEET_BASE_URL` para probar sin publicar:

```bash
cd dist && python3 -m http.server 8099
TIMESHEET_BASE_URL=http://127.0.0.1:8099 sh install.sh
```

## Release

El workflow `.github/workflows/release.yml` publica `timesheet.mjs` y su
checksum al empujar un tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```
