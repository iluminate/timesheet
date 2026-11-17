import { fromApi, ymd } from './dates.mjs';

export const WORK_DAY = 1;
export const WEEKEND_DAY = 2;
export const HOLIDAY_DAY = 4;
export const HALF_DAY = 5;

const fmtHours = (n) => Number(n).toFixed(2);

/**
 * Normaliza el calendario del periodo.
 * @returns {{api:string, ms:number, dayId:number, dayType:string, nonWrk:boolean}[]}
 */
export function normalizeDays(days) {
  return days
    .map((d) => ({
      api: d.StartDate,
      ms: fromApi(d.StartDate),
      dayId: Number(d.DayID),
      dayType: String(d.DayType ?? ''),
      nonWrk: String(d.EnblNonWRKDay ?? '0') === '1',
    }))
    .sort((a, b) => a.ms - b.ms);
}

/**
 * Construye el plan del dia: que dias faltan, el payload exacto y si toca submit.
 */
export function plan({
  emp,
  days,
  grid,
  period,
  today,
  config,
  onlyDate = null,
  force = false,
  noop = false,
  allowNew = false,
}) {
  const info = normalizeDays(days);
  const periodArg = ymd(period.start ?? fromApi(period.startApi));
  const initHint = `Ejecuta: timesheet fill ${periodArg}`;
  const allRows = (grid ?? []).filter(Boolean);
  if (!allRows.length) {
    throw new Error('El periodo no tiene filas de proyecto. Abre el timesheet una vez en la web y reintenta.');
  }

  // iEngage devuelve una fila "plantilla" con TID=0 cuando el periodo aun no
  // se ha abierto en la web. `allowNew` permite adoptarla y dejar que el Save
  // cree la fila usando las fechas reales del periodo (no las de la plantilla).
  const pendingInit = allRows.filter((r) => !(Number(r.TID) > 0));
  const savedRows = allRows.filter((r) => Number(r.TID) > 0);
  let rows = savedRows;
  if (!rows.length) {
    const pendingPrimary = pendingInit.find(
      (r) => String(r.ProjectCode) === String(config.projectCode),
    );
    if (allowNew && pendingPrimary) {
      rows = [pendingPrimary];
    } else {
      const found = pendingInit.map((r) => r.ProjectCode).filter(Boolean).join(', ') || 'sin proyecto';
      throw new Error(
        `El periodo aun no esta inicializado (filas con TID=0: ${found}). ${initHint}`,
      );
    }
  }

  const primary = rows.find((r) => String(r.ProjectCode) === String(config.projectCode));
  if (!primary) {
    const pendingPrimary = pendingInit.find(
      (r) => String(r.ProjectCode) === String(config.projectCode),
    );
    if (pendingPrimary) {
      throw new Error(
        `La fila del proyecto ${config.projectCode} aun no esta inicializada (TID=0). ${initHint}`,
      );
    }
    const found = rows.map((r) => r.ProjectCode).join(', ') || 'ninguno';
    throw new Error(`No hay fila para el proyecto ${config.projectCode}. Filas del periodo: ${found}`);
  }

  const isNew = !(Number(primary.TID) > 0);
  const periodEnd = period.end ?? fromApi(period.endApi);
  const isPast = periodEnd < today;

  const workdays = info.filter((d) => d.dayId === WORK_DAY && !d.nonWrk);

  const hoursOn = (dayApi) =>
    rows.reduce((sum, r) => {
      const col = (r.lstColumns ?? []).find((c) => c.Key === dayApi);
      const v = Number(col?.Value);
      return sum + (Number.isFinite(v) ? v : 0);
    }, 0);

  // Periodo pasado: se rellenan todos sus laborables. Periodo actual: solo hasta hoy.
  let targets = noop ? [] : workdays.filter((d) => isPast || d.ms <= today);
  if (onlyDate) {
    const match = targets.filter((d) => ymd(d.ms) === onlyDate || d.api === onlyDate);
    if (!match.length) throw new Error(`${onlyDate} no es un dia laborable pendiente dentro del periodo.`);
    targets = match;
  }
  if (!force) targets = targets.filter((d) => hoursOn(d.api) === 0);

  const targetSet = new Set(targets.map((d) => d.api));

  const entries = [];
  for (const row of rows) {
    for (const d of info) {
      const col = (row.lstColumns ?? []).find((c) => c.Key === d.api) ?? {};
      const isTarget = row === primary && targetSet.has(d.api);
      entries.push({
        TID: row.TID,
        EmpCode: emp.EmpCode,
        TimesheetFromDate: period.startApi,
        TimesheetToDate: period.endApi,
        ProjectCode: isTarget ? config.projectCode : row.ProjectCode,
        ActivityCode: isTarget ? config.activityCode : row.ActivityCode,
        CategoryCode: isTarget ? config.categoryCode : row.CategoryCode,
        ShiftCode: isTarget ? config.shiftCode : row.ShiftCode,
        SupervisorCode: emp.SupervisorCode ?? '',
        IsMultipleSupv: String(emp.IsMultipleSupv ?? false),
        TimesheetDate: d.api,
        EffortHrs: isTarget ? fmtHours(config.hoursPerDay) : String(col.Value ?? ''),
        DayID: d.dayId,
        DID: col.DID ?? 0,
        Remarks1: String(col.Remarks1 ?? ''),
      });
    }
  }

  const missing = workdays.filter((d) => hoursOn(d.api) === 0 && d.ms <= today);
  const pendingFuture = workdays.filter((d) => hoursOn(d.api) === 0 && d.ms > today);
  const lastWorkDay = workdays.length ? workdays[workdays.length - 1].ms : null;
  const complete = workdays.every((d) => hoursOn(d.api) > 0);

  return {
    info,
    workdays,
    targets,
    missing,
    pendingFuture,
    entries,
    hoursOn,
    complete,
    lastWorkDay,
    primary,
    rows,
    isNew,
    isPast,
  };
}

/**
 * Tras un save, recalcula para decidir si toca submit automatico.
 */
export function submitDue({ today, complete, lastWorkDay }) {
  return Boolean(complete && lastWorkDay !== null && today === lastWorkDay);
}


