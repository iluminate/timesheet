// Catalogo de periodos (semanas) que devuelve iEngage.
import { fromApi } from './dates.mjs';

/** Todos los periodos del año actual y el anterior, ordenados. */
export async function periodCatalog(client, today) {
  const year = new Date(today).getUTCFullYear();
  const byStart = new Map();

  for (const y of [year - 1, year]) {
    for (const p of await client.periods(y)) {
      const [s, e] = String(p.Value ?? '')
        .split(' - ')
        .map((v) => v.trim());
      if (!s || !e) continue;
      byStart.set(s, { startApi: s, endApi: e, start: fromApi(s), end: fromApi(e) });
    }
  }

  return [...byStart.values()].sort((a, b) => a.start - b.start);
}

/** Los `count` periodos inmediatamente anteriores a `current`, del mas nuevo al mas viejo. */
export async function previousPeriods(client, current, today, count) {
  if (count <= 0) return [];
  const all = await periodCatalog(client, today);
  const index = all.findIndex((p) => p.startApi === current.startApi);
  if (index < 0) return [];
  return all.slice(Math.max(0, index - count), index).reverse();
}
