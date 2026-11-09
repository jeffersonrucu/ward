// Regras locais da tela `Primeiro uso` (Fase 30). São as mesmas de
// `app/nucleo/src/primeiro_uso.rs`, que confere tudo de novo antes do `criar_cofre`:
// aqui servem só para o erro aparecer no campo sem ida ao Rust. Nada daqui guarda o
// valor: as funções recebem o texto e devolvem só mensagem ou nível.

export const MIN_CARACTERES_SENHA = 12;
export const MIN_DIGITOS_PIN = 4;
export const MAX_DIGITOS_PIN = 6;

export const MENSAGENS = Object.freeze({
  senhaCurta: 'A senha mestra precisa de 12 caracteres ou mais.',
  senhaDiferente: 'As duas senhas não são iguais.',
  pinInvalido: 'O PIN tem de ter de 4 a 6 dígitos, só números.',
  pinComLetra: 'Só números: letras e símbolos não entram no PIN.',
  pinLongo: 'O PIN tem no máximo 6 dígitos.',
  pinDiferente: 'Os dois PINs não são iguais.',
});

/** Caracteres (pontos de código), como o `chars().count()` do Rust. */
export function caracteres(texto) {
  return [...texto].length;
}

export function erroDaSenha(senha) {
  return caracteres(senha) < MIN_CARACTERES_SENHA ? MENSAGENS.senhaCurta : '';
}

export function pinValido(pin) {
  return new RegExp(`^[0-9]{${MIN_DIGITOS_PIN},${MAX_DIGITOS_PIN}}$`).test(pin);
}

// Força estimada da senha digitada: tamanho × log2 do alfabeto usado, com os limiares do
// gerador (D65 f: fraca < 60 bits, razoável < 80, forte < 128, muito forte ≥ 128). É só
// uma indicação; a regra que barra é o mínimo de 12 caracteres.
const FORCAS = [
  { ate: 60, codigo: 'fraca', rotulo: 'fraca', segmentos: 1, tom: 'perigo' },
  { ate: 80, codigo: 'razoavel', rotulo: 'razoável', segmentos: 2, tom: 'aviso' },
  { ate: 128, codigo: 'forte', rotulo: 'forte', segmentos: 3, tom: 'sucesso' },
  { ate: Infinity, codigo: 'muito-forte', rotulo: 'muito forte', segmentos: 4, tom: 'sucesso' },
];

export function forcaDaSenha(senha) {
  if (!senha) return null;
  let alfabeto = 0;
  if (/[a-z]/.test(senha)) alfabeto += 26;
  if (/[A-Z]/.test(senha)) alfabeto += 26;
  if (/[0-9]/.test(senha)) alfabeto += 10;
  if (/[\x21-\x2f\x3a-\x40\x5b-\x60\x7b-\x7e ]/.test(senha)) alfabeto += 33;
  if (/[^\x00-\x7f]/.test(senha)) alfabeto += 64;
  const bits = caracteres(senha) * Math.log2(Math.max(alfabeto, 2));
  return FORCAS.find((f) => bits < f.ate);
}

/** Mensagem de erro do Rust (`motivo`, texto fixo) com a primeira letra maiúscula. */
export function mensagemDoRust(motivo) {
  const t = String(motivo ?? '');
  return t ? `${t[0].toUpperCase()}${t.slice(1)}.` : '';
}
