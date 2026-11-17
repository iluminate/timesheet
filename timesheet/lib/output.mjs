// Salida del CLI: color solo en TTY, fechas cortas y tablas alineadas.
const tty = Boolean(process.stdout.isTTY) && !process.env.NO_COLOR;
const wrap = (code) => (s) => (tty ? `\x1b[${code}m${s}\x1b[0m` : String(s));

export const bold = wrap('1');
export const dim = wrap('2');
export const red = wrap('31');
export const green = wrap('32');
export const yellow = wrap('33');
export const cyan = wrap('36');

export const OK = green('✔');
export const FAIL = red('✘');
export const ARROW = dim('→');
export const DOT = dim('·');

const MONTHS = ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sep', 'oct', 'nov', 'dic'];
const WEEKDAYS = ['dom', 'lun', 'mar', 'mié', 'jue', 'vie', 'sáb'];

const strip = (s) => String(s).replace(/\x1b\[[0-9;]*m/g, '');
const padEnd = (s, n) => s + ' '.repeat(Math.max(0, n - strip(s).length));
const padStart = (s, n) => ' '.repeat(Math.max(0, n - strip(s).length)) + s;

/** "2026-11-09" anclado a 12:00 UTC -> "09 nov" */
export function dayMonth(ms) {
  const d = new Date(ms);
  return `${String(d.getUTCDate()).padStart(2, '0')} ${MONTHS[d.getUTCMonth()]}`;
}

export function weekday(ms) {
  return WEEKDAYS[new Date(ms).getUTCDay()];
}

export function dayMonthWeekday(ms) {
  return `${dayMonth(ms)} ${weekday(ms)}`;
}

/** Rango corto: "08 nov – 14 nov" (con año solo si no es el actual). */
export function range(startMs, endMs, todayMs = Date.now()) {
  const s = new Date(startMs);
  const e = new Date(endMs);
  const currentYear = new Date(todayMs).getUTCFullYear();
  const year = s.getUTCFullYear();
  const suffix = year === currentYear ? '' : ` ${String(year).slice(2)}`;
  const sameMonth = s.getUTCMonth() === e.getUTCMonth();
  const left = sameMonth
    ? String(s.getUTCDate()).padStart(2, '0')
    : `${String(s.getUTCDate()).padStart(2, '0')} ${MONTHS[s.getUTCMonth()]}`;
  return `${left} – ${dayMonth(endMs)}${suffix}`;
}

export function periodRange(period, todayMs = Date.now()) {
  return range(period.start, period.end, todayMs);
}

export function say(line = '') {
  console.log(line);
}

export function ok(message) {
  console.log(`${OK} ${message}`);
}

export function warn(message) {
  console.log(`${yellow('!')} ${message}`);
}

export function fail(message) {
  console.error(`${FAIL} ${message}`);
}

export function hint(command) {
  console.log(`${ARROW} ${cyan(command)}`);
}

/** "1 día" / "5 días" */
export function plural(n, one, many) {
  return `${n} ${n === 1 ? one : many}`;
}

export function daysText(n) {
  return plural(n, 'día', 'días');
}

let progressActive = false;

/** Progreso efimero en stderr, solo si hay terminal. */
export function progress(text) {
  if (!process.stderr.isTTY) return;
  progressActive = true;
  process.stderr.write(`\r\x1b[2K${dim(text)}`);
}

export function progressDone() {
  if (!progressActive) return;
  process.stderr.write('\r\x1b[2K');
  progressActive = false;
}

/** Imprime una tabla con encabezado tenue y columnas alineadas. */
export function table(headers, rows, { align = {}, color = {} } = {}) {
  const widths = headers.map((h) => strip(h).length);
  for (const row of rows) {
    row.forEach((cell, i) => {
      widths[i] = Math.max(widths[i] ?? 0, strip(cell).length);
    });
  }
  const render = (row) =>
    row
      .map((cell, i) => (align[i] === 'right' ? padStart(cell, widths[i]) : padEnd(cell, widths[i])))
      .join('  ')
      .trimEnd();

  console.log(dim(render(headers)));
  for (const row of rows) {
    const line = row.map((cell, i) => {
      const text = align[i] === 'right' ? padStart(cell, widths[i]) : padEnd(cell, widths[i]);
      return color[i] ? color[i](text) : text;
    });
    console.log(line.join('  ').trimEnd());
  }
}
