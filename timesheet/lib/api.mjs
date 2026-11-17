import { readFileSync } from 'node:fs';
import { fromApi } from './dates.mjs';
import { log, sleep } from './log.mjs';

const DEFAULT_BASE = 'https://iengage.coforge.com/ess2/Timesheet/TimesheetHandler';
const UA =
  'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36';

/** La sesion murio o el servidor nos mando al login/error. */
export class SessionExpired extends Error {
  constructor(detail) {
    super(detail || 'sesion no valida');
    this.name = 'SessionExpired';
    this.detail = detail || '';
  }
}

/** No hubo red (tipico al reanudar el equipo antes de que levante el Wi-Fi). */
export class NetworkError extends Error {
  constructor(detail) {
    super(detail);
    this.name = 'NetworkError';
    this.detail = detail;
  }
}

/** Error de negocio. */
export class ApiError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ApiError';
  }
}

export function loadCookie(path) {
  let raw;
  try {
    raw = readFileSync(path, 'utf8');
  } catch {
    throw new ApiError(`No existe el archivo de cookie: ${path}`);
  }
  const line = raw
    .split(/\r?\n/)
    .map((s) => s.trim())
    .find((s) => s && !s.startsWith('#'));
  if (!line) throw new ApiError(`El archivo de cookie esta vacio: ${path}`);
  if (/^cookie:/i.test(line)) return line.slice(7).trim();
  return line;
}

const clean = (s) => String(s ?? '').replace(/\s+/g, ' ').trim();

/** Esperas entre reintentos cuando falla la red. */
const RETRY_DELAYS = [15000, 30000, 60000, 120000];

export class Client {
  constructor({ cookie, empCode, baseUrl = DEFAULT_BASE, retryDelays = RETRY_DELAYS }) {
    this.baseUrl = baseUrl.replace(/\/+$/, '');
    this.cookie = cookie;
    this.empCode = empCode;
    this.retryDelays = retryDelays;
  }

  async post(path, body) {
    for (let attempt = 0; ; attempt += 1) {
      try {
        return await this.#request(path, body);
      } catch (err) {
        const canRetry = err instanceof NetworkError && attempt < this.retryDelays.length;
        if (!canRetry) throw err;
        const wait = this.retryDelays[attempt];
        log(
          `sin red (${err.detail}); reintento ${attempt + 1}/${this.retryDelays.length} en ${Math.round(wait / 1000)}s`,
        );
        await sleep(wait);
      }
    }
  }

  async #request(path, body) {
    let res;
    try {
      res = await fetch(`${this.baseUrl}/${path}`, {
        method: 'POST',
        headers: {
          accept: 'application/json, text/javascript, */*; q=0.01',
          'accept-language': 'es-419,es;q=0.9',
          'content-type': 'application/json; charset=UTF-8',
          cookie: this.cookie,
          origin: 'https://iengage.coforge.com',
          referer: 'https://iengage.coforge.com/',
          'x-requested-with': 'XMLHttpRequest',
          'user-agent': UA,
        },
        body: JSON.stringify(body),
        redirect: 'manual',
        signal: AbortSignal.timeout(30000),
      });
    } catch (err) {
      const code = err?.cause?.code ?? err?.code ?? err?.name ?? 'error';
      throw new NetworkError(`${code}: ${err.message}`);
    }

    const location = res.headers.get('location') ?? '';
    const type = res.headers.get('content-type') ?? '';

    if (res.status >= 300 && res.status < 400) {
      throw new SessionExpired(`redireccion ${res.status}${location ? ` -> ${location}` : ''}`);
    }
    if (!type.includes('application/json')) {
      throw new SessionExpired(`respuesta no JSON (HTTP ${res.status}, ${type || 'sin content-type'})`);
    }
    if (!res.ok) throw new ApiError(`${path}: HTTP ${res.status}`);

    const text = await res.text();
    let json;
    try {
      json = JSON.parse(text);
    } catch {
      throw new SessionExpired(`${path}: respuesta ilegible`);
    }

    // iEngage responde HTTP 200 con un sobre de error cuando la sesion caduca:
    //   { "Exception": "... due to Timeout", "StatusCode": 500 }
    const status = Number(json?.StatusCode);
    if (Number.isFinite(status) && status >= 400) {
      throw new SessionExpired(clean(json.Exception) || clean(json.ErrorMsg) || `StatusCode ${status}`);
    }
    if (!json || !('Result' in json)) {
      throw new SessionExpired(clean(json?.Exception) || 'respuesta sin Result');
    }

    return json;
  }

  /** Lista de periodos ("22-Oct-2026 - 31-Oct-2026"). */
  async periods(year) {
    const out = await this.post('BindControls', { _InputData: { Type: 1, Year: String(year) } });
    return (out.Result ?? []).filter((p) => String(p.SearchText ?? '').trim().toUpperCase() === 'WEEKS');
  }

  /** Periodo que contiene `todayMs`. */
  async currentPeriod(todayMs) {
    const year = new Date(todayMs).getUTCFullYear();
    for (const y of [year, year - 1, year + 1]) {
      for (const p of await this.periods(y)) {
        const [s, e] = String(p.Value ?? '')
          .split(' - ')
          .map((v) => v.trim());
        if (!s || !e) continue;
        if (todayMs >= fromApi(s) && todayMs <= fromApi(e)) {
          return { startApi: s, endApi: e, start: fromApi(s), end: fromApi(e), selected: p.Selected };
        }
      }
    }
    throw new ApiError(`no hay periodo para ${new Date(todayMs).toISOString().slice(0, 10)}`);
  }

  /** Datos del empleado: TID, status, supervisor, minimos/maximos. */
  async employee(startApi, endApi) {
    const out = await this.post('GetEmpDetails', {
      _InputData: { Type: 2, StartDate: startApi, EndDate: endApi, EmpCode: this.empCode },
    });
    const emp = out.Result?.[0];
    if (!emp) throw new ApiError('GetEmpDetails no devolvio datos del empleado.');
    return emp;
  }

  /** Calendario del periodo: DayID (1 laborable, 2 finde, 4 feriado, 5 medio dia). */
  async days(startApi, endApi) {
    const out = await this.post('GetMyTimesheetList', {
      _InputData: { Type: 3, StartDate: startApi, EndDate: endApi },
    });
    return out.Result ?? [];
  }

  /** Filas de proyecto con las horas ya registradas. */
  async grid(startApi, endApi) {
    const out = await this.post('GetEmployeeTimesheetData', {
      _InputData: { Type: 4, StartDate: startApi, EndDate: endApi },
    });
    return out.Result ?? [];
  }

  /** Save o Submit. `entries` = grid completo. */
  async persist(entries, actionType) {
    const out = await this.post('SubmitTimesheetDetails', {
      _submitData: entries,
      _actionType: actionType,
      _actionTakenBy: 'User',
      _actionTakenFrom: 'Web',
    });
    const output = String(out.Result?.[0]?.OUTPUT ?? '').trim().toUpperCase();
    if (output !== 'SUCCESS') {
      throw new ApiError(`${actionType} no confirmado por el servidor (OUTPUT=${output || 'vacio'}).`);
    }
    return out;
  }
}
