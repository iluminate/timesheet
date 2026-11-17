// Resuelve el argumento de periodo: current | 2026-11-01 | 01-Nov-2026
import { parseAny } from './dates.mjs';

const CURRENT = new Set(['current', 'now', 'today', '.', '-0']);
const ISO = /^\d{4}-\d{2}-\d{2}$/;
const API = /^\d{1,2}-[A-Za-z]{3}-\d{4}$/;

export async function resolvePeriodValue(client, today, value) {
  const v = String(value ?? '').trim();
  if (CURRENT.has(v.toLowerCase())) return client.currentPeriod(today);
  if (!ISO.test(v) && !API.test(v)) {
    throw new Error(`periodo inválido: ${v} · usa 2026-11-01`);
  }
  return client.currentPeriod(parseAny(v));
}
