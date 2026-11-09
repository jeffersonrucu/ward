// Regras locais da tela `Configurações` (Fase 38). As de PIN e senha mestra são as do
// primeiro uso (`primeiro-uso.js`); o Rust (`app/nucleo/src/configuracoes.rs`) confere
// tudo de novo antes de pedir ao daemon. Nada daqui guarda valor digitado.

// Espelho de `ward_core::limites::{MIN_MINUTOS_TRAVA, MAX_MINUTOS_TRAVA}` (D45).
export const MIN_MINUTOS_TRAVA = 1;
export const MAX_MINUTOS_TRAVA = 1440;

// O select do design (`Configurações / Geral`, 15313) diz "máximo 4 h": as opções vão
// até 4 horas, o padrão de D30.
export const OPCOES_TRAVA = Object.freeze([15, 30, 60, 120, 240]);

/** "15 minutos", "1 hora", "4 horas", "1 h 30 min". */
export function rotuloMinutos(minutos) {
  const m = Number(minutos);
  if (m < 60) return `${m} ${m === 1 ? 'minuto' : 'minutos'}`;
  const h = Math.floor(m / 60);
  const resto = m % 60;
  if (resto) return `${h} h ${resto} min`;
  return `${h} ${h === 1 ? 'hora' : 'horas'}`;
}

/**
 * Opções do select de trava. Um valor gravado fora da lista (outro cliente, ou um cofre
 * antigo) aparece também, para o select não mentir sobre o que está gravado.
 */
export function opcoesDeTrava(atual) {
  const lista = [...OPCOES_TRAVA];
  const m = Number(atual);
  if (Number.isInteger(m) && m >= MIN_MINUTOS_TRAVA && m <= MAX_MINUTOS_TRAVA && !lista.includes(m)) {
    lista.push(m);
    lista.sort((a, b) => a - b);
  }
  return lista.map((v) => ({ valor: String(v), rotulo: rotuloMinutos(v) }));
}
