<script>
  // Approval PIN change button of Settings / General (Phase 38, V5.22, S9). Frame
  // Settings / Change PIN (FigCraft 20193; Phase 39d), with the change dialog, PIN type
  // (19979, instance 20471): modal (611) with three PIN fields (650) and the button
  // (263).
  //
  // Asks for the current PIN (the daemon checks it; the 5th error in a row locks the
  // vault) and the new one twice. The values live only in the local state of this
  // component; they leave the state before the reply of the request and on close.
  // Nothing goes to storage, URL or log.
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import { call } from '../lib/bridge/index.js';
  import {
    MAX_DIGITOS_PIN as MAX_PIN_DIGITS,
    MENSAGENS as MESSAGES,
    mensagemDoRust as rustMessage,
    pinValido as validPin,
  } from './primeiro-uso.js';

  /**
   * @type {{ open?: boolean, onclose?: () => void, onchanged?: () => void,
   *   onlocked?: () => void, onfailure?: (code: string) => void }}
   */
  let {
    open = false,
    onclose = undefined,
    onchanged = undefined,
    onlocked = undefined,
    onfailure = undefined,
  } = $props();

  // Keyed by the wire field names that the errors of the reply use.
  const IDS = { 'atual': 'change-pin-current', 'novo': 'change-pin-new', 'confirmacao': 'change-pin-confirmation' };

  let current = $state('');
  let next = $state('');
  let confirmation = $state('');
  let errors = $state({ 'atual': '', 'novo': '', 'confirmacao': '' });
  let wrongPin = $state(false);
  let failure = $state('');
  let sending = $state(false);

  function clear() {
    current = '';
    next = '';
    confirmation = '';
  }

  function clearErrors() {
    errors = { 'atual': '', 'novo': '', 'confirmacao': '' };
    wrongPin = false;
    failure = '';
  }

  async function focusField(field) {
    await tick();
    document.getElementById(IDS[field])?.focus();
  }

  $effect(() => {
    if (open) {
      clear();
      clearErrors();
      focusField('atual');
    }
  });

  onDestroy(clear);

  function close() {
    clear();
    clearErrors();
    onclose?.();
  }

  // Letter, symbol or 7th digit: the PIN field does not take it, and the refusal shows
  // in the field (the same notice as the first use).
  function refusal(field) {
    return (e) => {
      const text = e.data ?? e.dataTransfer?.getData('text/plain') ?? '';
      if (!text) return;
      const value = { 'atual': current, 'novo': next, 'confirmacao': confirmation }[field];
      const noSelection = e.target?.selectionStart === e.target?.selectionEnd;
      if (/\D/.test(text)) errors[field] = MESSAGES.pinComLetra;
      else if (noSelection && value.length + text.length > MAX_PIN_DIGITS) errors[field] = MESSAGES.pinLongo;
      else errors[field] = '';
    };
  }

  async function submit(e) {
    e?.preventDefault();
    if (sending) return;
    clearErrors();
    if (!validPin(current)) {
      errors.atual = MESSAGES.pinInvalido;
      return focusField('atual');
    }
    if (!validPin(next)) {
      errors.novo = MESSAGES.pinInvalido;
      return focusField('novo');
    }
    if (confirmation !== next) {
      errors.confirmacao = MESSAGES.pinDiferente;
      return focusField('confirmacao');
    }
    // The PINs leave the state before the reply: the screen keeps them not even in the error.
    const args = { 'dados': { 'atual': current, 'novo': next, 'confirmacao': confirmation } };
    clear();
    sending = true;
    try {
      const r = await call('trocar_pin', args);
      if (r?.trocado) {
        onchanged?.();
        return;
      }
      for (const item of r?.erros ?? []) {
        if (item.campo in errors) errors[item.campo] = rustMessage(item.motivo);
      }
    } catch (err) {
      if (err?.codigo === 'pin-incorreto') {
        wrongPin = true;
        errors.atual = 'PIN incorreto. Nada foi alterado.';
        if (await locked()) return;
      } else if (err?.codigo === 'travado') {
        onlocked?.();
        return;
      } else {
        failure = rustMessage(err?.mensagem) || 'Não foi possível trocar o PIN.';
        onfailure?.(err?.codigo ?? '');
      }
    } finally {
      args.dados = null;
      sending = false;
    }
    await focusField('atual');
  }

  /** After a wrong PIN: the 5th in a row locks the vault (the daemon counts). */
  async function locked() {
    try {
      const r = await call('estado_do_cofre');
      if (r?.estado === 'destravado') return false;
    } catch {
      // No reply, treat it as locked: the unlock screen asks again.
    }
    onlocked?.();
    return true;
  }
</script>

<Modal title="Trocar PIN" {open} onclose={close}>
  <form class="body" onsubmit={submit} novalidate data-test="change-pin">
    <p class="explanation">
      O PIN aprova usos e mudanças de nível; ele não destrava o cofre. O PIN atual confirma
      a troca.
    </p>
    <div class="pin" onbeforeinput={refusal('atual')}>
      <PinField label="PIN atual" id={IDS.atual} bind:value={current} error={errors.atual} />
    </div>
    <div class="pin" onbeforeinput={refusal('novo')}>
      <PinField
        label="Novo PIN"
        id={IDS.novo}
        bind:value={next}
        help="4 a 6 dígitos."
        error={errors.novo}
      />
    </div>
    <div class="pin" onbeforeinput={refusal('confirmacao')}>
      <PinField
        label="Confirmar novo PIN"
        id={IDS.confirmacao}
        bind:value={confirmation}
        help="Digite o mesmo PIN de novo."
        error={errors.confirmacao}
      />
    </div>
    {#if wrongPin}
      <p class="note warning" role="alert">O PIN continua o mesmo. 5 erros de PIN seguidos travam o cofre.</p>
    {:else}
      <p class="note">5 erros de PIN seguidos travam o cofre.</p>
    {/if}
    {#if failure}<p class="failure" role="alert">{failure}</p>{/if}
    <button class="visually-hidden" type="submit" tabindex="-1">Trocar PIN</button>
  </form>
  {#snippet footer()}
    <Button variant="secondary" onclick={close}>Cancelar</Button>
    <Button variant="primary" disabled={sending} onclick={submit}>Trocar PIN</Button>
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
  /* Phase 54j: the explanation is the body text style (`8:12`, 14/20) as in frame
     `20193` › `20478`; the note is `8:14` (13/18). */
  .explanation {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-secondary);
  }
  .note {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .note.warning {
    color: var(--color-warning-text);
  }
  .failure {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-danger-text);
  }
</style>
