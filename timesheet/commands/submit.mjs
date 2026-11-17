// timesheet submit [periodo] [--dry-run] [-f]
// Sin periodo muestra un selector con los periodos listos para enviar.
import { parseArgs } from 'node:util';
import { loadContext, record } from '../lib/context.mjs';
import { plan } from '../lib/plan.mjs';
import { contextFrom, fetchSummary, stateOf, summarize } from '../lib/report.mjs';
import { previousPeriods } from '../lib/periods.mjs';
import { ymd } from '../lib/dates.mjs';
import { canPrompt, select } from '../lib/prompt.mjs';
import {
  daysText,
  dim,
  DOT,
  ok,
  periodRange,
  plural,
  progress,
  progressDone,
} from '../lib/output.mjs';
import { log } from '../lib/log.mjs';
import { reportFatal } from '../lib/errors.mjs';

export const usage = `uso: timesheet submit [periodo] [--dry-run] [-f]

  sin periodo   elige entre los periodos listos para enviar
  periodo       current | 2026-11-01 | 01-Nov-2026
  --dry-run     valida y muestra, sin enviar
  -f            envía aunque falten días`;

async function submitOne(ctx, { dryRun = false, force = false } = {}) {
  const { config, client, today, period, emp, days, grid } = ctx;
  const head = periodRange(period, today);
  const s = summarize(ctx);

  if (!s.editable) {
    ok(`ya enviado ${dim(DOT)} ${head}`);
    return { code: 0, submitted: 0 };
  }

  if (!s.complete && !force) {
    throw new Error(
      `faltan ${daysText(s.total - s.filled)} ${dim('·')} timesheet fill ${ymd(period.start)}`,
    );
  }

  if (dryRun) {
    ok(`listo para enviar ${dim(DOT)} ${head} (${s.filled}/${s.total}) ${dim('· dry-run')}`);
    return { code: 0, submitted: 0 };
  }

  const p = plan({ emp, days, grid, period, today, config });
  await client.persist(p.entries, 'Submit');
  record({ action: 'submit', period: `${period.startApi} - ${period.endApi}`, dates: [] });
  log(`Submit OK · ${period.startApi} - ${period.endApi}`);
  ok(`enviado a aprobación ${dim(DOT)} ${head}`);
  return { code: 0, submitted: 1 };
}

async function pendingSubmit(ctx) {
  const summaries = [summarize(ctx)];
  const previous = await previousPeriods(ctx.client, ctx.period, ctx.today, 6);
  let i = 0;
  for (const p of previous) {
    i += 1;
    progress(`revisando ${i}/${previous.length}…`);
    summaries.push(await fetchSummary(ctx.client, p, ctx.today));
  }
  progressDone();
  return summaries.filter((s) => stateOf(s).action === 'submit');
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
        period: { type: 'string' },
        help: { type: 'boolean', short: 'h', default: false },
      },
    }));
  } catch (err) {
    throw new Error(`${err.message} · timesheet submit -h`);
  }

  if (values.help) {
    console.log(usage);
    return 0;
  }

  const target = positionals[0] ?? values.period ?? null;
  const options = { dryRun: values['dry-run'], force: values.force };
  const ctx = await loadContext({ periodValue: target });

  if (target) return (await submitOne(ctx, options)).code;

  if (!canPrompt()) throw new Error('indica el periodo · timesheet submit 2026-11-01');

  const periods = await pendingSubmit(ctx);
  if (!periods.length) {
    ok('nada que enviar');
    return 0;
  }

  const choice = await select({ title: '¿Qué periodo enviar?', items: itemsFrom(periods) });
  if (!choice) return 0;

  if (choice === 'all') {
    let done = 0;
    let sent = 0;
    for (const s of [...periods].reverse()) {
      if (done > 0) console.log('');
      const res = await submitOne(contextFrom(s, ctx), options);
      if (res.code !== 0) return res.code;
      sent += res.submitted;
      done += 1;
    }
    if (done > 1) {
      console.log('');
      ok(`${plural(sent, 'periodo enviado', 'periodos enviados')}`);
    }
    return 0;
  }

  return (await submitOne(contextFrom(choice, ctx), options)).code;
}

export function main(argv = []) {
  return run(argv).catch((err) => process.exit(reportFatal(err)));
}
