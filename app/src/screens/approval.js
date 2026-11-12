// Texts and rules of the approval window (Phase 36), with the texts of the approval
// frames in FigCraft (16701, 16810, 16885, 16960, 17035).

// Same value as the urgent threshold of the approval module of `ward_app_core` (the Tauri
// config test checks it).
export const URGENT_SECONDS = 20;

// PIN of 4 to 6 digits (the limits of `ward_core`); the Rust side checks it again.
export function pinSufficient(pin) {
  return /^[0-9]{4,6}$/.test(pin);
}

export const DENY_ID = 'approval-deny';

// Phase 44c (D9): decisions that take no PIN. The confirm decision only applies to the
// confirmation of an `http` origin (the Rust side and the daemon check it again).
export function needsPin(decision) {
  return decision !== 'deny' && decision !== 'confirm';
}

// Separator of the process chain ("claude → bash → ward run"). An arrow inside a process
// name reaches us from the Rust side as `\u{2192}` (its chain separator constant).
export const SEPARATOR = ' \u2192 ';

export const TEXTS = Object.freeze({
  title: 'Pedido de aprovação',
  subtitle: 'Claude Code pediu uma credencial',
  summary: 'Confira a entrada, o comando exato e quem pediu antes de permitir.',
  level: 'Nível',
  levelValue: 'com aprovação',
  command: 'Comando',
  directory: 'Diretório de trabalho',
  requestedBy: 'Quem pediu',
  emptyChain: 'processo desconhecido',
  pin: 'PIN',
  pinHelp: 'Para permitir, digite o PIN. Negar não pede PIN.',
  pinMissing: 'Digite o PIN para permitir. Negar não pede PIN.',
  pinIncorrect: 'PIN incorreto. Nada foi permitido. Cinco erros seguidos travam o cofre.',
  expired: 'O pedido já tinha expirado e foi negado.',
  deny: 'Negar',
  once: 'Permitir uma vez',
  oneHour: 'Permitir por 1 hora',
  always: 'Permitir sempre',
  closeAndDeny: 'Fechar e negar',
  note:
    'Fechar a janela nega o pedido. Sem resposta em 2 min, ele expira e também é negado. “Permitir sempre” deixa a entrada livre; para voltar atrás, mude o nível na entrada.',
  prodHighlight:
    'Credencial de produção. As opções são as mesmas dos outros ambientes; confira o comando com atenção.',
  expiring: (s) =>
    `O pedido expira em ${s} s. Sem resposta, ele é negado e o Claude recebe “negado”.`,
  blockedHighlight: 'Cinco erros seguidos de PIN. O cofre foi travado e o pedido, negado.',
  blockedSummary: 'O pedido foi negado. Para usar o cofre de novo, destrave com a senha mestra.',
  blockedNote:
    'Os botões de permitir ficam desligados até o cofre ser destravado. Negar ou fechar a janela só confirma o que já aconteceu: o Claude recebeu “negado”.',
  noRequest: 'Nenhum pedido de aprovação esperando.',
  // Phase 44d/44e: texts of the confirmation and of the PIN request with an origin
  // (frames 22733, 22817, 22882): those requests have no command.
  prodHighlightOrigin: 'Credencial de produção. Confira a origem com atenção antes de confirmar.',
  originSummary: 'Confira a entrada, a origem e quem pediu antes de permitir.',
  // Phase 44c (D9): `http` origin (localhost/127.0.0.1). Phase 44e: the same applies to
  // every loopback host over `https` (`originHighlightLocal`; no frame of its own).
  popupSubtitle: 'A extensão do Chrome quer preencher uma senha',
  origin: 'Origem que recebe usuário e senha',
  originHighlight:
    'Endereço http desta máquina. Qualquer programa seu pode escutar nele, inclusive no lugar do serviço que você espera. Confirme só se foi você quem pediu agora.',
  originHighlightLocal:
    'Endereço desta máquina. Qualquer programa seu pode escutar nele, inclusive no lugar do serviço que você espera. Confirme só se foi você quem pediu agora.',
  confirmationSummary: 'Confira a entrada e a origem antes de confirmar.',
  confirmationLevel: 'confirmar a origem',
  confirm: 'Confirmar a origem',
  // Phase 54b (D68): the command's stdin is a pipe or a file (frames 22964 and 23051 in
  // FigCraft; text decided by the user, 2026-11-09).
  stdinHighlight:
    'O comando recebe dados pela entrada padrão, que esta janela não mostra. Esses dados podem usar a credencial do jeito que quiserem. Permita só se você sabe o que vai por ela.',
  stdinAcknowledged: 'Sei o que vai pela entrada padrão e permito mesmo assim',
  stdinMissing: 'Marque a opção da entrada padrão para permitir. Negar não pede nada.',
  confirmationNote:
    'Confirmar não pede PIN: a entrada não exige. Fechar a janela nega o pedido. Sem resposta em 2 min, ele expira e também é negado.',
});

/** The origin warning: the frame's text for `http`; for loopback `https` (Phase 44e),
 * the same text without the "http". */
export function originNotice(origin) {
  let http = false;
  try {
    http = new URL(origin).protocol === 'http:';
  } catch {
    http = false;
  }
  return http ? TEXTS.originHighlight : TEXTS.originHighlightLocal;
}

/** Message in the PIN field (FigCraft 650) for the result of the approval answer. */
export function resultMessage(result) {
  if (result === 'wrong-pin') return TEXTS.pinIncorrect;
  if (result === 'expired') return TEXTS.expired;
  return '';
}
