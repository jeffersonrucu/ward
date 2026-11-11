// Rules of the Credentials screen (Phase 32) without Svelte: search, filters, grouping
// and the count texts of the frames 4063 (full), 4410 (filtered) and 4780 (empty).
// Nothing here touches a secret: the items come from the list command of the bridge,
// which has none. The item fields are wire names (m2:Phase 54).

import { TYPES } from '../lib/components/types.js';

export const ENVIRONMENTS = Object.freeze(['dev', 'stage', 'prod']);
export const EMPTY_FILTERS = Object.freeze({ group: '', kind: '', environment: '' });

/** Text to compare: lowercase and without accents. */
export function normalize(text) {
  return String(text ?? '')
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase();
}

function plural(n, one, many) {
  return `${n} ${n === 1 ? one : many}`;
}

export function countEntries(n) {
  return plural(n, 'entrada', 'entradas');
}

/** How many filters of the dialog are on (the search does not count). */
export function activeFilters(f) {
  return [f.group, f.kind, f.environment].filter(Boolean).length;
}

/** The search matches name, host/URL or user (placeholder of the search). */
export function matchesSearch(item, search) {
  const s = normalize(search).trim();
  if (!s) return true;
  return [item.name, item.address, item.username].some((c) => normalize(c).includes(s));
}

export function filterItems(items, filters, search) {
  return items.filter(
    (i) =>
      (!filters.group || i.group === filters.group) &&
      (!filters.kind || i.type === filters.kind) &&
      (!filters.environment || i.environment === filters.environment) &&
      matchesSearch(i, search),
  );
}

function compare(a, b) {
  return a.localeCompare(b, 'pt-BR');
}

/** The groups of all entries, in order, each with its fixed color (1 to 5). */
export function groupsOf(items) {
  const names = [...new Set(items.map((i) => i.group))].sort(compare);
  return names.map((name, k) => ({ name, color: (k % 5) + 1 }));
}

/**
 * Groups with the visible items, in name order. Each group carries the count text
 * (the total, or "visible of total" when there is a search or a filter).
 */
export function groupItems(all, visible, filtering) {
  const total = new Map();
  for (const i of all) total.set(i.group, (total.get(i.group) ?? 0) + 1);
  return groupsOf(all)
    .map((g) => {
      const items = visible.filter((i) => i.group === g.name).sort((a, b) => compare(a.name, b.name));
      const n = total.get(g.name) ?? 0;
      return {
        ...g,
        items,
        count: filtering ? `${items.length} de ${countEntries(n)}` : countEntries(n),
      };
    })
    .filter((g) => g.items.length > 0);
}

/** Summary line at the top. */
export function topSummary(total, groups, visible, filtering) {
  if (total === 0) return 'Nenhuma entrada';
  if (filtering) return `${visible} de ${countEntries(total)} · filtros ativos`;
  return `${countEntries(total)} · ${plural(groups, 'grupo', 'grupos')}`;
}

/** The active filters and the search in one line (the active filters line of the list). */
export function filtersSummary(filters, search) {
  const parts = [];
  if (filters.group) parts.push(`grupo ${filters.group}`);
  if (filters.kind) parts.push(`tipo ${TYPES[filters.kind]?.label ?? filters.kind}`);
  if (filters.environment) parts.push(`ambiente ${filters.environment}`);
  const s = String(search ?? '').trim();
  if (s) parts.push(`busca ${s}`);
  const text = parts.join(' · ');
  return text.charAt(0).toUpperCase() + text.slice(1);
}

// Level card of the form, by the level name of the vault v5 (m2:DT4).
export const LEVELS = Object.freeze({
  'free': {
    label: 'Livre',
    icon: 'check',
    explanation:
      'O Claude usa sem perguntar e só vê a saída mascarada. Mudar o nível pede o PIN.',
  },
  'approval': {
    label: 'Com aprovação',
    icon: 'lock',
    explanation:
      'Cada uso do Claude abre a janela de aprovação, que pede o seu PIN. Mudar o nível também pede o PIN.',
  },
  'blocked': {
    label: 'Bloqueado',
    icon: 'eye-closed',
    explanation:
      'O Claude nem vê esta entrada: ela some do ward list. Só você usa, pelo app. Mudar o nível pede o PIN.',
  },
});
