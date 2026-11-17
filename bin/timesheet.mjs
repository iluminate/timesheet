#!/usr/bin/env node
// Punto de entrada del CLI: elige el comando y delega en timesheet/commands.
import { reportFatal } from '../timesheet/lib/errors.mjs';
import { run as status } from '../timesheet/commands/status.mjs';
import { run as list } from '../timesheet/commands/list.mjs';
import { run as fill } from '../timesheet/commands/fill.mjs';
import { run as submit } from '../timesheet/commands/submit.mjs';
import { run as login } from '../timesheet/commands/login.mjs';
import { run as init } from '../timesheet/commands/init.mjs';

const VERSION = typeof __VERSION__ !== 'undefined' ? __VERSION__ : 'dev';

const COMMANDS = { status, list, fill, submit, login, init };

const USAGE = `uso: timesheet [comando] [opciones]

  timesheet              estado del periodo actual y pendientes
  timesheet list         periodos recientes
  timesheet fill         rellena un periodo pendiente
  timesheet submit       envía un periodo a aprobación
  timesheet login        renueva la sesión de iEngage
  timesheet init         configuración inicial

Opciones globales:
  -h, --help             muestra esta ayuda
  -V, --version          muestra la versión

Cada comando acepta -h para ver su ayuda.`;

async function dispatch(argv) {
  const [first, ...rest] = argv;

  if (!first) return status([]);
  if (first === '--help' || first === '-h') {
    console.log(USAGE);
    return 0;
  }
  if (first === '--version' || first === '-V') {
    console.log(`timesheet ${VERSION}`);
    return 0;
  }
  if (first.startsWith('-')) return status(argv);

  const run = COMMANDS[first];
  if (!run) {
    console.error(`comando desconocido: ${first}`);
    console.error(USAGE);
    return 1;
  }
  return run(rest);
}

dispatch(process.argv.slice(2))
  .then((code) => {
    if (typeof code === 'number') process.exitCode = code;
  })
  .catch((err) => {
    process.exitCode = reportFatal(err);
  });
