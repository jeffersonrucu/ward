<script>
  // Level dialog (FigCraft set 967): choose (968), PIN (1027), wrong PIN (1078). Width
  // width-wide-dialog, `hug`, radius window, padding space-24, gap space-20, background
  // color-surface-raised, border color-border-default, shadow-modal.
  // - `header` (gap space-12): title (title-modal) and a 32 px close (control-sm, radius
  //   sm) with the close icon;
  // - `entry` (padding space-12, gap space-12, radius card, background
  //   color-surface-default, border color-border-subtle): key icon, name
  //   (text-body-strong), type badge and environment badge;
  // - Choose: `body` gap space-12 with the explanation, the three level options and the
  //   current level line (text-caption); footer cancel and continue;
  // - PIN: `body` gap space-20 with the "from ... to ..." line (gap space-8; the new one
  //   in color-text-brand), the PIN field and the note (5 errors lock); footer back
  //   and confirm. On a wrong PIN, the error in the field and the note in
  //   color-warning-text (the level stays as it was).
  //
  // S9: the PIN stays only in the local state of this component, leaves it before the
  // answer of the request (the field goes back empty) and is gone on close. Nothing goes
  // to storage, URL or log. The 5th error in a row locks the vault in the daemon: after
  // each wrong PIN the dialog asks for the status and, locked, calls `onlocked`.
  import { tick } from 'svelte';
  import EnvironmentBadge from './EnvironmentBadge.svelte';
  import TypeBadge from './TypeBadge.svelte';
  import Button from './Button.svelte';
  import PinField from './PinField.svelte';
  import Icon from './Icon.svelte';
  import LevelOption from './LevelOption.svelte';
  import { LEVEL_ORDER, accessLevel } from './levels.js';
  import { call } from '../bridge/index.js';

  /**
   * Wire fields of the entry (m2:Phase 54): name, type, environment, level.
   * @typedef {{ name: string, type: string, environment: 'dev' | 'stage' | 'prod', level: string }} Entry
   * @type {{
   *   open?: boolean, entry: Entry | null,
   *   onclose?: () => void, onchanged?: (level: string) => void, onlocked?: () => void
   * }}
   */
  let {
    open = false,
    entry,
    onclose = undefined,
    onchanged = undefined,
    onlocked = undefined,
  } = $props();

  const PIN_ID = 'nivel-pin';

  let dialog = $state();
  /** The choose step or the PIN step (literal values). */
  let step = $state('escolher');
  let chosen = $state('');
  let pin = $state('');
  let wrongPin = $state(false);
  let error = $state('');
  let sending = $state(false);

  const current = $derived(accessLevel(entry?.level) ?? accessLevel('approval'));
  const next = $derived(accessLevel(chosen));

  function reset() {
    step = 'escolher';
    chosen = entry?.level ?? '';
    pin = '';
    wrongPin = false;
    error = '';
    sending = false;
  }

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      reset();
      dialog.showModal();
    }
    if (!open && dialog.open) dialog.close();
  });

  function close() {
    pin = '';
    onclose?.();
  }

  async function proceed() {
    if (!next || chosen === entry?.level) return;
    step = 'pin';
    pin = '';
    wrongPin = false;
    error = '';
    await tick();
    document.getElementById(PIN_ID)?.focus();
  }

  function back() {
    pin = '';
    wrongPin = false;
    error = '';
    step = 'escolher';
  }

  async function confirm() {
    if (sending || pin.length < 4 || !entry || !next) return;
    // The PIN leaves the state before the answer: the screen does not keep it, not even
    // on error.
    const args = { entry: entry.name, level: chosen, pin };
    pin = '';
    sending = true;
    error = '';
    try {
      const r = await call('set_level', args);
      args.pin = '';
      sending = false;
      onchanged?.(r.level);
      return;
    } catch (e) {
      args.pin = '';
      sending = false;
      if (e.code === 'wrong-pin') {
        wrongPin = true;
        if (await isLocked()) return;
      } else if (e.code === 'locked') {
        onlocked?.();
        return;
      } else {
        wrongPin = false;
        error = e.message;
      }
    }
    await tick();
    document.getElementById(PIN_ID)?.focus();
  }

  /** After a wrong PIN: the 5th in a row locks the vault (the daemon counts). */
  async function isLocked() {
    try {
      const r = await call('vault_status');
      if (r?.status === 'unlocked') return false;
    } catch {
      // No answer: treat as locked; the unlock screen asks again.
    }
    onlocked?.();
    return true;
  }
</script>

<dialog
  bind:this={dialog}
  class="dialog"
  aria-labelledby="level-title"
  data-step={wrongPin ? 'pin-errado' : step}
  onclose={close}
>
  <div class="header">
    <h2 id="level-title">{step === 'escolher' ? 'Nível de acesso' : 'Confirmar com o PIN'}</h2>
    <button class="close" type="button" aria-label="Fechar" onclick={close}>
      <Icon name="close" />
    </button>
  </div>

  {#if entry}
    <div class="entry">
      <Icon name="key" />
      <span class="entry-name">{entry.name}</span>
      <TypeBadge kind={entry.type} />
      <EnvironmentBadge environment={entry.environment} />
    </div>
  {/if}

  {#if step === 'escolher'}
    <div class="body choose" role="radiogroup" aria-labelledby="level-title">
      <p class="explanation">
        O ambiente não muda as regras: dev, stage e prod têm os mesmos três níveis. Mudar o
        nível pede o PIN.
      </p>
      {#each LEVEL_ORDER as n (n)}
        <LevelOption
          level={n}
          name="nivel-de-acesso"
          checked={chosen === n}
          onchoose={(v) => (chosen = v)}
        />
      {/each}
      <p class="current">Nível atual: {current.lower}.</p>
    </div>
    <div class="footer">
      <Button variant="secondary" onclick={close}>Cancelar</Button>
      <Button
        variant="primary"
        disabled={!next || chosen === entry?.level}
        onclick={proceed}>Continuar</Button
      >
    </div>
  {:else}
    <form
      class="body pin"
      onsubmit={(e) => {
        e.preventDefault();
        confirm();
      }}
    >
      <p class="change">
        <span class="weak">De</span>
        <span class="strong">{current.lower}</span>
        <span class="weak">para</span>
        <span class="strong brand">{next?.lower}</span>
      </p>
      <PinField
        label="PIN"
        id={PIN_ID}
        bind:value={pin}
        help="O PIN confirma a mudança. Ele não destrava o cofre."
        error={wrongPin ? 'PIN incorreto. Nada foi alterado.' : error}
      />
      {#if wrongPin}
        <p class="note warning" role="alert">
          O nível continua {current.lower}. 5 erros de PIN seguidos travam o cofre.
        </p>
      {:else}
        <p class="note">5 erros de PIN seguidos travam o cofre.</p>
      {/if}
    </form>
    <div class="footer">
      <Button variant="secondary" onclick={back}>Voltar</Button>
      <Button variant="primary" disabled={sending || pin.length < 4} onclick={confirm}
        >Confirmar</Button
      >
    </div>
  {/if}
</dialog>

<style>
  .dialog {
    width: var(--width-wide-dialog);
    max-width: calc(100vw - 2 * var(--space-24));
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-modal);
  }
  .dialog[open] {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
  }
  .dialog::backdrop {
    background: var(--color-bg-overlay);
  }
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  h2 {
    flex: 1;
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    padding: var(--space-0);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
  }
  .entry {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-12);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .entry-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .body {
    display: flex;
    flex-direction: column;
    margin: var(--space-0);
  }
  .body.choose {
    gap: var(--space-12);
  }
  .body.pin {
    gap: var(--space-20);
  }
  p {
    margin: var(--space-0);
  }
  .explanation,
  .note {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .note.warning {
    color: var(--color-warning-text);
  }
  .current {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .change {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  .weak {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .strong {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .brand {
    color: var(--color-text-brand);
  }
  .footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-12);
    padding-top: var(--space-4);
  }
</style>
