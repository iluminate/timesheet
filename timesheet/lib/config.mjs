// config.json en ~/.config/timesheet/config.json
import { chmodSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { CONFIG_PATH, ensureDir, migrateLegacy } from './paths.mjs';
import { ApiError } from './api.mjs';

export const CONFIG_DEFAULTS = {
  activityCode: '1',
  categoryCode: '1',
  shiftCode: '0',
  hoursPerDay: 8,
};

export const REQUIRED_FIELDS = ['empCode', 'projectCode'];

export function configExists() {
  return existsSync(CONFIG_PATH);
}

/** Carga la configuracion, migrando el layout antiguo si hace falta. */
export function loadConfig() {
  migrateLegacy();
  if (!existsSync(CONFIG_PATH)) {
    throw new ApiError('falta configuración · timesheet init');
  }
  const raw = JSON.parse(readFileSync(CONFIG_PATH, 'utf8'));
  return { ...CONFIG_DEFAULTS, ...raw };
}

export function saveConfig(config) {
  ensureDir();
  const data = { ...CONFIG_DEFAULTS, ...config };
  writeFileSync(CONFIG_PATH, `${JSON.stringify(data, null, 2)}\n`, { mode: 0o600 });
  chmodSync(CONFIG_PATH, 0o600);
  return CONFIG_PATH;
}

export function missingFields(config) {
  return REQUIRED_FIELDS.filter((key) => !String(config[key] ?? '').trim());
}
