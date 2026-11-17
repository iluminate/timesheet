import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { Client, loadCookie } from './api.mjs';
import { loadConfig } from './config.mjs';
import { ensureDir, migrateLegacy, COOKIE_PATH, CONFIG_PATH, STATE_PATH } from './paths.mjs';
import { log } from './log.mjs';
import { todayMs } from './dates.mjs';
import { resolvePeriodValue } from './resolve.mjs';

export { log, CONFIG_PATH, COOKIE_PATH, STATE_PATH };

/**
 * Carga configuracion, sesion y datos del periodo.
 * - `periodValue`: fecha dentro del periodo ("current" para hoy).
 * - `periodAt`: ms dentro del periodo (compatibilidad).
 */
export async function loadContext({ periodValue = null, periodAt = null } = {}) {
  migrateLegacy();
  const config = loadConfig();
  const client = new Client({ cookie: loadCookie(COOKIE_PATH), empCode: config.empCode });
  const today = todayMs();

  const period =
    periodAt !== null
      ? await client.currentPeriod(periodAt)
      : periodValue
        ? await resolvePeriodValue(client, today, periodValue)
        : await client.currentPeriod(today);

  const kind = period.end < today ? 'pasado' : period.start > today ? 'futuro' : 'actual';
  const emp = await client.employee(period.startApi, period.endApi);
  const days = await client.days(period.startApi, period.endApi);
  const grid = await client.grid(period.startApi, period.endApi);
  return { config, client, today, period, kind, emp, days, grid };
}

/** Anota cada escritura en ~/.config/timesheet/state.json. */
export function record(entry) {
  const state = existsSync(STATE_PATH) ? JSON.parse(readFileSync(STATE_PATH, 'utf8')) : [];
  state.push({ ts: new Date().toISOString(), ...entry });
  ensureDir();
  writeFileSync(STATE_PATH, `${JSON.stringify(state, null, 2)}\n`);
}
