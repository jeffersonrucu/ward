<script>
  // Generator popover (FigCraft 11671), opened by the generate action of the Password
  // field (frame 14307, the password generator in the form): width-dialog, padding
  // space-20, gap space-16, radius card, raised surface, default border,
  // shadow-floating. Header (title-section) with close; secret field of the generated
  // password with reveal and copy; length (label and the count of characters in
  // mono-small) with the slider; character sets in compact switches; footer with
  // generate again and use this password.
  //
  // S11/D4 (D65 f): the password is born in the Rust side (the generate password
  // command, length 8–64, symbols from the generator of `ward_core`) and stays there.
  // Use this password calls the use generated password command, which hands the password
  // to the field on the Rust side; here only the length comes back. The value only comes
  // in on "reveal" (30 s, hide or close).
  import { onDestroy, onMount } from 'svelte';
  import Button from './Button.svelte';
  import SecretField from './SecretField.svelte';
  import Slider from './Slider.svelte';
  import Icon from './Icon.svelte';
  import Switch from './Switch.svelte';
  import Toast from './Toast.svelte';
  import { call } from '../bridge/index.js';

  // Mirror of the min, max and default length of the generator in `ward_app_core`.
  const MIN_LENGTH = 8;
  const MAX_LENGTH = 64;
  const DEFAULT_LENGTH = 24;
  const REVEAL_DEADLINE_MS = 30_000;
  const COPIED_DEADLINE_MS = 2_000;
  // Thresholds of Phase 37 (bits): weak < 60, fair < 80, strong < 128, very strong.
  // The keys are the wire values of the strength.
  const STRENGTHS = {
    'weak': 'fraca',
    'fair': 'razoável',
    'strong': 'forte',
    'very-strong': 'muito forte',
  };

  /** @type {{ field: string, onuse: (length: number) => void, onclose: () => void }} */
  let { field, onuse, onclose } = $props();

  let length = $state(DEFAULT_LENGTH);
  // Character sets, by the wire names of the generate password command (m2:Phase 54):
  // lowercase, uppercase, digits, symbols.
  let charsets = $state({ lowercase: true, uppercase: true, digits: true, symbols: true });
  /** @type {{ size: number, bits: number, strength: string, symbols: string } | null} */
  let generated = $state(null);
  /** @type {string | null} */
  let revealed = $state(null);
  let copied = $state(false);
  let error = $state('');
  let alive = true;
  let revealDeadline = 0;
  let copiedDeadline = 0;
  /** @type {HTMLElement | undefined} */
  let box = $state();

  const onCount = $derived(Object.values(charsets).filter(Boolean).length);

  function hide() {
    clearTimeout(revealDeadline);
    revealed = null;
  }

  let generating = false;
  let again = false;
  async function generate() {
    hide();
    copied = false;
    if (generating) {
      again = true;
      return;
    }
    generating = true;
    try {
      do {
        again = false;
        try {
          const r = await call('generate_password', { size: length, ...charsets });
          if (alive) {
            generated = r;
            error = '';
          }
        } catch (e) {
          if (alive) {
            generated = null;
            error = e.message;
          }
        }
      } while (again && alive);
    } finally {
      generating = false;
    }
  }

  async function reveal() {
    try {
      const r = await call('reveal_generated_password');
      if (!alive || generating) return;
      revealed = r.value;
      clearTimeout(revealDeadline);
      revealDeadline = setTimeout(hide, REVEAL_DEADLINE_MS);
    } catch (e) {
      error = e.message;
    }
  }

  async function copy() {
    try {
      await call('copy_generated_password');
      copied = true;
      clearTimeout(copiedDeadline);
      copiedDeadline = setTimeout(() => (copied = false), COPIED_DEADLINE_MS);
    } catch (e) {
      error = e.message;
    }
  }

  async function use() {
    hide();
    try {
      const r = await call('use_generated_password', { field: field });
      onuse(r.size);
    } catch (e) {
      error = e.message;
    }
  }

  function changeLength(v) {
    length = Math.min(MAX_LENGTH, Math.max(MIN_LENGTH, Math.round(v)));
    generate();
  }

  function changeCharset(name, on) {
    if (!on && onCount <= 1) return;
    charsets[name] = on;
    generate();
  }

  function onKey(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    }
  }

  onMount(() => {
    generate();
    box?.focus();
  });

  onDestroy(() => {
    alive = false;
    hide();
    clearTimeout(copiedDeadline);
    // The generator password leaves the Rust side; the "used" one stays until save or
    // cancel.
    call('discard_generated_password').catch(() => {});
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="popover"
  role="dialog"
  aria-modal="false"
  aria-labelledby="generator-popover-title"
  tabindex="-1"
  bind:this={box}
  onkeydown={onKey}
>
  <header class="header">
    <h3 id="generator-popover-title" class="title">Gerar senha</h3>
    <button class="close" type="button" aria-label="Fechar o gerador" onclick={onclose}>
      <Icon name="close" />
    </button>
  </header>
  <SecretField
    id="popover-password"
    label="Senha gerada"
    empty={!generated}
    mask={'•'.repeat(generated?.size ?? 0)}
    help="A senha só aparece no revelar. Copiar limpa a área de transferência em 30 s."
    {revealed}
    {copied}
    onreveal={reveal}
    onhide={hide}
    oncopy={copy}
  />
  <div class="length">
    <div class="line">
      <label class="label" for="popover-length">Tamanho</label>
      <span class="length-value">{length} caracteres</span>
    </div>
    <Slider
      id="popover-length"
      accessibleLabel="Tamanho da senha"
      value={length}
      min={MIN_LENGTH}
      max={MAX_LENGTH}
      oninput={changeLength}
    />
    {#if generated}
      <span class="strength" data-strength={generated.strength}>
        {STRENGTHS[generated.strength] ?? ''} · cerca de {generated.bits} bits
      </span>
    {/if}
  </div>
  <div class="charsets">
    <div class="option-line">
      <Switch
        id="popover-lowercase"
        label="Minúsculas"
        on={charsets.lowercase}
        disabled={charsets.lowercase && onCount === 1}
        onchange={(v) => changeCharset('lowercase', v)}
      />
      <Switch
        id="popover-uppercase"
        label="Maiúsculas"
        on={charsets.uppercase}
        disabled={charsets.uppercase && onCount === 1}
        onchange={(v) => changeCharset('uppercase', v)}
      />
    </div>
    <div class="option-line">
      <Switch
        id="popover-digits"
        label="Dígitos"
        on={charsets.digits}
        disabled={charsets.digits && onCount === 1}
        onchange={(v) => changeCharset('digits', v)}
      />
      <Switch
        id="popover-symbols"
        label="Símbolos"
        on={charsets.symbols}
        disabled={charsets.symbols && onCount === 1}
        onchange={(v) => changeCharset('symbols', v)}
      />
    </div>
    {#if generated?.symbols}
      <span class="symbols" data-test="symbols">{generated.symbols}</span>
    {/if}
  </div>
  {#if error}<Toast kind="error" message={error} />{/if}
  <footer class="footer">
    <Button variant="ghost" icon="generator" onclick={generate}>Gerar de novo</Button>
    <Button variant="primary" onclick={use} disabled={!generated}>Usar esta senha</Button>
  </footer>
</div>

<style>
  .popover {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    width: var(--width-dialog);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-floating);
  }
  .popover:focus-visible {
    outline: none;
  }
  .header,
  .line,
  .footer {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .title {
    flex: 1;
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-md);
    line-height: var(--line-height-md);
  }
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .close:hover {
    background: var(--color-surface-hover);
  }
  .length,
  .charsets {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  .label {
    flex: 1;
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .length-value {
    color: var(--color-text-secondary);
    font-family: var(--font-family-mono);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .strength,
  .symbols {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .strength[data-strength='weak'] {
    color: var(--color-danger-text);
  }
  .strength[data-strength='fair'] {
    color: var(--color-warning-text);
  }
  .strength[data-strength='strong'],
  .strength[data-strength='very-strong'] {
    color: var(--color-success-text);
  }
  .symbols {
    overflow-wrap: anywhere;
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
  }
  .option-line {
    display: flex;
    gap: var(--space-12);
  }
  .option-line > :global(*) {
    flex: 1;
  }
  .footer {
    justify-content: flex-end;
  }
</style>
