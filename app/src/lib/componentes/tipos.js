// Os 9 tipos de entrada (escopo › "Tipos prontos"): rótulo do `Badge tipo` e ícone.
// A chave é o identificador do protocolo (`TipoEntrada::nome` no `ward-core`), o mesmo
// que o daemon manda na lista e no detalhe.
export const TIPOS = Object.freeze({
  login: { rotulo: 'login', icone: 'tipo-login' },
  banco: { rotulo: 'banco', icone: 'tipo-banco' },
  servidor: { rotulo: 'servidor', icone: 'tipo-servidor' },
  api: { rotulo: 'api', icone: 'tipo-api' },
  aws: { rotulo: 'aws', icone: 'tipo-aws' },
  smtp: { rotulo: 'smtp', icone: 'tipo-smtp' },
  'ftp-sftp': { rotulo: 'ftp/sftp', icone: 'tipo-ftp' },
  'nota-segura': { rotulo: 'nota segura', icone: 'tipo-nota' },
  generico: { rotulo: 'genérico', icone: 'tipo-generico' },
});

// Tipo desconhecido (daemon mais novo que a interface): mostra como genérico.
export function tipoDe(id) {
  return Object.hasOwn(TIPOS, id) ? TIPOS[id] : TIPOS.generico;
}
