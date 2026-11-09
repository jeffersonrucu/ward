// Formulário de credencial (Fase 33): os campos de cada tipo, como o FigCraft os desenha
// (frames `Formulário / <tipo> / Novo` e `/ Edição`, docs/mission/design-nodes.md), e a
// montagem do pedido `salvar_credencial`.
//
// S11: o formulário nunca recebe segredo do cofre. Segredo salvo vai como "manter" (o
// Rust o põe no rascunho), senha do gerador vai como "gerada" (idem), e só o que o
// usuário digitou vai como `{ novo }`.

export const AMBIENTES_FORM = Object.freeze([
  { valor: '', rotulo: 'Escolha o ambiente' },
  { valor: 'dev', rotulo: 'dev' },
  { valor: 'stage', rotulo: 'stage' },
  { valor: 'prod', rotulo: 'prod' },
]);

// Regiões da AWS oferecidas no select (a validação do `ward-core` aceita qualquer região
// no formato; a da entrada editada entra na lista se não estiver nela).
export const REGIOES_AWS = Object.freeze([
  'us-east-1',
  'us-east-2',
  'us-west-1',
  'us-west-2',
  'sa-east-1',
  'ca-central-1',
  'eu-west-1',
  'eu-west-2',
  'eu-west-3',
  'eu-central-1',
  'eu-north-1',
  'eu-south-1',
  'ap-south-1',
  'ap-southeast-1',
  'ap-southeast-2',
  'ap-northeast-1',
  'ap-northeast-2',
  'ap-northeast-3',
  'af-south-1',
  'me-south-1',
]);

export const SEGURANCAS = Object.freeze([
  { valor: 'starttls', rotulo: 'STARTTLS' },
  { valor: 'tls', rotulo: 'TLS' },
  { valor: 'nenhuma', rotulo: 'Nenhuma' },
]);

// Cada linha da grade: um campo inteiro ou dois lado a lado. `tipo`: texto, segredo,
// select, totp (o `Código TOTP`) ou nota (o texto da nota segura).
const T = (id, rotulo, extra = {}) => ({ id, rotulo, tipo: 'texto', ...extra });
const S = (id, rotulo, extra = {}) => ({ id, rotulo, tipo: 'segredo', ...extra });

export const TIPOS_FORM = Object.freeze({
  login: {
    acesso: 'Entregues como WARD_URL, WARD_USER, WARD_PASS e WARD_TOTP.',
    linhas: [
      [T('url', 'URL', { placeholder: 'Endereço com http ou https', url: true })],
      [T('usuario', 'Usuário', { placeholder: 'Ex.: wp_admin' }), S('senha', 'Senha', { gerar: true })],
      [S('totp_semente', 'Semente TOTP'), { id: 'codigo', tipo: 'totp' }],
    ],
  },
  api: {
    acesso: 'Entregues como WARD_URL, WARD_TOKEN e a variável escolhida.',
    linhas: [
      [T('url', 'URL base', { placeholder: 'Endereço com http ou https', url: true })],
      [S('token', 'Token')],
      [
        T('variavel', 'Nome da variável', {
          placeholder: 'Ex.: GITLAB_TOKEN',
          ajuda: 'O token vai nesta variável e em WARD_TOKEN.',
        }),
      ],
    ],
  },
  aws: {
    acesso:
      'Entregues como AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION e AWS_SESSION_TOKEN.',
    linhas: [
      [T('access_key_id', 'Access key ID', { placeholder: 'Ex.: AKIA…' })],
      [S('secret_access_key', 'Secret access key')],
      [
        { id: 'regiao', rotulo: 'Região', tipo: 'select', opcoes: 'regioes' },
        S('session_token', 'Session token (opcional)'),
      ],
      [T('role_arn', 'Role ARN (opcional)', { placeholder: 'arn:aws:iam::…' })],
    ],
  },
  smtp: {
    acesso: 'Entregues como SMTP_HOST, SMTP_PORT, SMTP_USER, SMTP_PASS e SMTP_SECURITY.',
    linhas: [
      [
        T('host', 'Host', { placeholder: 'Ex.: smtp.exemplo.com' }),
        T('porta', 'Porta', { placeholder: '587', modo: 'numeric' }),
      ],
      [
        T('usuario', 'Usuário', { placeholder: 'Ex.: mailer' }),
        { id: 'seguranca', rotulo: 'Segurança', tipo: 'select', opcoes: 'segurancas' },
      ],
      [S('senha', 'Senha', { gerar: true })],
    ],
  },
  'nota-segura': {
    acesso: 'Sem entrega: o ward run recusa este tipo e o Claude nunca recebe o texto.',
    tituloAcesso: 'Conteúdo',
    linhas: [[{ id: 'texto', rotulo: 'Nota', tipo: 'nota' }]],
  },
  generico: { acesso: '', linhas: [] },
});

export const TEXTOS_EXTRAS = Object.freeze({
  'nota-segura': 'Ficam com a nota; nota segura não tem entrega.',
  generico: 'Só estes campos são entregues, como WARD_<NOME>.',
  padrao: 'Viram WARD_<NOME> no ward run.',
});

export const TEXTOS_ANEXOS = Object.freeze({
  'nota-segura': 'Ficam cifrados com a nota; sem entrega no ward run.',
  padrao: 'Viram WARD_ARQUIVO_<NOME> no ward run.',
});

// Cartões de `Nova credencial / Escolher tipo` (17657). Os três tipos da Fase 34 ficam
// desabilitados até lá.
export const CARTOES_TIPO = Object.freeze([
  { tipo: 'login', nome: 'login', descricao: 'URL, usuário, senha e TOTP' },
  { tipo: 'banco', nome: 'banco', descricao: 'Motor, host, porta, base e senha', depois: true },
  { tipo: 'servidor', nome: 'servidor', descricao: 'SSH com senha ou chave', depois: true },
  { tipo: 'api', nome: 'api', descricao: 'URL base, token e variável' },
  { tipo: 'aws', nome: 'aws', descricao: 'Access key, secret, região e role' },
  { tipo: 'smtp', nome: 'smtp', descricao: 'Envio de e-mail com usuário e senha' },
  { tipo: 'ftp-sftp', nome: 'ftp/sftp', descricao: 'Arquivos por senha ou chave', depois: true },
  { tipo: 'nota-segura', nome: 'nota segura', descricao: 'Texto secreto, sem entrega' },
  { tipo: 'generico', nome: 'genérico', descricao: 'Só campos extras, do seu jeito' },
]);

export function camposDoTipo(tipo) {
  return (TIPOS_FORM[tipo]?.linhas ?? []).flat();
}

/** Texto como o cofre guarda: NFC (o Rust valida de novo). */
export function nfc(v) {
  return typeof v === 'string' ? v.normalize('NFC') : '';
}

/**
 * URL com o host em punycode (IDN), como o `new URL` do navegador o escreve; o resto
 * fica como foi digitado. Texto que não é URL vai como está (o Rust dá o erro).
 */
export function urlComPunycode(v) {
  const texto = nfc(v).trim();
  try {
    const u = new URL(texto);
    if (!/[^\x00-\x7F]/.test(texto) || !u.hostname) return texto;
    const inicio = texto.indexOf('//') + 2;
    const resto = texto.slice(inicio);
    const fim = resto.search(/[/?#]/);
    const autoridade = fim === -1 ? resto : resto.slice(0, fim);
    const arroba = autoridade.lastIndexOf('@');
    const antes = arroba === -1 ? '' : autoridade.slice(0, arroba + 1);
    const hostPorta = autoridade.slice(arroba + 1);
    const porta = /:\d+$/.exec(hostPorta)?.[0] ?? '';
    return texto.slice(0, inicio) + antes + u.hostname + porta + resto.slice(autoridade.length);
  } catch {
    return texto;
  }
}

/** Tags digitadas ("wordpress, painel") como lista, sem vazias. */
export function tagsDe(texto) {
  return nfc(texto)
    .split(',')
    .map((t) => t.trim())
    .filter((t) => t !== '');
}

/**
 * Corpo do `salvar_credencial`. `segredos[id]`: `{ modo, digitado }` com modo salvo,
 * gerada ou digitando; `extras`: `{ nome, valor, segredo, guardado, original }`.
 */
export function montarFormulario(tipo, estado) {
  const campos = {};
  const segredos = {};
  for (const c of camposDoTipo(tipo)) {
    if (c.tipo === 'texto' || c.tipo === 'select') {
      const v = estado.campos[c.id] ?? '';
      campos[c.id] = c.url ? urlComPunycode(v) : nfc(v).trim();
    } else if (c.tipo === 'segredo' || c.tipo === 'nota') {
      const s = estado.segredos[c.id];
      if (!s) segredos[c.id] = 'vazio';
      else if (s.modo === 'salvo') segredos[c.id] = 'manter';
      else if (s.modo === 'gerada') segredos[c.id] = 'gerada';
      else if (s.digitado === '') segredos[c.id] = 'vazio';
      else segredos[c.id] = { novo: c.tipo === 'nota' ? nfc(s.digitado) : s.digitado };
    }
  }
  return {
    tipo,
    nome: nfc(estado.nome).trim(),
    grupo: nfc(estado.grupo).trim(),
    ambiente: estado.ambiente,
    projeto: nfc(estado.projeto).trim(),
    tags: tagsDe(estado.tags),
    campos,
    segredos,
    extras: estado.extras.map((x) => ({
      nome: nfc(x.nome).trim(),
      segredo: x.segredo,
      valor: x.guardado ? { manter: x.original } : { novo: x.segredo ? x.valor : nfc(x.valor) },
    })),
  };
}

/** Erros por campo (`{ campo, posicao, motivo }`) num mapa para a tela. */
export function errosPorCampo(erros) {
  const mapa = {};
  for (const e of erros ?? []) {
    const chave = e.posicao && e.campo === 'extras' ? `extras-${e.posicao}` : e.campo;
    if (!(chave in mapa)) mapa[chave] = e.motivo;
  }
  return mapa;
}
