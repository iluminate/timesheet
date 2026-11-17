// timesheet            -> estado del periodo actual + pendientes cercanos
// timesheet status X   -> detalle de un periodo concreto
import { parseArgs } from 'node:util';
import { loadContext } from '../lib/context.mjs';
import { commandFor, fetchSummary, printDetail, stateOf, summarize } from '../lib/report.mjs';
import { previousPeriods } from '../lib/periods.mjs';
import { isVerbose } from '../lib/log.mjs';
import { hint, periodRange, progress, progressDone, table } from '../lib/output.mjs';
import { reportFatal } from '../lib/errors.mjs';

export const usage = `uso: timesheet [status] [periodo] [-n N] [-v]

  periodo   current | 2026-11-01 | 01-Nov-2026
  -n N      periodos anteriores a revisar (por defecto 3)`;

export function rowsOf(summaries) {
  return summaries.map((s) => {
    const state = stateOf(s);
    const label = state.color ? state.color(state.label) : state.label;
    return [periodRange(s.period, s.today), `${s.filled}/${s.total}`, label];
  });
}

export function printActions(summaries) {
  const commands = [];
  for (const s of summaries) {
    const command = commandFor(s);
    if (command && !commands.includes(command)) commands.push(command);
  }
  if (!commands.length) return;
  console.log('');
  for (const command of commands.slice(0, 3)) hint(command);
}

export async function run(argv = []) {
  let values;
  let positionals;
  try {
    ({ values, positionals } = parseArgs({
      args: argv,
      allowPositionals: true,
      options: {
        n: { type: 'string', default: '3' },
        help: { type: 'boolean', short: 'h', default: false },
      },
    }));
  } catch (err) {
    throw new Error(`${err.message} · timesheet -h`);
  }

  if (values.help) {
    console.log(usage);
    return 0;
  }

  const back = Number(values.n);
  if (!Number.isInteger(back) || back < 0) throw new Error('--n debe ser un entero >= 0');

  const target = positionals[0] ?? null;
  const ctx = await loadContext({ periodValue: target });
  const current = summarize(ctx);

  if (target) {
    if (isVerbose()) printDetail(current);
    table(['periodo', 'días', 'estado'], rowsOf([current]));
    printActions([current]);
    return 0;
  }

  const previous = await previousPeriods(ctx.client, ctx.period, ctx.today, back);
  const summaries = [current];
  let i = 0;
  for (const p of previous) {
    i += 1;
    progress(`consultando ${i}/${previous.length}…`);
    summaries.push(await fetchSummary(ctx.client, p, ctx.today));
  }
  progressDone();

  const visible = summaries.filter((s, index) => index === 0 || stateOf(s).action !== null);
  if (isVerbose()) for (const s of visible) printDetail(s);
  table(['periodo', 'días', 'estado'], rowsOf(visible));
  printActions(visible);
  return 0;
}

export function main(argv = []) {
  return run(argv).catch((err) => process.exit(reportFatal(err)));
}
