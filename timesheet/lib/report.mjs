// Resumen de un periodo: dias llenos, estado y siguiente accion.
import { normalizeDays, WORK_DAY } from './plan.mjs';
import { dayMonthWeekday, dim, periodRange, red, table, yellow } from './output.mjs';

const hours = (n) => `${Number(n).toFixed(2).replace(/\.?0+$/, '')}h`;

/** Calcula el resumen sin llamar a la API. */
export function summarize({ period, emp, days, grid, today }) {
  const info = normalizeDays(days);
  const rows = (grid ?? []).filter(Boolean);
  const hoursOn = (api) =>
    rows.reduce((sum, r) => {
      const col = (r.lstColumns ?? []).find((c) => c.Key === api);
      const value = Number(col?.Value);
      return sum + (Number.isFinite(value) ? value : 0);
    }, 0);

  const workdays = info.filter((d) => d.dayId === WORK_DAY && !d.nonWrk);
  const filled = workdays.filter((d) => hoursOn(d.api) > 0).length;
  const missingNow = workdays.filter((d) => d.ms <= today && hoursOn(d.api) === 0);
  const missingFuture = workdays.filter((d) => d.ms > today && hoursOn(d.api) === 0);
  const complete = workdays.length > 0 && workdays.every((d) => hoursOn(d.api) > 0);
  const lastWorkDay = workdays.length ? workdays[workdays.length - 1].ms : null;
  const kind = period.end < today ? 'pasado' : period.start > today ? 'futuro' : 'actual';

  return {
    period,
    emp,
    days,
    grid,
    info,
    rows,
    hoursOn,
    workdays,
    filled,
    total: workdays.length,
    missingNow,
    missingFuture,
    complete,
    lastWorkDay,
    kind,
    today,
    editable: ['0', '1'].includes(String(emp.Status)),
    isNew: rows.length > 0 && rows.every((r) => !(Number(r.TID) > 0)),
  };
}

/** Igual que `summarize` pero pidiendo los datos a iEngage. */
export async function fetchSummary(client, period, today) {
  const emp = await client.employee(period.startApi, period.endApi);
  const days = await client.days(period.startApi, period.endApi);
  const grid = await client.grid(period.startApi, period.endApi);
  return summarize({ period, emp, days, grid, today });
}

/** Estado corto para tablas: en curso, atrasado, falta submit, enviado, aprobado. */
export function stateOf(summary) {
  if (summary.kind === 'futuro') return { label: 'futuro', color: dim, action: null };
  if (!summary.editable) {
    const done = /complet/i.test(String(summary.emp.StatusDesc ?? ''));
    return { label: done ? 'aprobado' : 'enviado', color: dim, action: null };
  }
  if (summary.kind === 'pasado') {
    return summary.complete
      ? { label: 'falta submit', color: yellow, action: 'submit' }
      : { label: 'atrasado', color: red, action: 'fill' };
  }
  if (summary.complete && summary.lastWorkDay !== null && summary.today >= summary.lastWorkDay) {
    return { label: 'falta submit', color: yellow, action: 'submit' };
  }
  return {
    label: 'en curso',
    color: null,
    action: summary.missingNow.length > 0 ? 'fill' : null,
  };
}

/** `timesheet fill` / `timesheet submit`: el selector ya pregunta el periodo. */
export function commandFor(summary) {
  const { action } = stateOf(summary);
  return action ? `timesheet ${action}` : null;
}

/** Reconstruye el contexto que necesita `fill`/`submit` a partir de un resumen. */
export function contextFrom(summary, { config, client }) {
  return {
    config,
    client,
    today: summary.today,
    period: summary.period,
    kind: summary.kind,
    emp: summary.emp,
    days: summary.days,
    grid: summary.grid,
  };
}

const dayState = (d, s) => {
  const filled = s.hoursOn(d.api) > 0;
  if (d.dayId === 2) return 'finde';
  if (d.dayId === 4) return 'feriado';
  if (d.ms > s.today) return 'futuro';
  return filled ? 'ok' : 'falta';
};

/** Detalle completo, solo con -v. */
export function printDetail(summary) {
  const s = summary;
  console.log(`${dim('empleado')}  ${s.emp.EmpCode} · ${s.emp.EmpName}`);
  console.log(`${dim('estado')}    ${s.emp.StatusDesc}`);
  console.log(`${dim('periodo')}   ${periodRange(s.period, s.today)}`);
  if (s.isNew) console.log(`${dim('aviso')}     periodo sin inicializar (TID=0)`);
  console.log('');

  const rows = s.info.map((d) => [
    dayMonthWeekday(d.ms),
    d.dayType,
    s.hoursOn(d.api) > 0 ? hours(s.hoursOn(d.api)) : '-',
    dayState(d, s),
  ]);
  table(['fecha', 'día', 'horas', 'estado'], rows, {
    align: { 2: 'right' },
  });
  console.log('');
}
