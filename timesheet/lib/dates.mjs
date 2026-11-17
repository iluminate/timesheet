// Fechas del timesheet en formato "22-Oct-2026".
// Todo se compara en milisegundos a medianoche+12h UTC para evitar lios de TZ.
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const NOON = 12 * 60 * 60 * 1000;

/** Hoy, segun el reloj local, anclado a las 12:00 UTC. */
export function todayMs() {
  const n = new Date();
  return Date.UTC(n.getFullYear(), n.getMonth(), n.getDate()) + NOON;
}

/** "22-Oct-2026" -> ms */
export function fromApi(s) {
  const m = /^(\d{1,2})-([A-Za-z]{3})-(\d{4})$/.exec(String(s ?? '').trim());
  if (!m) throw new Error(`Fecha invalida de la API: ${JSON.stringify(s)}`);
  const mi = MONTHS.indexOf(m[2][0].toUpperCase() + m[2].slice(1).toLowerCase());
  if (mi < 0) throw new Error(`Mes invalido de la API: ${s}`);
  return Date.UTC(Number(m[3]), mi, Number(m[1])) + NOON;
}

/** ms -> "22-Oct-2026" */
export function toApi(ms) {
  const d = new Date(ms);
  return `${String(d.getUTCDate()).padStart(2, '0')}-${MONTHS[d.getUTCMonth()]}-${d.getUTCFullYear()}`;
}

/** ms -> "2026-10-27" */
export function ymd(ms) {
  return new Date(ms).toISOString().slice(0, 10);
}

/** Acepta "2026-10-27" o "27-Oct-2026" y devuelve ms. */
export function parseAny(s) {
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) {
    const [y, m, d] = s.split('-').map(Number);
    return Date.UTC(y, m - 1, d) + NOON;
  }
  return fromApi(s);
}

export function dayName(ms) {
  return ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'][new Date(ms).getUTCDay()];
}
