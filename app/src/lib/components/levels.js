// The three access levels (S9), by the protocol names (the access level name in
// `ward-core`), with the FigCraft texts: `card` is the explanation of the level card
// (18986, 18999, 19011) and `option` the one of the level option in the level dialog
// (989, 999, 1008). `lower` is the label inside a sentence (the "from ... to ..." line
// and the current level line).
// The order is the one of the dialog: blocked, with approval, free.

export const ACCESS_LEVELS = Object.freeze({
  'bloqueado': Object.freeze({
    label: 'Bloqueado',
    lower: 'bloqueado',
    icon: 'eye-closed',
    card:
      'O Claude nem vê esta entrada: ela some do ward lista. Só você usa, pelo app. Mudar o nível pede o PIN.',
    option: 'O Claude nem vê esta entrada: ela some do ward lista. Só você usa, pelo app.',
  }),
  'com-aprovacao': Object.freeze({
    label: 'Com aprovação',
    lower: 'com aprovação',
    icon: 'lock',
    card:
      'Cada uso do Claude abre a janela de aprovação, que pede o seu PIN. Mudar o nível também pede o PIN.',
    option: 'Padrão. Cada uso do Claude abre a janela de aprovação, que pede o seu PIN.',
  }),
  'livre': Object.freeze({
    label: 'Livre',
    lower: 'livre',
    icon: 'check',
    card: 'O Claude usa sem perguntar e só vê a saída mascarada. Mudar o nível pede o PIN.',
    option: 'O Claude usa sem perguntar e continua sem ver o segredo: a saída volta mascarada.',
  }),
});

export const LEVEL_ORDER = Object.freeze(['bloqueado', 'com-aprovacao', 'livre']);

/** The level by the protocol name, or `undefined` if the name is not one of the three. */
export function accessLevel(name) {
  return Object.hasOwn(ACCESS_LEVELS, name) ? ACCESS_LEVELS[name] : undefined;
}
