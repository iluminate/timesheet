# Agents

Agentes simples para tareas repetitivas. Cada carpeta es un agente independiente.

| agente | que hace | estado |
|--------|----------|--------|
| [`timesheet/`](timesheet/README.md) | Registra horas diarias en iEngage (CoForge) | listo |

## Instalación

Linux, Node 20+ y `curl`:

```bash
curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh
```

El instalador baja el último release, verifica su checksum, instala Playwright y
Chromium, y deja `timesheet` en `~/.local/bin`.

| ruta | contenido |
|------|-----------|
| `~/.local/bin/timesheet` | lanzador |
| `~/.local/share/timesheet/` | binario, Playwright y Chromium |
| `~/.config/timesheet/` | configuración y sesión |

Desinstalar:

```bash
curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh -s -- --uninstall
```

Añade `--purge` para borrar también `~/.config/timesheet`.

## Uso

```bash
timesheet            # estado del periodo
timesheet list       # periodos recientes
timesheet fill       # elige el periodo y lo rellena
timesheet login      # renueva la sesión
```

## Desarrollo

```bash
npm install
npm run dev -- list
npm run build        # dist/timesheet.mjs + sha256
```

Cada agente documenta sus comandos en su propio README.
