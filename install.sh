#!/bin/sh
# Instalador de timesheet.
#
#   curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh
#
# Variables:
#   TIMESHEET_REPO        repo de GitHub (por defecto iluminate/timesheet)
#   TIMESHEET_VERSION     tag a instalar (por defecto, el ultimo release)
#   TIMESHEET_BIN_DIR     destino del lanzador (por defecto ~/.local/bin)
#   TIMESHEET_SHARE_DIR   binario y dependencias (por defecto ~/.local/share/timesheet)
#   TIMESHEET_HOME        configuracion (por defecto ~/.config/timesheet)
#   TIMESHEET_SKIP_BROWSER=1  no instala Playwright/Chromium
#   TIMESHEET_BASE_URL    URL directa con timesheet.mjs (para pruebas locales)
#
# Uso directo: sh install.sh [--uninstall [--purge]]

set -eu

REPO="${TIMESHEET_REPO:-iluminate/timesheet}"
BIN_DIR="${TIMESHEET_BIN_DIR:-$HOME/.local/bin}"
SHARE_DIR="${TIMESHEET_SHARE_DIR:-$HOME/.local/share/timesheet}"
BROWSERS_DIR="$SHARE_DIR/browsers"
CONFIG_DIR="${TIMESHEET_HOME:-$HOME/.config/timesheet}"
VERSION="${TIMESHEET_VERSION:-latest}"
BASE_URL="${TIMESHEET_BASE_URL:-}"

say() { printf '%s\n' "$*"; }
ok() { printf '✔ %s\n' "$*"; }
warn() { printf '! %s\n' "$*" >&2; }
die() { printf '✘ %s\n' "$*" >&2; exit 1; }

usage() {
  say "uso: install.sh [--uninstall [--purge]]"
  say ""
  say "  instala el lanzador en $BIN_DIR/timesheet"
  say "  el binario y Playwright viven en $SHARE_DIR"
}

uninstall() {
  purge=0
  [ "${1:-}" = "--purge" ] && purge=1

  for file in "$BIN_DIR/timesheet" "$BIN_DIR/timesheet.mjs"; do
    if [ -f "$file" ]; then
      rm -f "$file"
      ok "eliminado ($file)"
    fi
  done

  if [ -d "$SHARE_DIR" ]; then
    rm -rf "$SHARE_DIR"
    ok "binario y dependencias eliminados ($SHARE_DIR)"
  fi

  if [ "$purge" = "1" ]; then
    rm -rf "$CONFIG_DIR"
    ok "configuración eliminada ($CONFIG_DIR)"
  else
    say "→ configuración conservada en $CONFIG_DIR (usa --purge para borrarla)"
  fi
}

case "${1:-}" in
  --uninstall) uninstall "${2:-}" ; exit 0 ;;
  -h|--help) usage ; exit 0 ;;
esac

command -v curl >/dev/null 2>&1 || die "falta curl"
command -v sha256sum >/dev/null 2>&1 || die "falta sha256sum (coreutils)"
command -v node >/dev/null 2>&1 || die "falta Node 20+ · https://nodejs.org"
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
[ "$NODE_MAJOR" -ge 20 ] || die "Node $NODE_MAJOR no sirve; se necesita 20+"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

if [ -n "$BASE_URL" ]; then
  say "→ descargando $BASE_URL/timesheet.mjs"
  curl -fsSL --retry 3 "$BASE_URL/timesheet.mjs" -o "$TMP/timesheet.mjs" \
    || die "no se pudo descargar"
  curl -fsSL --retry 3 "$BASE_URL/timesheet.mjs.sha256" -o "$TMP/timesheet.mjs.sha256" \
    || die "no se pudo descargar el checksum"
else
  if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/$REPO/releases/latest/download"
  else
    URL="https://github.com/$REPO/releases/download/$VERSION"
  fi

  say "→ descargando release ($REPO · $VERSION)"
  curl -fsSL --retry 3 "$URL/timesheet.mjs" -o "$TMP/timesheet.mjs" \
    || die "no se pudo descargar $URL/timesheet.mjs"
  curl -fsSL --retry 3 "$URL/timesheet.mjs.sha256" -o "$TMP/timesheet.mjs.sha256" \
    || die "no se pudo descargar el checksum"
fi

( cd "$TMP" && sha256sum -c timesheet.mjs.sha256 >/dev/null 2>&1 ) \
  || die "checksum inválido; instalación cancelada"
say "✔ checksum verificado"

mkdir -p "$SHARE_DIR" "$BIN_DIR"
if command -v install >/dev/null 2>&1; then
  install -m 755 "$TMP/timesheet.mjs" "$SHARE_DIR/timesheet.mjs"
else
  cp "$TMP/timesheet.mjs" "$SHARE_DIR/timesheet.mjs"
  chmod 755 "$SHARE_DIR/timesheet.mjs"
fi

# Node resuelve `import('playwright')` desde SHARE_DIR/node_modules.
if [ "${TIMESHEET_SKIP_BROWSER:-0}" = "1" ]; then
  warn "Playwright omitido (TIMESHEET_SKIP_BROWSER=1); timesheet login no funcionará"
else
  command -v npm >/dev/null 2>&1 || die "falta npm (viene con Node) para instalar Playwright"

  if [ -d "$SHARE_DIR/node_modules/playwright" ]; then
    say "→ Playwright ya instalado"
  else
    say "→ instalando Playwright (puede tardar)"
    npm_config_update_notifier=false npm install --prefix "$SHARE_DIR" \
      --no-audit --no-fund --loglevel=error playwright \
      || die "no se pudo instalar Playwright"
  fi

  if ls "$BROWSERS_DIR"/chromium-* >/dev/null 2>&1; then
    say "→ Chromium ya instalado"
  else
    say "→ descargando Chromium (~150 MB)"
    if ! PLAYWRIGHT_BROWSERS_PATH="$BROWSERS_DIR" \
      "$SHARE_DIR/node_modules/.bin/playwright" install chromium; then
      warn "no se pudo instalar Chromium"
      say "  prueba: sudo $SHARE_DIR/node_modules/.bin/playwright install --with-deps chromium"
      say "  timesheet login no funcionará hasta entonces"
    fi
  fi
fi

cat > "$BIN_DIR/timesheet" <<LAUNCHER
#!/bin/sh
# Lanzador de timesheet.
export PLAYWRIGHT_BROWSERS_PATH="$BROWSERS_DIR"
exec node "$SHARE_DIR/timesheet.mjs" "\$@"
LAUNCHER
chmod 755 "$BIN_DIR/timesheet"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    warn "$BIN_DIR no está en el PATH"
    say "  agrega esto a tu ~/.bashrc:"
    say "    export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac

ok "instalado en $BIN_DIR/timesheet"
"$BIN_DIR/timesheet" --version || die "el binario no arranca"

if [ -f "$CONFIG_DIR/config.json" ]; then
  say "→ timesheet"
else
  say "→ timesheet init"
fi
