// Registro: al archivo ~/.config/timesheet/timesheet.log siempre;
// a la consola solo en modo -v o cuando es un aviso importante.
import { appendFileSync, mkdirSync } from 'node:fs';
import { CONFIG_DIR, LOG_PATH } from './paths.mjs';
import { dim, yellow } from './output.mjs';

let verbose = false;

export function setVerbose(value) {
  verbose = Boolean(value);
}

export function isVerbose() {
  return verbose;
}

/** Timestamp local "2026-10-30 18:30:05". */
export function stamp(date = new Date()) {
  const p = (n) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())} ${p(date.getHours())}:${p(date.getMinutes())}:${p(date.getSeconds())}`;
}

function write(level, message) {
  try {
    mkdirSync(CONFIG_DIR, { recursive: true, mode: 0o700 });
    appendFileSync(LOG_PATH, `${stamp()} ${level} ${message}\n`);
  } catch {
    // el log nunca debe romper el comando
  }
}

/** Detalle de depuracion: archivo siempre, consola solo con -v. */
export function log(message) {
  write('INFO', message);
  if (verbose) console.error(`${dim(stamp())} ${message}`);
}

/** Aviso visible siempre (reintentos de red, etc.). */
export function notice(message) {
  write('WARN', message);
  console.error(`${yellow('!')} ${message}`);
}

export function warn(message) {
  notice(message);
}

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
