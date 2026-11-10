// Textos e regras da janela de aprovação (Fase 36), com os textos dos frames
// `Aprovação / *` do FigCraft (16701, 16810, 16885, 16960, 17035).

// Mesmo valor de `ward_app_nucleo::aprovacao::SEGUNDOS_URGENTE` (o teste
// `configuracao_tauri` confere).
export const SEGUNDOS_URGENTE = 20;

// PIN de 4 a 6 dígitos (`ward_core::limites`); o lado Rust confere de novo.
export function pinSuficiente(pin) {
  return /^[0-9]{4,6}$/.test(pin);
}

export const ID_NEGAR = 'aprovacao-negar';

// Fase 44c (D9): decisões que não levam PIN. "confirmar" só vale na confirmação da
// origem `http` (o lado Rust e o daemon conferem de novo).
export function pedePin(decisao) {
  return decisao !== 'negar' && decisao !== 'confirmar';
}

// Separador da cadeia ("claude → bash → ward run"). A seta que vier dentro de um nome
// de processo chega do lado Rust como `\u{2192}` (`SEPARADOR_CADEIA`).
export const SEPARADOR = ' \u2192 ';

export const TEXTOS = Object.freeze({
  titulo: 'Pedido de aprovação',
  subtitulo: 'Claude Code pediu uma credencial',
  resumo: 'Confira a entrada, o comando exato e quem pediu antes de permitir.',
  nivel: 'Nível',
  nivelValor: 'com aprovação',
  comando: 'Comando',
  diretorio: 'Diretório de trabalho',
  quemPediu: 'Quem pediu',
  cadeiaVazia: 'processo desconhecido',
  pin: 'PIN',
  pinAjuda: 'Para permitir, digite o PIN. Negar não pede PIN.',
  pinFaltando: 'Digite o PIN para permitir. Negar não pede PIN.',
  pinIncorreto: 'PIN incorreto. Nada foi permitido. Cinco erros seguidos travam o cofre.',
  expirado: 'O pedido já tinha expirado e foi negado.',
  negar: 'Negar',
  umaVez: 'Permitir uma vez',
  umaHora: 'Permitir por 1 hora',
  sempre: 'Permitir sempre',
  fecharENegar: 'Fechar e negar',
  nota:
    'Fechar a janela nega o pedido. Sem resposta em 2 min, ele expira e também é negado. “Permitir sempre” deixa a entrada livre; para voltar atrás, mude o nível na entrada.',
  prodDestaque:
    'Credencial de produção. As opções são as mesmas dos outros ambientes; confira o comando com atenção.',
  expirando: (s) =>
    `O pedido expira em ${s} s. Sem resposta, ele é negado e o Claude recebe “negado”.`,
  bloqueadoDestaque: 'Cinco erros seguidos de PIN. O cofre foi travado e o pedido, negado.',
  bloqueadoResumo: 'O pedido foi negado. Para usar o cofre de novo, destrave com a senha mestra.',
  bloqueadoNota:
    'Os botões de permitir ficam desligados até o cofre ser destravado. Negar ou fechar a janela só confirma o que já aconteceu: o Claude recebeu “negado”.',
  semPedido: 'Nenhum pedido de aprovação esperando.',
  // Fase 44c (D9): origem `http` (localhost/127.0.0.1).
  subtituloPopup: 'A extensão do Chrome quer preencher uma senha',
  origem: 'Origem que recebe usuário e senha',
  origemDestaque:
    'Endereço http desta máquina. Qualquer programa seu pode escutar nele, inclusive no lugar do serviço que você espera. Confirme só se foi você quem pediu agora.',
  resumoConfirmacao: 'Confira a entrada e a origem antes de confirmar.',
  nivelConfirmacao: 'confirmar a origem',
  confirmar: 'Confirmar a origem',
  notaConfirmacao:
    'Confirmar não pede PIN: a entrada não exige. Fechar a janela nega o pedido. Sem resposta em 2 min, ele expira e também é negado.',
});

/** Mensagem no `Campo PIN` para o resultado do `responder_aprovacao`. */
export function mensagemDoResultado(resultado) {
  if (resultado === 'pin-incorreto') return TEXTOS.pinIncorreto;
  if (resultado === 'expirado') return TEXTOS.expirado;
  return '';
}
