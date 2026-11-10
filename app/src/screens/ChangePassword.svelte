<script>
  // Master password change button of Settings / General (Phase 38, V5.22). No frame of
  // its own in FigCraft: builds the modal (611) with three password fields (17511), the
  // first-use strength meter and the button (263).
  //
  // Asks for the current password (the daemon checks it: D52) and the new one twice,
  // with the first-use rule (12 characters or more; the Rust side checks again). The
  // three values live only in the local state of this component; they leave the state
  // before the reply of the request and on close. Nothing goes to storage, URL or log.
  //
  // Phase 54 (final audit): notice that the old backups still open with the old
  // password (warning text color, like the note of `ChangePin`).
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import { call } from '../lib/bridge/index.js';
  import {
    MENSAGENS as MESSAGES,
    erroDaSenha as passwordError,
    forcaDaSenha as passwordStrength,
    mensagemDoRust as rustMessage,
  } from './primeiro-uso.js';

  /**
   * @type {{ open?: boolean, onclose?: () => void, onchanged?: () => void,
   *   onfailure?: (code: string) => void }}
   */
  let { open = false, onclose = undefined, onchanged = undefined, onfailure = undefined } = $props();

  let current = $state('');
  let next = $state('');
  let confirmation = $state('');
  // Keyed by the wire field names that the errors of the reply use.
  let errors = $state({ 'atual': '', 'nova': '', 'confirmacao': '' });
  let failure = $state('');
  let sending = $state(false);
  let currentField = $state();
  let nextField = $state();
  let confirmationField = $state();

  const strength = $derived(passwordStrength(next));

  function clear() {
    current = '';
    next = '';
    confirmation = '';
  }

  function clearErrors() {
    errors = { 'atual': '', 'nova': '', 'confirmacao': '' };
    failure = '';
  }

  $effect(() => {
    if (open) {
      clear();
      clearErrors();
      tick().then(() => currentField?.focus());
    }
  });

  onDestroy(clear);

  function close() {
    clear();
    clearErrors();
    onclose?.();
  }

  async function submit(e) {
    e?.preventDefault();
    if (sending) return;
    clearErrors();
    if (!current) {
      errors.atual = 'Digite a senha mestra atual.';
      currentField?.focus();
      return;
    }
    const short = passwordError(next);
    if (short) {
      errors.nova = short;
      nextField?.focus();
      return;
    }
    if (confirmation !== next) {
      errors.confirmacao = MESSAGES.senhaDiferente;
      confirmationField?.focus();
      return;
    }
    // The values leave the state before the reply: the screen keeps them not even in the error.
    const args = { 'dados': { 'atual': current, 'nova': next, 'confirmacao': confirmation } };
    clear();
    sending = true;
    try {
      const r = await call('trocar_senha_mestra', args);
      if (r?.trocado) {
        onchanged?.();
        return;
      }
      for (const item of r?.erros ?? []) {
        if (item.campo in errors) errors[item.campo] = rustMessage(item.motivo);
      }
    } catch (err) {
      if (err?.codigo === 'senha-incorreta') {
        errors.atual = 'Senha mestra incorreta. Nada foi alterado.';
      } else {
        failure = rustMessage(err?.mensagem) || 'Não foi possível trocar a senha mestra.';
        onfailure?.(err?.codigo ?? '');
      }
    } finally {
      args.dados = null;
      sending = false;
    }
    await tick();
    currentField?.focus();
  }
</script>

<Modal title="Trocar senha mestra" {open} onclose={close}>
  <form class="body" onsubmit={submit} novalidate data-test="change-password">
    <p class="explanation">
      A senha atual confirma a troca. Depois dela, o cofre só abre com a nova.
    </p>
    <p class="warning" data-test="change-password-backups-notice">
      Os backups antigos, no computador e no pendrive, continuam abrindo com a senha antiga.
      Se você troca porque a senha antiga vazou, apague esses backups.
    </p>
    <PasswordField
      label="Senha mestra atual"
      id="change-password-current"
      bind:value={current}
      bind:input={currentField}
      autocomplete="current-password"
      error={errors.atual}
    />
    <PasswordField
      label="Nova senha mestra"
      id="change-password-new"
      bind:value={next}
      bind:input={nextField}
      help="Mínimo de 12 caracteres."
      error={errors.nova}
    />
    <div class="strength" aria-live="polite">
      <div class="bars" aria-hidden="true">
        {#each [1, 2, 3, 4] as n (n)}
          <span class="bar" data-tone={strength && n <= strength.segmentos ? strength.tom : undefined}
          ></span>
        {/each}
      </div>
      <span class="caption" data-tone={strength?.tom}>
        {strength ? `Força: ${strength.rotulo}` : 'Força: digite a senha'}
      </span>
    </div>
    <PasswordField
      label="Confirmar nova senha"
      id="change-password-confirmation"
      bind:value={confirmation}
      bind:input={confirmationField}
      error={errors.confirmacao}
    />
    {#if failure}<p class="failure" role="alert">{failure}</p>{/if}
    <button class="visually-hidden" type="submit" tabindex="-1">Trocar senha</button>
  </form>
  {#snippet footer()}
    <Button variant="secondary" onclick={close}>Cancelar</Button>
    <Button variant="primary" disabled={sending} onclick={submit}>Trocar senha</Button>
  {/snippet}
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    margin: var(--space-0);
    color: var(--color-text-primary);
  }
  p {
    margin: var(--space-0);
  }
  .explanation {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-secondary);
  }
  .warning {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-warning-text);
  }
  .failure {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-danger-text);
  }
  .strength {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .bars {
    display: flex;
    gap: var(--space-6);
  }
  .bar {
    flex: 1;
    height: var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--color-border-default);
  }
  .bar[data-tone='perigo'] {
    background: var(--color-danger-default);
  }
  .bar[data-tone='aviso'] {
    background: var(--color-warning-default);
  }
  .bar[data-tone='sucesso'] {
    background: var(--color-success-default);
  }
  .caption {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .caption[data-tone='perigo'] {
    color: var(--color-danger-text);
  }
  .caption[data-tone='aviso'] {
    color: var(--color-warning-text);
  }
  .caption[data-tone='sucesso'] {
    color: var(--color-success-text);
  }
</style>
