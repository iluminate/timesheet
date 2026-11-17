// Rutas del CLI. Todo lo personal vive en ~/.config/timesheet,
// nunca dentro del paquete instalado.
import { chmodSync, copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));

export const CONFIG_DIR = process.env.TIMESHEET_HOME
  ? process.env.TIMESHEET_HOME
  : join(homedir(), '.config', 'timesheet');

export const CONFIG_PATH = join(CONFIG_DIR, 'config.json');
export const COOKIE_PATH = join(CONFIG_DIR, 'cookie.txt');
export const STATE_PATH = join(CONFIG_DIR, 'state.json');
export const LOG_PATH = join(CONFIG_DIR, 'timesheet.log');
export const BROWSER_DIR = join(CONFIG_DIR, 'browser');

export function ensureDir() {
  mkdirSync(CONFIG_DIR, { recursive: true, mode: 0o700 });
}

/**
 * Primera ejecucion desde el repo: copia config.json, cookie y estado
 * del layout antiguo (timesheet/config.json, timesheet/.auth/cookie.txt).
 * Nunca sobrescribe nada existente.
 */
export function migrateLegacy() {
  const legacyDir = join(HERE, '..');
  const legacyConfig = join(legacyDir, 'config.json');
  if (existsSync(CONFIG_PATH) || !existsSync(legacyConfig)) return false;

  ensureDir();
  copyFileSync(legacyConfig, CONFIG_PATH);
  chmodSync(CONFIG_PATH, 0o600);

  const legacyCookie = join(legacyDir, '.auth', 'cookie.txt');
  if (!existsSync(COOKIE_PATH) && existsSync(legacyCookie)) {
    copyFileSync(legacyCookie, COOKIE_PATH);
    chmodSync(COOKIE_PATH, 0o600);
  }

  const legacyState = join(legacyDir, 'state.json');
  if (!existsSync(STATE_PATH) && existsSync(legacyState)) {
    copyFileSync(legacyState, STATE_PATH);
  }

  return true;
}
