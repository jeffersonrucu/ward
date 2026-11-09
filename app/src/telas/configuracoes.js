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

// Backup externo (Fase 48, V7.9). O rótulo do pendrive vem do udisks2 (não confiável):
// o lado Rust já trocou o invisível por escape (D51); aqui ele é só texto.

/** Nome do pendrive para a tela: o rótulo, ou um texto fixo sem rótulo. */
export function nomeDoPendrive(p) {
  const r = typeof p?.rotulo === 'string' ? p.rotulo : '';
  return r || 'Pendrive sem rótulo';
}

/** "montado" / "conectado, mas não montado", para a lista da escolha. */
export function situacaoNaLista(p) {
  return p?.montado ? 'montado' : 'conectado, mas não montado';
}

/**
 * Textos do cartão do pendrive escolhido: nome, situação (depois do UUID), a linha de
 * estado e se está montado (a cópia só vai para pendrive montado; o Ward não monta).
 */
export function cartaoDoPendrive(externo) {
  const p = externo?.pendrive ?? null;
  if (p?.montado) {
    return {
      nome: nomeDoPendrive(p),
      situacao: 'conectado e montado',
      estado: 'O Ward copia as versões que faltam para ward-backup/, até 30.',
      montado: true,
    };
  }
  if (p) {
    return {
      nome: nomeDoPendrive(p),
      situacao: 'conectado, mas não montado',
      estado: 'Abra o pendrive no gerenciador de arquivos para montá-lo e receber a cópia.',
      montado: false,
    };
  }
  if (externo && externo.pendrives_visiveis === false) {
    return {
      nome: 'Pendrive escolhido',
      situacao: 'não deu para ver os pendrives agora',
      estado: 'O udisks não respondeu. A cópia volta quando ele responder.',
      montado: false,
    };
  }
  return {
    nome: 'Pendrive escolhido',
    situacao: 'não conectado',
    estado: 'A cópia acontece quando ele for conectado e montado.',
    montado: false,
  };
}

/**
 * Fase 49a: a pasta do backup local não abre ou não é segura (tem de ser um diretório
 * só seu, 0700). O daemon grava o cofre mesmo assim, mas sem versão nova.
 */
export const AVISO_BACKUP_LOCAL =
  'A pasta de backups não é segura (tem de ser só sua, sem acesso para outros usuários), ' +
  'então as mudanças estão sendo gravadas sem backup. Confira as permissões de ' +
  '~/.local/share/ward/backups/ (chmod 700).';
