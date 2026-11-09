// Os três níveis de acesso (S9), pelos nomes do protocolo (`NivelAcesso::nome`), com os
// textos do FigCraft: `cartao` é a explicação do `Cartão de nível` (18986, 18999, 19011)
// e `opcao` a da `Opção de nível` no `Diálogo de nível` (989, 999, 1008). `minusculo` é o
// rótulo dentro da frase ("De com aprovação para livre", "Nível atual: …").
// A ordem é a do diálogo: bloqueado, com aprovação, livre.

export const NIVEIS_DE_ACESSO = Object.freeze({
  bloqueado: Object.freeze({
    rotulo: 'Bloqueado',
    minusculo: 'bloqueado',
    icone: 'olho-fechado',
    cartao:
      'O Claude nem vê esta entrada: ela some do ward lista. Só você usa, pelo app. Mudar o nível pede o PIN.',
    opcao: 'O Claude nem vê esta entrada: ela some do ward lista. Só você usa, pelo app.',
  }),
  'com-aprovacao': Object.freeze({
    rotulo: 'Com aprovação',
    minusculo: 'com aprovação',
    icone: 'cadeado',
    cartao:
      'Cada uso do Claude abre a janela de aprovação, que pede o seu PIN. Mudar o nível também pede o PIN.',
    opcao: 'Padrão. Cada uso do Claude abre a janela de aprovação, que pede o seu PIN.',
  }),
  livre: Object.freeze({
    rotulo: 'Livre',
    minusculo: 'livre',
    icone: 'check',
    cartao: 'O Claude usa sem perguntar e só vê a saída mascarada. Mudar o nível pede o PIN.',
    opcao: 'O Claude usa sem perguntar e continua sem ver o segredo: a saída volta mascarada.',
  }),
});

export const ORDEM_DOS_NIVEIS = Object.freeze(['bloqueado', 'com-aprovacao', 'livre']);

/** O nível pelo nome do protocolo, ou `undefined` se o nome não for um dos três. */
export function nivelDe(nome) {
  return Object.hasOwn(NIVEIS_DE_ACESSO, nome) ? NIVEIS_DE_ACESSO[nome] : undefined;
}
