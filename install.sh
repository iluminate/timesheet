#!/bin/sh
# Instalador de timesheet: un binario estático, sin Node ni navegador.
#
#   curl -fsSL https://raw.githubusercontent.com/iluminate/timesheet/main/install.sh | sh
#
# Variables:
#   TIMESHEET_REPO        repo de GitHub (por defecto iluminate/timesheet)
#   TIMESHEET_VERSION     tag a instalar (por defecto, el último release)
#   TIMESHEET_BIN_DIR     destino del binario (por defecto ~/.local/bin)
#   TIMESHEET_HOME        configuración (por defecto ~/.config/timesheet)
#   TIMESHEET_BASE_URL    URL directa con el binario (para pruebas locales)
#
# Uso directo: sh install.sh [--uninstall [--purge]]

set -eu

REPO="${TIMESHEET_REPO:-iluminate/timesheet}"
BIN_DIR="${TIMESHEET_BIN_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${TIMESHEET_HOME:-$HOME/.config/timesheet}"
SHARE_DIR="${TIMESHEET_SHARE_DIR:-$HOME/.local/share/timesheet}"
VERSION="${TIMESHEET_VERSION:-latest}"
BASE_URL="${TIMESHEET_BASE_URL:-}"
ASSET="timesheet-linux-x86_64"

say() { printf '%s\n' "$*"; }
ok() { printf '✔ %s\n' "$*"; }
warn() { printf '! %s\n' "$*" >&2; }
die() { printf '✘ %s\n' "$*" >&2; exit 1; }

usage() {
  say "uso: install.sh [--uninstall [--purge]]"
  say ""
  say "  instala el binario en $BIN_DIR/timesheet"
  say "  la configuración vive en $CONFIG_DIR"
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
    ok "navegador y datos locales eliminados ($SHARE_DIR)"
  fi

  if [ "$purge" = "1" ]; then
    rm -rf "$CONFIG_DIR"
    ok "configuración eliminada ($CONFIG_DIR)"
  else
    say "→ configuración conservada en $CONFIG_DIR (usa --purge para borrarla)"
  fi
}

case "${1:-}" in
  --uninstall) uninstall "${2:-}"; exit 0 ;;
  -h|--help) usage; exit 0 ;;
esac

[ "$(uname -s)" = "Linux" ] || die "solo Linux está soportado"
[ "$(uname -m)" = "x86_64" ] || die "solo x86_64 está soportado"
command -v curl >/dev/null 2>&1 || die "falta curl"
command -v sha256sum >/dev/null 2>&1 || die "falta sha256sum (coreutils)"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

if [ -n "$BASE_URL" ]; then
  URL="$BASE_URL"
else
  if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/$REPO/releases/latest/download"
  else
    URL="https://github.com/$REPO/releases/download/$VERSION"
  fi
fi

say "→ descargando $REPO ($VERSION)"
curl -fsSL --retry 3 "$URL/$ASSET" -o "$TMP/$ASSET" || die "no se pudo descargar $URL/$ASSET"
curl -fsSL --retry 3 "$URL/$ASSET.sha256" -o "$TMP/$ASSET.sha256" \
  || die "no se pudo descargar el checksum"

(cd "$TMP" && sha256sum -c "$ASSET.sha256" >/dev/null 2>&1) \
  || die "checksum inválido; instalación cancelada"

mkdir -p "$BIN_DIR"
if command -v install >/dev/null 2>&1; then
  install -m 755 "$TMP/$ASSET" "$BIN_DIR/timesheet"
else
  cp "$TMP/$ASSET" "$BIN_DIR/timesheet"
  chmod 755 "$BIN_DIR/timesheet"
fi

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
