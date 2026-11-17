// timesheet init [--emp CODE] [--project CODE] [--hours N]
import { homedir } from 'node:os';
import { createInterface } from 'node:readline/promises';
import { parseArgs } from 'node:util';
import { CONFIG_PATH } from '../lib/paths.mjs';
import { CONFIG_DEFAULTS, configExists, loadConfig, saveConfig } from '../lib/config.mjs';
import { dim, ok } from '../lib/output.mjs';
import { reportFatal } from '../lib/errors.mjs';

export const usage = `uso: timesheet init [--emp CODE] [--project CODE] [--hours N]

  --emp       código de empleado en iEngage
  --project   código de proyecto (ej. 20788-7)
  --hours     horas por día laborable (por defecto 8)`;

const pretty = (p) => p.replace(homedir(), '~');

const validEmp = (value) => /^\d{4,12}$/.test(value);
const validProject = (value) => /^[\w.-]+$/.test(value);

export async function run(argv = []) {
  let values;
  try {
    ({ values } = parseArgs({
      args: argv,
      options: {
        emp: { type: 'string' },
        project: { type: 'string' },
        hours: { type: 'string' },
        help: { type: 'boolean', short: 'h', default: false },
      },
    }));
  } catch (err) {
    throw new Error(`${err.message} · timesheet init -h`);
  }

  if (values.help) {
    console.log(usage);
    return 0;
  }

  const exists = configExists();
  const current = exists ? loadConfig() : { ...CONFIG_DEFAULTS };
  const given = values.emp || values.project || values.hours;

  if (exists && !given && !process.stdin.isTTY) {
    ok(`ya configurado ${dim('·')} ${pretty(CONFIG_PATH)}`);
    return 0;
  }

  let empCode = values.emp ?? current.empCode ?? '';
  let projectCode = values.project ?? current.projectCode ?? '';
  let hoursPerDay = values.hours ?? current.hoursPerDay ?? CONFIG_DEFAULTS.hoursPerDay;

  if (!given) {
    const rl = createInterface({ input: process.stdin, output: process.stdout });
    const ask = async (label, fallback) => {
      const suffix = fallback ? ` ${dim(`[${fallback}]`)}` : '';
      const answer = (await rl.question(`  ${label}${suffix}: `)).trim();
      return answer || fallback || '';
    };
    try {
      empCode = await ask('código de empleado', empCode);
      projectCode = await ask('código de proyecto', projectCode);
      hoursPerDay = await ask('horas por día', hoursPerDay);
    } finally {
      rl.close();
    }
  }

  empCode = String(empCode).trim();
  projectCode = String(projectCode).trim();
  hoursPerDay = Number(hoursPerDay);

  if (!validEmp(empCode)) throw new Error('código de empleado inválido');
  if (!validProject(projectCode)) throw new Error('código de proyecto inválido');
  if (!Number.isFinite(hoursPerDay) || hoursPerDay <= 0) throw new Error('horas por día inválidas');

  saveConfig({ ...current, empCode, projectCode, hoursPerDay });
  console.log('');
  ok(`configuración guardada ${dim('·')} ${pretty(CONFIG_PATH)}`);
  console.log(`${dim('→')} timesheet login`);
  return 0;
}

export function main(argv = []) {
  return run(argv).catch((err) => process.exit(reportFatal(err)));
}
