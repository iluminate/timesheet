// Errores en una sola linea, siempre con la salida.
import { NetworkError, SessionExpired } from './api.mjs';
import { cyan, dim, fail } from './output.mjs';

export const TIMESHEET_URL =
  'https://iengage.coforge.com/ess2/Timesheet/TimesheetHandler/MyTimesheet';
export const LOGIN_URL = 'https://iengage.coforge.com/ess2/login';

/**
 * Imprime el error y devuelve el codigo de salida.
 *   2 = sesion vencida, 3 = sin red, 1 = otros
 */
export function reportFatal(err) {
  if (err instanceof SessionExpired) {
    fail(`sesión expirada ${dim('·')} ${cyan('timesheet login')}`);
    return 2;
  }
  if (err instanceof NetworkError) {
    fail(`sin conexión ${dim('·')} ${err.detail}`);
    return 3;
  }
  const message = String(err?.message ?? err).replace(/\s+/g, ' ').trim();
  fail(message);
  return 1;
}
