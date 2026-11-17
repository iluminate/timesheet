// timesheet fill [periodo] [--dry-run] [-f]
// Sin periodo muestra un selector con lo que falta.
import { parseArgs } from 'node:util';
import { loadContext, record } from '../lib/context.mjs';
import { plan } from '../lib/plan.mjs';
import { contextFrom, fetchSummary, stateOf, summarize } from '../lib/report.mjs';
import { previousPeriods } from '../lib/periods.mjs';
import { parseAny, ymd } from '../lib/dates.mjs';
import { canPrompt, select } from '../lib/prompt.mjs';
import {
  dayMonthWeekday,
  daysText,
  dim,
  DOT,
  fail,
  ok,
  periodRange,
  progress,
  progressDone,
  warn,
} from '../lib/output.mjs';
import { log } from '../lib/log.mjs';
import { reportFatal } from '../lib/errors.mjs';

export const usage = `uso: timesheet fill [periodo] [--dry-run] [-f]

  sin periodo   elige entre los periodos pendientes
  periodo       current | 2026-11-01 | 01-Nov-2026
  --dry-run     muestra lo que haría, sin escribir
  -f            sobrescribe horas ya registradas`;

/** Rellena un periodo y, si queda completo, lo envía. */
async function fillOne(ctx, { dryRun = false, force = false, date = null } = {}) {
  const { config, client, today, period, kind, emp, days, grid } = ctx;
  const head = periodRange(period, today);

  if (kind === 'futuro') throw new Error(`periodo futuro ${dim(`· ${head}`)}`);

  const s0 = summarize(ctx);
  if (!s0.editable) {
    ok(`ya enviado ${dim(DOT)} ${head}`);
    return { code: 0, saved: 0, submitted: 0 };
  }

  const onlyDate = date ? ymd(parseAny(date)) : null;
  const p = plan({
    emp,
    days,
    grid,
    period,
    today,
    config,
    onlyDate,
    force,
    allowNew: true,
  });

  const targets = p.targets;
  const submitDue =
    s0.complete && (s0.kind === 'pasado' || (s0.lastWorkDay !== null && today === s0.lastWorkDay));

  // Nada que guardar: puede que solo falte enviar.
  if (!targets.length) {
    if (submitDue) {
      if (dryRun) {
        ok(`listo para enviar ${dim(DOT)} ${head} (${s0.filled}/${s0.total}) ${dim('· dry-run')}`);
        return { code: 0, saved: 0, submitted: 0 };
      }
      await client.persist(p.entries, 'Submit');
      record({ action: 'submit', period: `${period.startApi} - ${period.endApi}`, dates: [] });
      log(`Submit OK · ${period.startApi} - ${period.endApi}`);
      ok(`enviado a aprobación ${dim(DOT)} ${head}`);
      return { code: 0, saved: 0, submitted: 1 };
    }

    ok(`al día ${dim(DOT)} ${head} (${s0.filled}/${s0.total})`);
    if (s0.complete && s0.lastWorkDay !== null) {
      console.log(`${dim('→')} submit el ${dayMonthWeekday(s0.lastWorkDay)}`);
    } else if (s0.missingFuture.length > 0) {
      console.log(`${dim('→')} ${dayMonthWeekday(s0.missingFuture[0].ms)} pendiente`);
    } else if (s0.missingNow.length > 0) {
      warn(`faltan ${daysText(s0.missingNow.length)} ${dim('·')} vuelve mañana`);
    }
    return { code: 0, saved: 0, submitted: 0 };
  }

  const after = s0.filled + targets.length;
  const willSubmit =
    s0.kind === 'pasado' ? after === s0.total : after === s0.total && today === s0.lastWorkDay;
  const totalHours = targets.length * config.hoursPerDay;

  if (dryRun) {
    console.log(`${head}  ${s0.filled}/${s0.total} ${dim('→')} ${after}/${s0.total}`);
    for (const d of targets) console.log(`${dayMonthWeekday(d.ms)}  ${config.hoursPerDay}h`);
    const tail = `dry-run · ${daysText(targets.length)} · ${totalHours}h${willSubmit ? ' · luego envía' : ''}`;
    console.log(dim(tail));
    return { code: 0, saved: 0, submitted: 0 };
  }

  const res = await client.persist(p.entries, 'Save');
  log(`Save OK · ${targets.length} días · OUTPUT=${res.Result?.[0]?.OUTPUT ?? '?'}`);

  const grid2 = await client.grid(period.startApi, period.endApi);
  const s1 = summarize({ period, emp, days, grid: grid2, today });
  const verified = targets.every((d) => s1.hoursOn(d.api) > 0);
  if (!verified) {
    fail('el servidor no confirmó las horas');
    return { code: 1, saved: 0, submitted: 0 };
  }

  record({
    action: 'save',
    period: `${period.startApi} - ${period.endApi}`,
    dates: targets.map((d) => d.api),
    verified,
  });

  console.log(`${head}  ${s0.filled}/${s0.total} ${dim('→')} ${s1.filled}/${s1.total}`);
  console.log(`${dim('guardado')} ${DOT} ${daysText(targets.length)} ${DOT} ${totalHours}h`);

  if (s1.complete && (s1.kind === 'pasado' || today === s1.lastWorkDay)) {
    const p2 = plan({ emp, days, grid: grid2, period, today, config });
    await client.persist(p2.entries, 'Submit');
    record({ action: 'submit', period: `${period.startApi} - ${period.endApi}`, dates: [] });
    log(`Submit OK · ${period.startApi} - ${period.endApi}`);
    ok('enviado a aprobación');
    return { code: 0, saved: targets.length, submitted: 1 };
  }

  if (s1.complete && s1.lastWorkDay !== null) {
    console.log(`${dim('→')} submit el ${dayMonthWeekday(s1.lastWorkDay)}`);
  } else if (s1.missingNow.length > 0) {
    warn(`quedan ${daysText(s1.missingNow.length)} ${dim('·')} ${head}`);
  }
  return { code: 0, saved: targets.length, submitted: 0 };
}

/** Periodos editables con días por rellenar. */
async function pendingPeriods(ctx) {
  const summaries = [summarize(ctx)];
  const previous = await previousPeriods(ctx.client, ctx.period, ctx.today, 6);
  let i = 0;
  for (const p of previous) {
    i += 1;
    progress(`revisando ${i}/${previous.length}…`);
    summaries.push(await fetchSummary(ctx.client, p, ctx.today));
  }
  progressDone();
  return summaries.filter((s) => stateOf(s).action === 'fill');
}

function itemsFrom(periods) {
  const items = [];
  if (periods.length > 1) {
    items.push({ label: `todos los pendientes (${periods.length})`, value: 'all' });
  }
  for (const s of periods) {
    items.push({
      label: `${periodRange(s.period, s.today)}  ${s.filled}/${s.total}  ${stateOf(s).label}`,
      value: s,
    });
  }
  return items;
}

export async function run(argv = []) {
  let values;
  let positionals;
  try {
    ({ values, positionals } = parseArgs({
      args: argv,
      allowPositionals: true,
      options: {
        'dry-run': { type: 'boolean', default: false },
        force: { type: 'boolean', short: 'f', default: false },
        date: { type: 'string' },
        period: { type: 'string' },
        help: { type: 'boolean', short: 'h', default: false },
      },
    }));
  } catch (err) {
    throw new Error(`${err.message} · timesheet fill -h`);
  }

  if (values.help) {
    console.log(usage);
    return 0;
  }

  const target = positionals[0] ?? values.period ?? null;
  const options = { dryRun: values['dry-run'], force: values.force, date: values.date };
  const ctx = await loadContext({ periodValue: target });

  if (target) return (await fillOne(ctx, options)).code;

  if (!canPrompt()) throw new Error('indica el periodo · timesheet fill 2026-11-01');

  const periods = await pendingPeriods(ctx);
  if (!periods.length) {
    ok('nada pendiente');
    return 0;
  }

  const choice = await select({ title: '¿Qué periodo?', items: itemsFrom(periods) });
  if (!choice) return 0;

  if (choice === 'all') {
    let done = 0;
    let submitted = 0;
    for (const s of [...periods].reverse()) {
      if (done > 0) console.log('');
      const res = await fillOne(contextFrom(s, ctx), options);
      if (res.code !== 0) return res.code;
      submitted += res.submitted;
      done += 1;
    }
    if (done > 1) {
      console.log('');
      ok(`${plural(done, 'periodo', 'periodos')} ${dim('·')} ${plural(submitted, 'enviado', 'enviados')}`);
    }
    return 0;
  }

  return (await fillOne(contextFrom(choice, ctx), options)).code;
}

export function main(argv = []) {
  return run(argv).catch((err) => process.exit(reportFatal(err)));
}
