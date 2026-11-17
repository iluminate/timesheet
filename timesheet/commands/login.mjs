// timesheet login [--timeout N]
// Abre un navegador, espera el login (password + 2FA) y guarda la cookie.
import { chmodSync, mkdirSync, renameSync, writeFileSync } from 'node:fs';
import { parseArgs } from 'node:util';
import { Client, SessionExpired } from '../lib/api.mjs';
import { loadConfig } from '../lib/config.mjs';
import { BROWSER_DIR, COOKIE_PATH, ensureDir } from '../lib/paths.mjs';
import { LOGIN_URL, reportFatal } from '../lib/errors.mjs';
import { dim, ok } from '../lib/output.mjs';
import { log, sleep } from '../lib/log.mjs';

export const usage = `uso: timesheet login [--timeout N]

  --timeout N   minutos de espera para autenticarte (por defecto 10)`;

async function loadPlaywright() {
  try {
    return await import('playwright');
  } catch {
    throw new Error(
      'falta playwright · npm i -g playwright && npx playwright install chromium',
    );
  }
}

export async function run(argv = []) {
  let values;
  try {
    ({ values } = parseArgs({
      args: argv,
      options: {
        timeout: { type: 'string', default: '10' },
        'no-fill': { type: 'boolean', default: false },
        help: { type: 'boolean', short: 'h', default: false },
      },
    }));
  } catch (err) {
    throw new Error(`${err.message} · timesheet login -h`);
  }

  if (values.help) {
    console.log(usage);
    return 0;
  }

  const { empCode } = loadConfig();
  const timeoutMin = Number(values.timeout);
  if (!Number.isFinite(timeoutMin) || timeoutMin <= 0) throw new Error('--timeout inválido');

  const { chromium } = await loadPlaywright();
  ensureDir();
  mkdirSync(BROWSER_DIR, { recursive: true, mode: 0o700 });

  const deadline = Date.now() + timeoutMin * 60 * 1000;
  console.log(`${dim('→')} autentícate en el navegador (máx ${timeoutMin} min)`);

  const context = await chromium.launchPersistentContext(BROWSER_DIR, {
    headless: false,
    viewport: null,
    locale: 'es-PE',
    args: ['--start-maximized'],
  });

  let closed = false;
  context.on('close', () => {
    closed = true;
  });

  const cookieHeader = async () => {
    const cookies = await context.cookies();
    return cookies
      .filter((c) => c.value)
      .map((c) => `${c.name}=${c.value}`)
      .join('; ');
  };

  let saved = false;
  try {
    const page = context.pages()[0] ?? (await context.newPage());
    await page.goto(LOGIN_URL, { waitUntil: 'domcontentloaded', timeout: 60000 });

    while (Date.now() < deadline) {
      if (closed) throw new Error('cerraste el navegador antes de completar la sesión');

      const header = await cookieHeader().catch(() => '');
      if (header) {
        try {
          const probe = new Client({ cookie: header, empCode, retryDelays: [] });
          const periods = await probe.periods(new Date().getFullYear());
          if (periods.length > 0) {
            const tmp = `${COOKIE_PATH}.tmp`;
            writeFileSync(tmp, `${header}\n`, { mode: 0o600 });
            chmodSync(tmp, 0o600);
            renameSync(tmp, COOKIE_PATH);
            saved = true;
            break;
          }
        } catch (err) {
          if (!(err instanceof SessionExpired)) log(`aviso: ${err.message}`);
        }
      }

      await sleep(2000);
    }
  } finally {
    await context.close().catch(() => {});
  }

  if (!saved) throw new Error(`no se detectó la sesión en ${timeoutMin} min`);

  ok('sesión guardada');
  return 0;
}

export function main(argv = []) {
  return run(argv).catch((err) => process.exit(reportFatal(err)));
}
