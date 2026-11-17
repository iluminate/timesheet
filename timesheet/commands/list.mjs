// timesheet list [-n N] [--all]
import { parseArgs } from 'node:util';
import { loadContext } from '../lib/context.mjs';
import { fetchSummary, stateOf, summarize } from '../lib/report.mjs';
import { previousPeriods } from '../lib/periods.mjs';
import { isVerbose } from '../lib/log.mjs';
import { periodRange, progress, progressDone, table } from '../lib/output.mjs';
import { reportFatal } from '../lib/errors.mjs';
import { printActions } from './status.mjs';

export const usage = `uso: timesheet list [-n N] [--all]

  -n N     periodos anteriores a mostrar (por defecto 6)
  --all    incluye los ya enviados o aprobados`;

export async function run(argv = []) {
  let values;
  try {
    ({ values } = parseArgs({
      args: argv,
      options: {
        n: { type: 'string', default: '6' },
        all: { type: 'boolean', short: 'a', default: false },
        help: { type: 'boolean', short: 'h', default: false },
      },
    }));
  } catch (err) {
    throw new Error(`${err.message} · timesheet list -h`);
  }

  if (values.help) {
    console.log(usage);
    return 0;
  }

  const back = Number(values.n);
  if (!Number.isInteger(back) || back < 0) throw new Error('--n debe ser un entero >= 0');

  const ctx = await loadContext();
  const current = summarize(ctx);
  const previous = await previousPeriods(ctx.client, ctx.period, ctx.today, back);

  const summaries = [current];
  let i = 0;
  for (const p of previous) {
    i += 1;
    progress(`consultando ${i}/${previous.length}…`);
    summaries.push(await fetchSummary(ctx.client, p, ctx.today));
  }
  progressDone();

  const visible = values.all
    ? summaries
    : summaries.filter((s, index) => index === 0 || stateOf(s).action !== null);

  if (isVerbose()) {
    const { printDetail } = await import('../lib/report.mjs');
    for (const s of visible) printDetail(s);
  }

  table(
    ['periodo', 'días', 'estado'],
    visible.map((s) => {
      const state = stateOf(s);
      const label = state.color ? state.color(state.label) : state.label;
      return [periodRange(s.period, s.today), `${s.filled}/${s.total}`, label];
    }),
  );
  printActions(visible);
  return 0;
}

export function main(argv = []) {
  return run(argv).catch((err) => process.exit(reportFatal(err)));
}
