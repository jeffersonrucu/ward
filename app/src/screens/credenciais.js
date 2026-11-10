// Regras da tela Credenciais (Fase 32) sem Svelte: busca, filtros, agrupamento e os
// textos de contagem dos frames `Credenciais / Cheia`, `/ Filtrada` e `/ Vazia`.
// Nada daqui toca em segredo: os itens vêm de `listar_credenciais`, que não tem.

import { TYPES } from '../lib/components/types.js';

export const AMBIENTES = Object.freeze(['dev', 'stage', 'prod']);
export const FILTROS_VAZIOS = Object.freeze({ grupo: '', tipo: '', ambiente: '' });

/** Texto para comparar: minúsculas e sem acento. */
export function normalizar(texto) {
  return String(texto ?? '')
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase();
}

function plural(n, um, varios) {
  return `${n} ${n === 1 ? um : varios}`;
}

export function contarEntradas(n) {
  return plural(n, 'entrada', 'entradas');
}

/** Quantos filtros do diálogo estão ligados (a busca não conta). */
export function filtrosLigados(f) {
  return [f.grupo, f.tipo, f.ambiente].filter(Boolean).length;
}

/** A busca casa com nome, host/URL ou usuário (placeholder da busca). */
export function casaBusca(item, busca) {
  const b = normalizar(busca).trim();
  if (!b) return true;
  return [item.nome, item.endereco, item.usuario].some((c) => normalizar(c).includes(b));
}

export function filtrar(itens, filtros, busca) {
  return itens.filter(
    (i) =>
      (!filtros.grupo || i.grupo === filtros.grupo) &&
      (!filtros.tipo || i.tipo === filtros.tipo) &&
      (!filtros.ambiente || i.ambiente === filtros.ambiente) &&
      casaBusca(i, busca),
  );
}

function comparar(a, b) {
  return a.localeCompare(b, 'pt-BR');
}

/** Os grupos de todas as entradas, em ordem, cada um com a cor fixa (1 a 5). */
export function gruposDe(itens) {
  const nomes = [...new Set(itens.map((i) => i.grupo))].sort(comparar);
  return nomes.map((nome, k) => ({ nome, cor: (k % 5) + 1 }));
}

/**
 * Grupos com os itens visíveis, em ordem de nome. Cada grupo traz a contagem
 * ("12 entradas" ou "2 de 12 entradas" quando há busca ou filtro).
 */
export function agrupar(todos, visiveis, filtrando) {
  const total = new Map();
  for (const i of todos) total.set(i.grupo, (total.get(i.grupo) ?? 0) + 1);
  return gruposDe(todos)
    .map((g) => {
      const itens = visiveis.filter((i) => i.grupo === g.nome).sort((a, b) => comparar(a.nome, b.nome));
      const n = total.get(g.nome) ?? 0;
      return {
        ...g,
        itens,
        contagem: filtrando ? `${itens.length} de ${contarEntradas(n)}` : contarEntradas(n),
      };
    })
    .filter((g) => g.itens.length > 0);
}

/** Linha de resumo do topo. */
export function resumoDoTopo(total, grupos, visiveis, filtrando) {
  if (total === 0) return 'Nenhuma entrada';
  if (filtrando) return `${visiveis} de ${contarEntradas(total)} · filtros ativos`;
  return `${contarEntradas(total)} · ${plural(grupos, 'grupo', 'grupos')}`;
}

/** "Tipo banco · ambiente stage · busca pg" (linha `filtros ativos` da lista). */
export function resumoDosFiltros(filtros, busca) {
  const partes = [];
  if (filtros.grupo) partes.push(`grupo ${filtros.grupo}`);
  if (filtros.tipo) partes.push(`tipo ${TYPES[filtros.tipo]?.label ?? filtros.tipo}`);
  if (filtros.ambiente) partes.push(`ambiente ${filtros.ambiente}`);
  const b = String(busca ?? '').trim();
  if (b) partes.push(`busca ${b}`);
  const texto = partes.join(' · ');
  return texto.charAt(0).toUpperCase() + texto.slice(1);
}

export const NIVEIS = Object.freeze({
  livre: {
    rotulo: 'Livre',
    icone: 'check',
    explicacao:
      'O Claude usa sem perguntar e só vê a saída mascarada. Mudar o nível pede o PIN.',
  },
  'com-aprovacao': {
    rotulo: 'Com aprovação',
    icone: 'lock',
    explicacao:
      'Cada uso do Claude abre a janela de aprovação, que pede o seu PIN. Mudar o nível também pede o PIN.',
  },
  bloqueado: {
    rotulo: 'Bloqueado',
    icone: 'eye-closed',
    explicacao:
      'O Claude nem vê esta entrada: ela some do ward lista. Só você usa, pelo app. Mudar o nível pede o PIN.',
  },
});
