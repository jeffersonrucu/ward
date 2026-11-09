// Formulário de credencial (Fases 33 e 34): os campos de cada um dos 9 tipos, como o
// FigCraft os desenha (frames `Formulário / <tipo> / Novo` e `/ Edição`,
// docs/mission/design-nodes.md), e a montagem do pedido `salvar_credencial`.
//
// Fase 34: `banco`, `servidor` e `ftp/sftp`. "Autenticação" é só da tela (senha ou
// chave): decide quais segredos vão; os da outra forma vão "vazio". Anexos vão só pelo
// id (o conteúdo fica no Rust, S11).
//
// S11: o formulário nunca recebe segredo do cofre. Segredo salvo vai como "manter" (o
// Rust o põe no rascunho), senha do gerador vai como "gerada" (idem), chave lida pelo
// "Carregar de arquivo" vai como "carregada" (idem, Fase 53a), e só o que o usuário
// digitou vai como `{ novo }`.

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

// Motores do `banco` (ward-core `Motor`), com a porta padrão e as variáveis de D10.
export const MOTORES = Object.freeze([
  { valor: 'postgres', rotulo: 'postgres', porta: '5432', variaveis: 'PGHOST, PGPORT, PGUSER, PGPASSWORD e DATABASE_URL' },
  { valor: 'mysql', rotulo: 'mysql', porta: '3306', variaveis: 'MYSQL_HOST, MYSQL_TCP_PORT, MYSQL_USER, MYSQL_PWD e DATABASE_URL' },
  { valor: 'mongo', rotulo: 'mongo', porta: '27017', variaveis: 'MONGODB_URI e DATABASE_URL' },
  { valor: 'redis', rotulo: 'redis', porta: '6379', variaveis: 'REDIS_HOST, REDIS_PORT, REDISCLI_AUTH, REDIS_URL e DATABASE_URL' },
  { valor: 'sqlserver', rotulo: 'sqlserver', porta: '1433', variaveis: 'SQLCMDSERVER, SQLCMDUSER, SQLCMDPASSWORD e DATABASE_URL' },
]);

export const PROTOCOLOS = Object.freeze([
  { valor: 'sftp', rotulo: 'sftp', porta: '22' },
  { valor: 'ftp', rotulo: 'ftp', porta: '21' },
]);

export const AUTENTICACOES = Object.freeze([
  { valor: 'senha', rotulo: 'Senha' },
  { valor: 'chave', rotulo: 'Chave' },
]);

/** Texto de "Acesso" do banco para o motor escolhido. */
export function acessoDoBanco(motor) {
  const m = MOTORES.find((x) => x.valor === motor) ?? MOTORES[0];
  return `Entregues como ${m.variaveis}.`;
}

/** Porta padrão (placeholder) do campo porta, pelo motor, protocolo ou segurança. */
export function portaPadrao(tipo, campos) {
  if (tipo === 'banco') return MOTORES.find((x) => x.valor === campos.motor)?.porta ?? '';
  if (tipo === 'ftp-sftp') return PROTOCOLOS.find((x) => x.valor === campos.protocolo)?.porta ?? '';
  if (tipo === 'servidor') return '22';
  if (tipo === 'smtp') return { nenhuma: '25', starttls: '587', tls: '465' }[campos.seguranca] ?? '587';
  return '';
}

// Cada linha da grade: um campo inteiro ou dois lado a lado. `tipo`: texto, segredo,
// select, totp (o `Código TOTP`) ou nota (o texto da nota segura).
const T = (id, rotulo, extra = {}) => ({ id, rotulo, tipo: 'texto', ...extra });
const S = (id, rotulo, extra = {}) => ({ id, rotulo, tipo: 'segredo', ...extra });
// Select só da tela: não vai em `campos` (decide quais segredos vão).
const AUTENTICACAO = { id: 'autenticacao', rotulo: 'Autenticação', tipo: 'select', opcoes: 'autenticacoes', local: true };
const LINHAS_AUTENTICACAO = [
  [S('senha', 'Senha', { gerar: true, auth: 'senha' })],
  [
    S('chave_privada', 'Chave privada', { auth: 'chave', multilinha: true }),
    S('passphrase', 'Passphrase (opcional)', { auth: 'chave' }),
  ],
];

export const TIPOS_FORM = Object.freeze({
  login: {
    acesso: 'Entregues como WARD_URL, WARD_USER, WARD_PASS e WARD_TOTP.',
    linhas: [
      [T('url', 'URL', { placeholder: 'Endereço com http ou https', url: true })],
      [T('usuario', 'Usuário', { placeholder: 'Ex.: wp_admin' }), S('senha', 'Senha', { gerar: true })],
      [S('totp_semente', 'Semente TOTP'), { id: 'codigo', tipo: 'totp' }],
    ],
  },
  banco: {
    exemplo: 'Ex.: acme-pg-dev',
    acesso: acessoDoBanco('postgres'),
    linhas: [
      [
        { id: 'motor', rotulo: 'Motor', tipo: 'select', opcoes: 'motores' },
        T('host', 'Host', { placeholder: 'Ex.: 127.0.0.1' }),
      ],
      [
        T('porta', 'Porta', { porta: true, modo: 'numeric' }),
        T('base', 'Base', { placeholder: 'Ex.: app_teste' }),
      ],
      [T('usuario', 'Usuário', { placeholder: 'Ex.: app' }), S('senha', 'Senha', { gerar: true })],
      [{ id: 'tunel', rotulo: 'Túnel SSH (opcional)', tipo: 'select', opcoes: 'servidores' }],
    ],
  },
  servidor: {
    exemplo: 'Ex.: acme-ssh-stage',
    acesso: 'O Claude entra com ward ssh; o Conectar abre o seu terminal.',
    linhas: [
      [
        T('host', 'Host', { placeholder: 'Ex.: 127.0.0.1' }),
        T('porta', 'Porta', { porta: true, modo: 'numeric' }),
      ],
      [T('usuario', 'Usuário', { placeholder: 'Ex.: deploy' }), AUTENTICACAO],
      ...LINHAS_AUTENTICACAO,
      [{ id: 'salto', rotulo: 'Servidor de salto (opcional)', tipo: 'select', opcoes: 'saltos' }],
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
  'ftp-sftp': {
    exemplo: 'Ex.: beta-sftp-dev',
    acesso:
      'Entregues como WARD_PROTOCOLO, WARD_HOST, WARD_PORT, WARD_USER e WARD_ARQUIVO_CHAVE (+ WARD_PASSPHRASE) ou WARD_PASS.',
    linhas: [
      [{ id: 'protocolo', rotulo: 'Protocolo', tipo: 'select', opcoes: 'protocolos' }],
      [
        T('host', 'Host', { placeholder: 'Ex.: sftp.exemplo.com' }),
        T('porta', 'Porta', { porta: true, modo: 'numeric' }),
      ],
      [T('usuario', 'Usuário', { placeholder: 'Ex.: sftpuser' }), AUTENTICACAO],
      ...LINHAS_AUTENTICACAO,
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

// Cartões de `Nova credencial / Escolher tipo` (17657).
export const CARTOES_TIPO = Object.freeze([
  { tipo: 'login', nome: 'login', descricao: 'URL, usuário, senha e TOTP' },
  { tipo: 'banco', nome: 'banco', descricao: 'Motor, host, porta, base e senha' },
  { tipo: 'servidor', nome: 'servidor', descricao: 'SSH com senha ou chave' },
  { tipo: 'api', nome: 'api', descricao: 'URL base, token e variável' },
  { tipo: 'aws', nome: 'aws', descricao: 'Access key, secret, região e role' },
  { tipo: 'smtp', nome: 'smtp', descricao: 'Envio de e-mail com usuário e senha' },
  { tipo: 'ftp-sftp', nome: 'ftp/sftp', descricao: 'Arquivos por senha ou chave' },
  { tipo: 'nota-segura', nome: 'nota segura', descricao: 'Texto secreto, sem entrega' },
  { tipo: 'generico', nome: 'genérico', descricao: 'Só campos extras, do seu jeito' },
]);

export function camposDoTipo(tipo) {
  return (TIPOS_FORM[tipo]?.linhas ?? []).flat();
}

/** O campo aparece com a autenticação escolhida? (Só os de senha ou de chave mudam.) */
export function campoVisivel(c, autenticacao) {
  return !c.auth || c.auth === autenticacao;
}

/** Tamanho do anexo como o design escreve (`1,2 KB`, `300 bytes`). */
export function tamanhoLegivel(bytes) {
  if (bytes < 1024) return `${bytes} ${bytes === 1 ? 'byte' : 'bytes'}`;
  return `${(bytes / 1024).toLocaleString('pt-BR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} KB`;
}

/** `WARD_ARQUIVO_<NOME>` como o ward-core monta (D10): maiúsculas; o resto vira `_`. */
export function variavelDoAnexo(nome) {
  return `WARD_ARQUIVO_${nome.toUpperCase().replace(/[^A-Z0-9]/g, '_')}`;
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
 * Novo ainda vazio (Fase 39c, V5.10): nada digitado, escolhido ou anexado além dos
 * padrões dos selects (`camposIniciais`). O frame `Formulário / <tipo> / Novo` desenha o
 * "Salvar" desabilitado nesse estado; com qualquer coisa preenchida ele habilita, e
 * salvar sem um obrigatório mostra o erro no campo.
 */
export function novoVazio(estado, camposIniciais) {
  const { nome, grupo, ambiente, projeto, tags, campos, segredos, extras, anexos } = estado;
  const semTexto = [nome, grupo, ambiente, projeto, tags].every((v) => !String(v ?? '').trim());
  const camposIguais = Object.entries(campos).every(([id, v]) => v === camposIniciais[id]);
  const semSegredo = Object.values(segredos).every((s) => s.modo === 'digitando' && !s.digitado);
  return semTexto && camposIguais && semSegredo && extras.length === 0 && anexos.length === 0;
}

/**
 * Corpo do `salvar_credencial`. `segredos[id]`: `{ modo, digitado }` com modo salvo,
 * gerada ou digitando; `extras`: `{ nome, valor, segredo, guardado, original }`.
 */
export function montarFormulario(tipo, estado) {
  const campos = {};
  const segredos = {};
  const auth = estado.campos.autenticacao;
  for (const c of camposDoTipo(tipo)) {
    if (c.local) continue;
    if (c.auth && c.auth !== auth) {
      segredos[c.id] = 'vazio';
    } else if (c.tipo === 'texto' || c.tipo === 'select') {
      const v = estado.campos[c.id] ?? '';
      campos[c.id] = c.url ? urlComPunycode(v) : nfc(v).trim();
    } else if (c.tipo === 'segredo' || c.tipo === 'nota') {
      const s = estado.segredos[c.id];
      if (!s) segredos[c.id] = 'vazio';
      else if (s.modo === 'salvo') segredos[c.id] = 'manter';
      else if (s.modo === 'gerada') segredos[c.id] = 'gerada';
      else if (s.modo === 'carregada') segredos[c.id] = 'carregada';
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
    anexos: (estado.anexos ?? []).map((a) => a.id),
    // Fase 39b: na edição, os anexos guardados que continuam (pelo nome). O conteúdo
    // fica no daemon; o que sair desta lista é removido ao salvar.
    anexos_mantidos: (estado.guardados ?? []).map((a) => a.nome),
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
