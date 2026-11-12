// Credential form (Phases 33 and 34): the fields of each of the 9 types, as the
// FigCraft draws them (the New and Edit frames of each type,
// docs/mission/design-nodes.md), and the building of the save request.
//
// Phase 34: the database, server and ftp/sftp types. The authentication select is only
// on the screen (password or key): it decides which secrets go; those of the other
// method go as empty. Attachments go only by id (the content stays in Rust, S11).
//
// S11: the form never receives a secret from the vault. A saved secret goes as keep
// (Rust puts it in the draft), the generator password goes as generated (likewise), a
// key read by the load-from-file button goes as loaded (likewise, Phase 53a), and only
// what the user typed goes as a new value.
//
// The type keys and the SMTP security values are the vault v5 names (m2:DT4, Phase 52).
// Wire names (m2:Phase 54): the field ids (the protocol v2 ones), the keys of the save
// form and of its errors, and the secret values above.

export const FORM_ENVIRONMENTS = Object.freeze([
  { value: '', label: 'Escolha o ambiente' },
  { value: 'dev', label: 'dev' },
  { value: 'stage', label: 'stage' },
  { value: 'prod', label: 'prod' },
]);

// AWS regions offered in the select (the `ward-core` validation accepts any region in
// the format; the one of the edited entry joins the list if it is not there).
export const AWS_REGIONS = Object.freeze([
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

export const SECURITY_OPTIONS = Object.freeze([
  { value: 'starttls', label: 'STARTTLS' },
  { value: 'tls', label: 'TLS' },
  { value: 'none', label: 'Nenhuma' },
]);

// Engines of the database type (ward-core engine), with the default port and the
// variables of D10.
export const ENGINES = Object.freeze([
  { value: 'postgres', label: 'postgres', port: '5432', variables: 'PGHOST, PGPORT, PGUSER, PGPASSWORD e DATABASE_URL' },
  { value: 'mysql', label: 'mysql', port: '3306', variables: 'MYSQL_HOST, MYSQL_TCP_PORT, MYSQL_USER, MYSQL_PWD e DATABASE_URL' },
  { value: 'mongo', label: 'mongo', port: '27017', variables: 'MONGODB_URI e DATABASE_URL' },
  { value: 'redis', label: 'redis', port: '6379', variables: 'REDIS_HOST, REDIS_PORT, REDISCLI_AUTH, REDIS_URL e DATABASE_URL' },
  { value: 'sqlserver', label: 'sqlserver', port: '1433', variables: 'SQLCMDSERVER, SQLCMDUSER, SQLCMDPASSWORD e DATABASE_URL' },
]);

export const PROTOCOLS = Object.freeze([
  { value: 'sftp', label: 'sftp', port: '22' },
  { value: 'ftp', label: 'ftp', port: '21' },
]);

// Values of the screen-only authentication select (never sent).
export const AUTHENTICATIONS = Object.freeze([
  { value: 'password', label: 'Senha' },
  { value: 'key', label: 'Chave' },
]);

/** Access text of the database type for the chosen engine. */
export function databaseAccess(engine) {
  const m = ENGINES.find((x) => x.value === engine) ?? ENGINES[0];
  return `Entregues como ${m.variables}.`;
}

/** Default port (placeholder) of the port field, by engine, protocol or security. */
export function defaultPort(kind, fields) {
  if (kind === 'database') return ENGINES.find((x) => x.value === fields.engine)?.port ?? '';
  if (kind === 'ftp-sftp') return PROTOCOLS.find((x) => x.value === fields.protocol)?.port ?? '';
  if (kind === 'server') return '22';
  if (kind === 'smtp') return { none: '25', starttls: '587', tls: '465' }[fields.security] ?? '587';
  return '';
}

// Each row of the grid: one whole field or two side by side. `kind`: text, secret,
// select, totp (the TOTP code) or note (the text of the secure note).
const T = (id, label, extra = {}) => ({ id, label, kind: 'text', ...extra });
const S = (id, label, extra = {}) => ({ id, label, kind: 'secret', ...extra });
// Screen-only select: it does not go in the form fields (it decides which secrets go).
const AUTHENTICATION = {
  id: 'authentication',
  label: 'Autenticação',
  kind: 'select',
  options: 'authentications',
  local: true,
};
const AUTHENTICATION_ROWS = [
  [S('password', 'Senha', { generate: true, auth: 'password' })],
  [
    S('private_key', 'Chave privada', { auth: 'key', multiline: true }),
    S('passphrase', 'Passphrase (opcional)', { auth: 'key' }),
  ],
];

export const FORM_TYPES = Object.freeze({
  'login': {
    access: 'Entregues como WARD_URL, WARD_USER, WARD_PASS e WARD_TOTP.',
    rows: [
      [T('url', 'URL', { placeholder: 'Endereço com http ou https', url: true })],
      [T('username', 'Usuário', { placeholder: 'Ex.: wp_admin' }), S('password', 'Senha', { generate: true })],
      [S('totp_seed', 'Semente TOTP'), { id: 'code', kind: 'totp' }],
    ],
  },
  'database': {
    example: 'Ex.: acme-pg-dev',
    access: databaseAccess('postgres'),
    rows: [
      [
        { id: 'engine', label: 'Motor', kind: 'select', options: 'engines' },
        T('host', 'Host', { placeholder: 'Ex.: 127.0.0.1' }),
      ],
      [
        T('port', 'Porta', { port: true, inputMode: 'numeric' }),
        T('db_name', 'Base', { placeholder: 'Ex.: app_teste' }),
      ],
      [T('username', 'Usuário', { placeholder: 'Ex.: app' }), S('password', 'Senha', { generate: true })],
      [{ id: 'tunnel', label: 'Túnel SSH (opcional)', kind: 'select', options: 'servers' }],
    ],
  },
  'server': {
    example: 'Ex.: acme-ssh-stage',
    access: 'O Claude entra com ward ssh; o Conectar abre o seu terminal.',
    rows: [
      [
        T('host', 'Host', { placeholder: 'Ex.: 127.0.0.1' }),
        T('port', 'Porta', { port: true, inputMode: 'numeric' }),
      ],
      [T('username', 'Usuário', { placeholder: 'Ex.: deploy' }), AUTHENTICATION],
      ...AUTHENTICATION_ROWS,
      [{ id: 'jump', label: 'Servidor de salto (opcional)', kind: 'select', options: 'jumps' }],
    ],
  },
  'api': {
    access: 'Entregues como WARD_URL, WARD_TOKEN e a variável escolhida.',
    rows: [
      [T('url', 'URL base', { placeholder: 'Endereço com http ou https', url: true })],
      [S('token', 'Token')],
      [
        T('variable', 'Nome da variável', {
          placeholder: 'Ex.: GITLAB_TOKEN',
          help: 'O token vai nesta variável e em WARD_TOKEN.',
        }),
      ],
    ],
  },
  'aws': {
    access:
      'Entregues como AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION e AWS_SESSION_TOKEN.',
    rows: [
      [T('access_key_id', 'Access key ID', { placeholder: 'Ex.: AKIA…' })],
      [S('secret_access_key', 'Secret access key')],
      [
        { id: 'region', label: 'Região', kind: 'select', options: 'regions' },
        S('session_token', 'Session token (opcional)'),
      ],
      [T('role_arn', 'Role ARN (opcional)', { placeholder: 'arn:aws:iam::…' })],
    ],
  },
  'smtp': {
    access: 'Entregues como SMTP_HOST, SMTP_PORT, SMTP_USER, SMTP_PASS e SMTP_SECURITY.',
    rows: [
      [
        T('host', 'Host', { placeholder: 'Ex.: smtp.exemplo.com' }),
        T('port', 'Porta', { placeholder: '587', inputMode: 'numeric' }),
      ],
      [
        T('username', 'Usuário', { placeholder: 'Ex.: mailer' }),
        { id: 'security', label: 'Segurança', kind: 'select', options: 'security' },
      ],
      [S('password', 'Senha', { generate: true })],
    ],
  },
  'ftp-sftp': {
    example: 'Ex.: beta-sftp-dev',
    access:
      'Entregues como WARD_PROTOCOL, WARD_HOST, WARD_PORT, WARD_USER e WARD_FILE_KEY (+ WARD_PASSPHRASE) ou WARD_PASS.',
    rows: [
      [{ id: 'protocol', label: 'Protocolo', kind: 'select', options: 'protocols' }],
      [
        T('host', 'Host', { placeholder: 'Ex.: sftp.exemplo.com' }),
        T('port', 'Porta', { port: true, inputMode: 'numeric' }),
      ],
      [T('username', 'Usuário', { placeholder: 'Ex.: sftpuser' }), AUTHENTICATION],
      ...AUTHENTICATION_ROWS,
    ],
  },
  'secure-note': {
    access: 'Sem entrega: o ward run recusa este tipo e o Claude nunca recebe o texto.',
    accessTitle: 'Conteúdo',
    rows: [[{ id: 'text', label: 'Nota', kind: 'note' }]],
  },
  'generic': { access: '', rows: [] },
});

export const EXTRAS_TEXTS = Object.freeze({
  'secure-note': 'Ficam com a nota; nota segura não tem entrega.',
  'generic': 'Só estes campos são entregues, como WARD_<NOME>.',
  default: 'Viram WARD_<NOME> no ward run.',
});

export const ATTACHMENTS_TEXTS = Object.freeze({
  'secure-note': 'Ficam cifrados com a nota; sem entrega no ward run.',
  default: 'Viram WARD_FILE_<NOME> no ward run.',
});

// Cards of the choose-type screen (FigCraft 17657).
export const TYPE_CARDS = Object.freeze([
  { kind: 'login', name: 'login', description: 'URL, usuário, senha e TOTP' },
  { kind: 'database', name: 'banco', description: 'Motor, host, porta, base e senha' },
  { kind: 'server', name: 'servidor', description: 'SSH com senha ou chave' },
  { kind: 'api', name: 'api', description: 'URL base, token e variável' },
  { kind: 'aws', name: 'aws', description: 'Access key, secret, região e role' },
  { kind: 'smtp', name: 'smtp', description: 'Envio de e-mail com usuário e senha' },
  { kind: 'ftp-sftp', name: 'ftp/sftp', description: 'Arquivos por senha ou chave' },
  { kind: 'secure-note', name: 'nota segura', description: 'Texto secreto, sem entrega' },
  { kind: 'generic', name: 'genérico', description: 'Só campos extras, do seu jeito' },
]);

export function fieldsOfType(kind) {
  return (FORM_TYPES[kind]?.rows ?? []).flat();
}

/** Is the field shown with the chosen authentication? (Only password or key fields change.) */
export function fieldVisible(c, authentication) {
  return !c.auth || c.auth === authentication;
}

/** Attachment size as the design writes it (`1,2 KB`, `300 bytes`). */
export function readableSize(bytes) {
  if (bytes < 1024) return `${bytes} ${bytes === 1 ? 'byte' : 'bytes'}`;
  return `${(bytes / 1024).toLocaleString('pt-BR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} KB`;
}

/** The attachment variable as ward-core builds it (D10): uppercase; the rest becomes `_`. */
export function attachmentVariable(name) {
  return `WARD_FILE_${name.toUpperCase().replace(/[^A-Z0-9]/g, '_')}`;
}

/** Text as the vault keeps it: NFC (Rust validates it again). */
export function nfc(v) {
  return typeof v === 'string' ? v.normalize('NFC') : '';
}

/**
 * URL with the host in punycode (IDN), as the browser's `new URL` writes it; the rest
 * stays as typed. Text that is not a URL goes as it is (Rust gives the error).
 */
export function urlWithPunycode(v) {
  const text = nfc(v).trim();
  try {
    const u = new URL(text);
    if (!/[^\x00-\x7F]/.test(text) || !u.hostname) return text;
    const start = text.indexOf('//') + 2;
    const rest = text.slice(start);
    const end = rest.search(/[/?#]/);
    const authority = end === -1 ? rest : rest.slice(0, end);
    const at = authority.lastIndexOf('@');
    const before = at === -1 ? '' : authority.slice(0, at + 1);
    const hostPort = authority.slice(at + 1);
    const port = /:\d+$/.exec(hostPort)?.[0] ?? '';
    return text.slice(0, start) + before + u.hostname + port + rest.slice(authority.length);
  } catch {
    return text;
  }
}

/** Typed tags (comma separated) as a list, without empty ones. */
export function tagsOf(text) {
  return nfc(text)
    .split(',')
    .map((t) => t.trim())
    .filter((t) => t !== '');
}

/**
 * New form still empty (Phase 39c, V5.10): nothing typed, chosen or attached beyond
 * the defaults of the selects (`initialFields`). The New frame of each type draws the
 * save button disabled in this state; with anything filled in it is enabled, and
 * saving without a required field shows the error on the field.
 */
export function newIsEmpty(state, initialFields) {
  const { name, group, environment, project, tags, fields, secrets, extras, attachments } = state;
  const noText = [name, group, environment, project, tags].every((v) => !String(v ?? '').trim());
  const sameFields = Object.entries(fields).every(([id, v]) => v === initialFields[id]);
  const noSecret = Object.values(secrets).every((s) => s.mode === 'typing' && !s.typed);
  return noText && sameFields && noSecret && extras.length === 0 && attachments.length === 0;
}

/**
 * Body of the save command. `secrets[id]`: `{ mode, typed }` with mode saved,
 * generated, loaded or typing; `extras`: `{ name, value, secret, stored, original }`;
 * `stored`: the attachments already in the edited entry that stay.
 */
export function buildForm(kind, state) {
  const fields = {};
  const secrets = {};
  const auth = state.fields.authentication;
  for (const c of fieldsOfType(kind)) {
    if (c.local) continue;
    if (c.auth && c.auth !== auth) {
      secrets[c.id] = 'empty';
    } else if (c.kind === 'text' || c.kind === 'select') {
      const v = state.fields[c.id] ?? '';
      fields[c.id] = c.url ? urlWithPunycode(v) : nfc(v).trim();
    } else if (c.kind === 'secret' || c.kind === 'note') {
      const s = state.secrets[c.id];
      if (!s) secrets[c.id] = 'empty';
      else if (s.mode === 'saved') secrets[c.id] = 'keep';
      else if (s.mode === 'generated') secrets[c.id] = 'generated';
      else if (s.mode === 'loaded') secrets[c.id] = 'loaded';
      else if (s.typed === '') secrets[c.id] = 'empty';
      else secrets[c.id] = { 'new': c.kind === 'note' ? nfc(s.typed) : s.typed };
    }
  }
  return {
    'type': kind,
    'name': nfc(state.name).trim(),
    'group': nfc(state.group).trim(),
    'environment': state.environment,
    'project': nfc(state.project).trim(),
    'tags': tagsOf(state.tags),
    'fields': fields,
    'secrets': secrets,
    'extras': state.extras.map((x) => ({
      'name': nfc(x.name).trim(),
      'secret': x.secret,
      'value': x.stored ? { 'keep': x.original } : { 'new': x.secret ? x.value : nfc(x.value) },
    })),
    'attachments': (state.attachments ?? []).map((a) => a.id),
    // Phase 39b: when editing, the stored attachments that stay (by name). The content
    // stays in the daemon; whatever leaves this list is removed on save.
    'kept_attachments': (state.stored ?? []).map((a) => a.name),
  };
}

/** Errors by field (field, position and reason, from the wire) in a map for the screen. */
export function errorsByField(errors) {
  const map = {};
  for (const e of errors ?? []) {
    const key = e.position && e.field === 'extras' ? `extras-${e.position}` : e.field;
    if (!(key in map)) map[key] = e.reason;
  }
  return map;
}
