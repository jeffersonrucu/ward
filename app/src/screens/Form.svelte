<script>
  // Credential form (Phases 33 and 34): the New and Edit frames of the 9 types (ids in
  // docs/mission/design-nodes.md). It fills the detail panel of Credentials: header
  // (avatar, name or the new-credential title, badges), scrolling body (identification
  // in a 2-column grid, the access section with the fields of the type, extra fields,
  // attachments, the level card) and footer (required fields, cancel, save).
  //
  // S11/D4/D65: the saved value never reaches this screen. A saved secret shows as a
  // mask and goes on save as keep; reveal asks Rust for the field and the value leaves
  // in 30 s, on hide or on leaving; copy asks Rust. The generator password stays in Rust
  // (using it returns only the length) and goes as generated. What the user types lives
  // only in the state of this screen, which goes away on leaving (the discard command
  // clears Rust).
  //
  // Phase 34: attachments are chosen and read in Rust (choose-attachment command); the
  // screen gets only id, name and size, or the notice (file above 256 KiB, sum above
  // that, empty). Connect (D3) asks Rust for the token and the terminal; the token
  // never comes back.
  //
  // Phase 53a (S2, S11): the load-from-file button of the private key field asks Rust
  // (load-key command), which chooses, reads and checks the file and keeps the key; the
  // screen gets only the size (or the notice), and the key goes on save as loaded, with
  // no clipboard and without passing through here.
  //
  // Wire names (frozen until m2:Phase 54): the request and reply fields of the Tauri
  // commands and of the entry detail, and the field ids (`dev/english-check/frozen.txt`).
  import { onDestroy, onMount } from 'svelte';
  import Attachment from '../lib/components/Attachment.svelte';
  import EnvironmentBadge from '../lib/components/EnvironmentBadge.svelte';
  import TypeBadge from '../lib/components/TypeBadge.svelte';
  import Button from '../lib/components/Button.svelte';
  import Field from '../lib/components/Field.svelte';
  import ExtraField from '../lib/components/ExtraField.svelte';
  import SecretField from '../lib/components/SecretField.svelte';
  import TotpCode from '../lib/components/TotpCode.svelte';
  import GroupCombobox from '../lib/components/GroupCombobox.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import GeneratorPopover from '../lib/components/GeneratorPopover.svelte';
  import Select from '../lib/components/Select.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import SectionTitle from '../lib/components/SectionTitle.svelte';
  import { entryType } from '../lib/components/types.js';
  import { call } from '../lib/bridge/index.js';
  import { LEVELS } from './credentials.js';
  import {
    FORM_ENVIRONMENTS,
    AUTHENTICATIONS,
    ENGINES,
    PROTOCOLS,
    AWS_REGIONS,
    SECURITY_OPTIONS,
    ATTACHMENTS_TEXTS,
    EXTRAS_TEXTS,
    FORM_TYPES,
    databaseAccess,
    fieldVisible,
    fieldsOfType,
    errorsByField,
    buildForm,
    newIsEmpty,
    defaultPort,
    readableSize,
    attachmentVariable,
  } from './form.js';

  const REVEAL_DEADLINE_MS = 30_000;
  const COPIED_MS = 2_000;

  /**
   * @type {{ kind: string, detail?: any, groups: string[], servers?: string[],
   *   oncancel: () => void, onsaved: (name: string) => void }}
   */
  let { kind, detail = null, groups, servers = [], oncancel, onsaved } = $props();

  const editing = detail !== null;
  const def = FORM_TYPES[kind];
  const typeFields = fieldsOfType(kind);

  const initial = (id) => detail?.campos.find((c) => c.id === id)?.valor ?? '';
  const saved = (id) => detail?.segredos.some((s) => s.id === id) ?? false;

  let name = $state(detail?.nome ?? '');
  let group = $state(detail?.grupo ?? '');
  let environment = $state(detail?.ambiente ?? '');
  let project = $state(detail?.projeto ?? '');
  let tags = $state((detail?.tags ?? []).join(', '));
  // Initial value of the selects with no saved value. The authentication select (only
  // on the screen) comes from the secret the edited entry has.
  const DEFAULTS = {
    'seguranca': 'starttls',
    'motor': 'postgres',
    'protocolo': 'sftp',
    authentication: saved('chave_privada') ? 'key' : 'password',
  };
  let fields = $state(
    Object.fromEntries(
      typeFields
        .filter((c) => c.kind === 'text' || c.kind === 'select')
        .map((c) => [c.id, initial(c.id) || (DEFAULTS[c.id] ?? '')]),
    ),
  );
  let secrets = $state(
    Object.fromEntries(
      typeFields
        .filter((c) => c.kind === 'secret' || c.kind === 'note')
        .map((c) => [c.id, { mode: saved(c.id) ? 'saved' : 'typing', typed: '', length: 0 }]),
    ),
  );
  let nextExtra = 0;
  let extras = $state(
    (detail?.extras ?? []).map((x) => ({
      key: nextExtra++,
      name: x.nome,
      value: x.segredo ? '' : (x.valor ?? ''),
      secret: x.segredo,
      stored: x.segredo,
      original: x.nome,
    })),
  );

  /** @type {Array<{ id: number, name: string, size: number }>} */
  let attachments = $state([]);
  // Phase 39b: attachments already stored in the edited entry (name and size from the
  // detail; the content never comes). Removing here only takes it off the list; it
  // counts on save.
  /** @type {Array<{ name: string, size: number }>} */
  let stored = $state((detail?.anexos ?? []).map((a) => ({ name: a.nome, size: a.tamanho })));

  // V5.10 (Phase 39c): in the New form, save stays disabled until something is filled in.
  const initialFields = $state.snapshot(fields);
  let empty = $derived(
    !editing &&
      newIsEmpty(
        { name, group, environment, project, tags, fields, secrets, extras, attachments },
        initialFields,
      ),
  );
  let attachmentNotice = $state('');
  let choosing = $state(false);
  /** @type {Record<string, string>} */
  let keyNotice = $state({});
  let loading = $state(false);
  let connecting = $state(false);
  let connected = $state(false);

  let errors = $state({});
  let generalError = $state('');
  let saving = $state(false);
  /** @type {Record<string, string>} */
  let revealed = $state({});
  let copied = $state('');
  let generatorFor = $state('');
  /** TOTP reply of the bridge (code and seconds left, wire names), or null. */
  let totp = $state(null);

  const level = $derived(LEVELS[detail?.nivel] ?? LEVELS['com-aprovacao']);
  const regions = $derived(
    [{ value: '', label: 'Escolha a região' }].concat(
      [...new Set([...(fields.regiao ? [fields.regiao] : []), ...AWS_REGIONS])].map((r) => ({
        value: r,
        label: r,
      })),
    ),
  );
  // Servers for tunnel and jump: the ones in the vault, minus the entry itself; the saved
  // value joins the list even if the entry no longer exists (the validation gives the
  // error).
  const serverOptions = (current) =>
    [{ value: '', label: 'Nenhum' }].concat(
      [...new Set([...(current ? [current] : []), ...servers])]
        .filter((n) => n !== detail?.nome)
        .map((n) => ({ value: n, label: n })),
    );
  const selectOptions = $derived({
    regions,
    security: SECURITY_OPTIONS,
    engines: ENGINES,
    protocols: PROTOCOLS,
    // ftp does not take a key (ward-core refuses it); the screen offers only password.
    authentications: fields.protocolo === 'ftp' ? AUTHENTICATIONS.slice(0, 1) : AUTHENTICATIONS,
    servers: serverOptions(fields.tunel),
    jumps: serverOptions(fields.salto),
  });
  const accessText = $derived(kind === 'banco' ? databaseAccess(fields.motor) : def.access);
  const visibleRows = $derived(
    def.rows
      .map((l) => l.filter((c) => fieldVisible(c, fields.authentication)))
      .filter((l) => l.length > 0),
  );

  $effect(() => {
    if (fields.protocolo === 'ftp' && fields.authentication !== 'password') fields.authentication = 'password';
  });

  let active = true;
  const deadlines = {};
  let copiedDeadline = 0;
  let totpDeadline = 0;

  function hide(id) {
    clearTimeout(deadlines[id]);
    delete revealed[id];
  }

  function hideAll() {
    for (const id of Object.keys(revealed)) hide(id);
  }

  async function reveal(id) {
    if (!editing) return;
    try {
      const r = await call('revelar_campo', { 'entrada': detail.nome, 'campo': id });
      if (!active) return;
      revealed[id] = r.valor;
      clearTimeout(deadlines[id]);
      deadlines[id] = setTimeout(() => hide(id), REVEAL_DEADLINE_MS);
    } catch (e) {
      generalError = e.mensagem;
    }
  }

  function markCopied(id) {
    copied = id;
    clearTimeout(copiedDeadline);
    copiedDeadline = setTimeout(() => (copied = ''), COPIED_MS);
  }

  async function copy(id) {
    if (!editing) return;
    try {
      await call('copiar_campo', { 'entrada': detail.nome, 'campo': id });
      markCopied(id);
    } catch (e) {
      generalError = e.mensagem;
    }
  }

  async function copyTotp() {
    try {
      await call('copiar_codigo_totp', { 'entrada': detail.nome });
      markCopied('code');
    } catch (e) {
      generalError = e.mensagem;
    }
  }

  // TOTP code of the edited entry: one request, a local countdown, a new request at the end.
  async function refreshTotp() {
    clearTimeout(totpDeadline);
    if (!editing || !saved('totp_semente') || secrets.totp_semente?.mode !== 'saved') {
      totp = null;
      return;
    }
    try {
      const r = await call('codigo_totp', { 'entrada': detail.nome });
      if (!active) return;
      totp = r;
      countdown();
    } catch {
      totp = null;
    }
  }

  function countdown() {
    totpDeadline = setTimeout(() => {
      if (!active || !totp) return;
      if (totp.restante <= 1) refreshTotp();
      else {
        totp = { ...totp, 'restante': totp.restante - 1 };
        countdown();
      }
    }, 1000);
  }

  function replaceSecret(id) {
    hide(id);
    delete keyNotice[id];
    secrets[id] = { mode: 'typing', typed: '', length: 0 };
    if (id === 'totp_semente') {
      clearTimeout(totpDeadline);
      totp = null;
    }
  }

  function useGenerated(length) {
    secrets[generatorFor] = { mode: 'generated', typed: '', length };
    generatorFor = '';
  }

  // Phase 53a: the key stays in Rust; here only the loaded mode and the size (mask).
  async function loadKey(id) {
    if (loading) return;
    loading = true;
    delete keyNotice[id];
    try {
      const r = await call('carregar_chave', { 'campo': id });
      if (!active) return;
      if (r.resultado === 'carregada') {
        secrets[id] = { mode: 'loaded', typed: '', length: r.tamanho };
        delete errors[id];
      } else if (r.resultado === 'aviso') {
        keyNotice[id] = r.motivo;
      }
    } catch (e) {
      keyNotice[id] = e.mensagem;
    } finally {
      loading = false;
    }
  }

  function addExtra() {
    extras.push({
      key: nextExtra++,
      name: '',
      value: '',
      secret: false,
      stored: false,
      original: '',
    });
  }

  async function addAttachment() {
    if (choosing) return;
    choosing = true;
    attachmentNotice = '';
    try {
      const r = await call(
        'escolher_anexo',
        editing ? { 'edicao': { 'entrada': detail.nome, 'mantidos': stored.map((a) => a.name) } } : {},
      );
      if (!active) return;
      if (r.resultado === 'anexado') {
        attachments.push({ id: r.id, name: r.nome, size: r.tamanho });
        delete errors.anexos;
      } else if (r.resultado === 'aviso') {
        attachmentNotice = r.motivo;
      }
    } catch (e) {
      attachmentNotice = e.mensagem;
    } finally {
      choosing = false;
    }
  }

  function removeStored(i) {
    stored.splice(i, 1);
    attachmentNotice = '';
    delete errors.anexos;
  }

  async function removeAttachment(i) {
    const a = attachments[i];
    try {
      await call('remover_anexo', { id: a.id });
      attachments.splice(i, 1);
      attachmentNotice = '';
    } catch (e) {
      attachmentNotice = e.mensagem;
    }
  }

  async function connect() {
    if (connecting || !editing) return;
    connecting = true;
    generalError = '';
    try {
      await call('conectar', { 'entrada': detail.nome });
      if (!active) return;
      connected = true;
      clearTimeout(copiedDeadline);
      copiedDeadline = setTimeout(() => (connected = false), COPIED_MS);
    } catch (e) {
      generalError = e.mensagem;
    } finally {
      connecting = false;
    }
  }

  function errorOf(id) {
    return errors[id] ?? '';
  }

  async function save() {
    if (saving || empty) return;
    saving = true;
    generalError = '';
    hideAll();
    try {
      const form = buildForm(kind, {
        name,
        group,
        environment,
        project,
        tags,
        fields,
        secrets,
        extras,
        attachments,
        stored,
      });
      const r = await call('salvar_credencial', {
        'anterior': editing ? detail.nome : null,
        'formulario': form,
      });
      if (!active) return;
      if (r.salvo) {
        onsaved(r.salvo);
        return;
      }
      errors = errorsByField(r.erros);
      generalError = errors.formulario ?? '';
    } catch (e) {
      generalError = e.mensagem;
    } finally {
      saving = false;
    }
  }

  onMount(refreshTotp);

  onDestroy(() => {
    active = false;
    hideAll();
    clearTimeout(copiedDeadline);
    clearTimeout(totpDeadline);
    call('descartar_formulario').catch(() => {});
  });
</script>

{#snippet secretField(c)}
  {@const s = secrets[c.id]}
  <SecretField
    id="form-{c.id}"
    label={c.label}
    editable
    empty={s.mode === 'typing'}
    bind:typed={secrets[c.id].typed}
    mask={s.mode === 'generated' ? '•'.repeat(s.length) : '••••••••'}
    showReveal={s.mode === 'saved'}
    showCopy={s.mode === 'saved'}
    showGenerate={!!c.generate}
    revealed={revealed[c.id] ?? null}
    copied={copied === c.id}
    error={errorOf(c.id)}
    help={s.mode === 'saved'
      ? 'Revelar e copiar não pedem PIN.'
      : s.mode === 'generated'
        ? 'Senha gerada. Ela fica oculta e vai para o cofre ao salvar.'
        : s.mode === 'loaded'
          ? `Chave carregada do arquivo (${readableSize(s.length)}). Ela fica oculta e vai para o cofre ao salvar.`
          : 'Digite ou gere o valor. Depois de salvo, ele fica oculto.'}
    placeholder="Nenhum valor salvo"
    onreveal={() => reveal(c.id)}
    onhide={() => hide(c.id)}
    oncopy={() => copy(c.id)}
    onchange={() => replaceSecret(c.id)}
    ongenerate={() => (generatorFor = c.id)}
  />
{/snippet}

{#snippet rowField(c)}
  {#if c.kind === 'text'}
    <Field
      id="form-{c.id}"
      label={c.label}
      bind:value={fields[c.id]}
      placeholder={c.port ? defaultPort(kind, fields) : (c.placeholder ?? '')}
      help={c.help ?? ''}
      error={errorOf(c.id)}
      inputmode={c.inputMode}
      oncopy={editing && initial(c.id) ? () => copy(c.id) : undefined}
      copied={copied === c.id}
    />
  {:else if c.kind === 'select'}
    <div class="select-with-error">
      <Select
        id="form-{c.id}"
        label={c.label}
        bind:value={fields[c.id]}
        options={selectOptions[c.options]}
      />
      {#if errorOf(c.id)}<span class="error">{errorOf(c.id)}</span>{/if}
    </div>
  {:else if c.kind === 'secret' && c.multiline && secrets[c.id].mode === 'typing'}
    <!-- Private key: several lines (an <input> would lose the line breaks). The text
         stays masked on the screen (text-security) and only in the state of this form.
         Phase 53a: load-from-file reads the file in Rust; the content never comes. -->
    {@const notice = keyNotice[c.id] ?? ''}
    <div class="note">
      <label class="label" for="form-{c.id}">{c.label}</label>
      <textarea
        id="form-{c.id}"
        class="note-text secret"
        class:with-error={errorOf(c.id)}
        bind:value={secrets[c.id].typed}
        placeholder="Cole a chave privada (formato OpenSSH ou PEM)"
        spellcheck="false"
        autocomplete="off"
        aria-invalid={errorOf(c.id) ? 'true' : undefined}
      ></textarea>
      <div class="key-action">
        <Button
          variant="secondary"
          icon="paperclip"
          disabled={loading}
          title="Lê o arquivo da chave direto para o cofre, sem área de transferência."
          data-action="load-key"
          onclick={() => loadKey(c.id)}>Carregar de arquivo</Button
        >
      </div>
      <span class="help" class:error={errorOf(c.id) || notice} role={notice ? 'alert' : undefined}>
        {errorOf(c.id) ||
          notice ||
          'Cole a chave ou use Carregar de arquivo. Depois de salvo, ela só aparece com revelar.'}
      </span>
    </div>
  {:else if c.kind === 'secret'}
    {@render secretField(c)}
  {:else if c.kind === 'totp'}
    <TotpCode
      id="form-code"
      code={totp?.codigo ?? null}
      remaining={totp?.restante ?? 0}
      copied={copied === 'code'}
      oncopy={copyTotp}
    />
  {:else if c.kind === 'note'}
    {@const s = secrets[c.id]}
    {#if s.mode === 'typing'}
      <div class="note">
        <label class="label" for="form-{c.id}">{c.label}</label>
        <textarea
          id="form-{c.id}"
          class="note-text"
          class:with-error={errorOf(c.id)}
          bind:value={secrets[c.id].typed}
          placeholder="Escreva a nota. Ela fica cifrada no cofre e só aparece aqui no app."
          spellcheck="false"
          aria-invalid={errorOf(c.id) ? 'true' : undefined}
        ></textarea>
        <span class="help" class:error={errorOf(c.id)}>
          {errorOf(c.id) || 'Cifrada no cofre'}
        </span>
      </div>
    {:else}
      {@render secretField(c)}
    {/if}
  {/if}
{/snippet}

<header class="header">
  <span class="avatar"><Icon name={entryType(kind).icon} size="lg" /></span>
  <div class="identification">
    <h2 class="name">{editing ? detail.nome : 'Nova credencial'}</h2>
    <div class="badges">
      <TypeBadge {kind} />
      {#if editing}
        <EnvironmentBadge environment={detail.ambiente} />
        <span class="subtitle">grupo {detail.grupo} · editando</span>
      {:else}
        <span class="subtitle">preencha nome, grupo e ambiente</span>
      {/if}
    </div>
  </div>
  {#if editing && kind === 'servidor'}
    <Button
      variant="secondary"
      icon={connected ? 'check' : 'type-server'}
      disabled={connecting}
      title="Abre o seu terminal já conectado, sem PIN (token de uso único)."
      data-connected={connected ? 'yes' : undefined}
      onclick={connect}>Conectar</Button
    >
  {/if}
</header>

<form
  class="body"
  id="credential-form"
  novalidate
  onsubmit={(e) => {
    e.preventDefault();
    save();
  }}
>
  <div class="grid">
    <Field
      id="form-name"
      label="Nome"
      bind:value={name}
      placeholder={def.example ?? 'Ex.: acme-wp-dev'}
      error={errorOf('nome')}
    />
    <Select
      id="form-kind"
      label="Tipo"
      value={kind}
      options={[{ value: kind, label: entryType(kind).label }]}
      disabled={editing}
    />
    <GroupCombobox id="form-group" bind:value={group} {groups} error={errorOf('grupo')} />
    <div class="select-with-error">
      <Select id="form-environment" label="Ambiente" bind:value={environment} options={FORM_ENVIRONMENTS} />
      {#if errorOf('ambiente')}<span class="error">{errorOf('ambiente')}</span>{/if}
    </div>
    <Field
      id="form-project"
      label="Projeto"
      bind:value={project}
      placeholder="Opcional"
      error={errorOf('projeto')}
    />
    <Field
      id="form-tags"
      label="Tags"
      bind:value={tags}
      placeholder="Separe por vírgula"
      error={errorOf('tags')}
    />
  </div>

  {#if def.rows.length > 0}
    <section class="section" aria-labelledby="section-access">
      <SectionTitle id="section-access" title={def.accessTitle ?? 'Acesso'} description={accessText} />
      {#each visibleRows as row (row.map((c) => c.id).join('+'))}
        <div class="row" class:double={row.length > 1}>
          {#each row as c (c.id)}
            {@render rowField(c)}
          {/each}
        </div>
      {/each}
    </section>
  {/if}

  <section class="section" aria-labelledby="section-extras">
    <SectionTitle
      id="section-extras"
      title="Campos extras"
      description={kind === 'generico' && editing
        ? 'Só eles são entregues, como WARD_<NOME>.'
        : (EXTRAS_TEXTS[kind] ?? EXTRAS_TEXTS.default)}
    >
      {#snippet action()}
        <Button variant="ghost" icon="plus" onclick={addExtra}>Adicionar campo</Button>
      {/snippet}
    </SectionTitle>
    {#if extras.length === 0}
      <p class="empty-box" class:error={errorOf('extras')}>
        {errorOf('extras') || 'Nenhum campo extra. Cada um tem nome, valor e a chave segredo.'}
      </p>
    {:else}
      {#each extras as x, i (x.key)}
        <ExtraField
          id="extra-{x.key}"
          bind:name={x.name}
          bind:value={x.value}
          bind:secret={x.secret}
          stored={x.stored}
          error={errors[`extras-${i + 1}`] ?? ''}
          onremove={() => extras.splice(i, 1)}
          onchange={() => {
            x.stored = false;
            x.value = '';
          }}
        />
      {/each}
      {#if errorOf('extras')}<span class="error">{errorOf('extras')}</span>{/if}
    {/if}
  </section>

  <section class="section" aria-labelledby="section-attachments">
    <SectionTitle
      id="section-attachments"
      title="Anexos"
      description={ATTACHMENTS_TEXTS[kind] ?? ATTACHMENTS_TEXTS.default}
    >
      {#snippet action()}
        <Button variant="ghost" icon="paperclip" disabled={choosing} onclick={addAttachment}
          >Adicionar arquivo</Button
        >
      {/snippet}
    </SectionTitle>
    {#if attachments.length === 0 && stored.length === 0}
      <p class="empty-box" class:error={errorOf('anexos')}>
        Nenhum anexo. Use Adicionar arquivo (até 256 KiB somados).
      </p>
    {:else}
      {#each stored as a, i (a.name)}
        <Attachment
          name={a.name}
          detail="{readableSize(a.size)} · {attachmentVariable(a.name)}"
          onremove={() => removeStored(i)}
        />
      {/each}
      {#each attachments as a, i (a.id)}
        <Attachment
          name={a.name}
          detail="{readableSize(a.size)} · {attachmentVariable(a.name)}"
          onremove={() => removeAttachment(i)}
        />
      {/each}
    {/if}
    {#if attachmentNotice}<span class="error" role="alert" id="attachment-notice">{attachmentNotice}</span>{/if}
    {#if errorOf('anexos')}<span class="error" id="attachments-error">{errorOf('anexos')}</span>{/if}
  </section>

  <div class="level">
    <Icon name={level.icon} />
    <div class="level-texts">
      <span class="level-label">Nível de acesso</span>
      <span class="level-name">{level.label}</span>
      <span class="explanation">
        {editing
          ? level.explanation
          : 'Toda entrada nova começa assim. Mudar o nível pede o PIN.'}
      </span>
    </div>
    <Button variant="secondary" icon="lock" disabled>Mudar nível</Button>
  </div>
</form>

{#if generatorFor}
  <div class="popover-layer">
    <GeneratorPopover field={generatorFor} onuse={useGenerated} onclose={() => (generatorFor = '')} />
  </div>
{/if}

<footer class="footer">
  <span class="metadata">{editing ? '' : 'Obrigatórios: nome, grupo e ambiente.'}</span>
  {#if generalError}<Toast kind="error" message={generalError} />{/if}
  <Button variant="secondary" onclick={oncancel}>Cancelar</Button>
  <Button
    variant="primary"
    type="submit"
    form="credential-form"
    disabled={saving || empty}
  >
    {editing ? 'Salvar alterações' : 'Salvar'}
  </Button>
</footer>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .identification {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  .name {
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
  }
  .badges {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  .subtitle {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subtitle,
  .metadata {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .grid,
  .row.double {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-16);
  }
  .grid {
    row-gap: var(--space-20);
  }
  .grid > :global(*),
  .row > :global(*) {
    min-width: 0;
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }
  .row {
    display: flex;
    flex-direction: column;
  }
  .select-with-error {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .select-with-error > :global(.select) {
    min-width: 0;
  }
  .note {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .key-action {
    display: flex;
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .note-text {
    min-height: var(--space-96);
    padding: var(--space-12) var(--space-16);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-field);
    color: var(--color-text-primary);
    font: inherit;
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    resize: vertical;
  }
  .note-text:focus-visible {
    outline: none;
    border-color: var(--color-border-focus);
  }
  .note-text.with-error {
    border-color: var(--color-danger-border);
  }
  .note-text.secret {
    -webkit-text-security: disc;
  }
  .note-text::placeholder {
    color: var(--color-text-secondary);
  }
  .help,
  .error {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .error,
  .help.error {
    color: var(--color-danger-text);
  }
  .empty-box {
    margin: 0;
    padding: var(--space-16);
    border-radius: var(--radius-item);
    border: var(--line-thin) dashed var(--color-border-default);
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    text-align: center;
  }
  .empty-box.error {
    border-color: var(--color-danger-border);
    color: var(--color-danger-text);
  }
  .level {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .level-texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
  }
  .level-label,
  .explanation {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .level-name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .explanation {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .popover-layer {
    position: absolute;
    right: var(--space-24);
    bottom: var(--space-80);
    z-index: 3;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16) var(--space-24);
    border-top: var(--line-thin) solid var(--color-border-subtle);
  }
  .metadata {
    flex: 1;
  }
</style>
