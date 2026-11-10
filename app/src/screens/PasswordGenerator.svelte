<script>
  // Standalone password generator screen (FigCraft 13141, Phase 37; V5.21).
  //
  // S11/D4: the password is born in the Rust side (`ward_core`'s generator, through the
  // generate password command) and stays there. This screen gets only the length, the
  // strength and the symbol set of D35. The value enters here only on "reveal" and
  // leaves in 30 s, on hide, on generating again or on leaving the screen. "Copy" asks
  // the Rust side (the copy generated password command), which uses the same clipboard
  // path as the copy field command (cleared in 30 s, V5.5). On leaving the screen, the
  // discard generated password command zeroes the password in the Rust side. Nothing
  // goes to browser storage, the URL or global state.
  import { onDestroy, onMount } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import SecretField from '../lib/components/SecretField.svelte';
  import Slider from '../lib/components/Slider.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import OptionRow from '../lib/components/OptionRow.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';

  // Mirror of the min, max and default length of the generator in `ward_app_core` (the
  // Tauri config test checks it). The Rust side refuses any other length.
  const MIN_LENGTH = 8;
  const MAX_LENGTH = 64;
  const DEFAULT_LENGTH = 24;
  const REVEAL_DEADLINE_MS = 30_000;
  const COPIED_DEADLINE_MS = 2_000;

  // The keys are the wire values of the strength.
  const STRENGTHS = {
    'fraca': { label: 'fraca', segments: 1, tone: 'danger' },
    'razoavel': { label: 'razoável', segments: 2, tone: 'warning' },
    'forte': { label: 'forte', segments: 3, tone: 'success' },
    'muito-forte': { label: 'muito forte', segments: 4, tone: 'success' },
  };

  let length = $state(DEFAULT_LENGTH);
  // Character sets, by the wire names of the generate password command (frozen until
  // Phase 54): lowercase, uppercase, digits, symbols.
  let charsets = $state({ 'minusculas': true, 'maiusculas': true, 'digitos': true, 'simbolos': true });
  /** @type {{ tamanho: number, bits: number, forca: string, simbolos: string } | null} */
  let generated = $state(null);
  /** @type {string | null} */
  let revealed = $state(null);
  let copied = $state(false);
  /** @type {string | null} */
  let error = $state(null);

  const strength = $derived(generated ? STRENGTHS[generated.forca] : null);
  const onCount = $derived(Object.values(charsets).filter(Boolean).length);

  let revealDeadline = 0;
  let copiedDeadline = 0;
  let alive = true;

  function hide() {
    clearTimeout(revealDeadline);
    revealed = null;
  }

  // One request at a time: if the options change during a request, generate again at
  // the end, so the password kept in the Rust side is always the one of the screen's
  // options.
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
          const r = await call('gerar_senha', { 'tamanho': length, ...charsets });
          if (alive) {
            generated = r;
            error = null;
          }
        } catch (e) {
          if (alive) {
            generated = null;
            error = e.mensagem;
          }
        }
      } while (again && alive);
    } finally {
      generating = false;
    }
  }

  async function reveal() {
    try {
      const r = await call('revelar_senha_gerada');
      if (!alive || generating) return;
      revealed = r.valor;
      clearTimeout(revealDeadline);
      revealDeadline = setTimeout(hide, REVEAL_DEADLINE_MS);
    } catch (e) {
      error = e.mensagem;
    }
  }

  async function copy() {
    try {
      await call('copiar_senha_gerada');
      copied = true;
      error = null;
      clearTimeout(copiedDeadline);
      copiedDeadline = setTimeout(() => (copied = false), COPIED_DEADLINE_MS);
    } catch (e) {
      error = e.mensagem;
    }
  }

  function changeLength(v) {
    length = Math.min(MAX_LENGTH, Math.max(MIN_LENGTH, Math.round(v)));
    generate();
  }

  function changeCharset(name, on) {
    // The last set that is on does not turn off (the Rust side also refuses no set).
    if (!on && onCount <= 1) return;
    charsets[name] = on;
    generate();
  }

  onMount(generate);

  onDestroy(() => {
    alive = false;
    hide();
    clearTimeout(copiedDeadline);
    call('descartar_senha_gerada').catch(() => {});
  });
</script>

<section class="page" aria-labelledby="generator-title">
  <header class="top">
    <h1 id="generator-title" class="title-page">Gerador de senha</h1>
    <p class="summary">Gera senhas para usar fora do cofre. Nada do que é gerado aqui fica salvo.</p>
  </header>

  <div class="columns">
    <section class="options" aria-labelledby="options-title">
      <h2 id="options-title" class="title-section">Opções</h2>
      <div class="length">
        <div class="row">
          <label class="label" for="generator-length">Tamanho</label>
          <span class="length-value">{length} caracteres</span>
        </div>
        <Slider
          id="generator-length"
          accessibleLabel="Tamanho da senha"
          value={length}
          min={MIN_LENGTH}
          max={MAX_LENGTH}
          oninput={changeLength}
        />
        <p class="help">
          De {MIN_LENGTH} a {MAX_LENGTH}. Use menos só quando o painel limitar o tamanho.
        </p>
      </div>
      <OptionRow
        id="generator-lowercase"
        title="Minúsculas"
        on={charsets.minusculas}
        disabled={charsets.minusculas && onCount === 1}
        onchange={(v) => changeCharset('minusculas', v)}
      >
        {#snippet description()}a a z{/snippet}
      </OptionRow>
      <OptionRow
        id="generator-uppercase"
        title="Maiúsculas"
        on={charsets.maiusculas}
        disabled={charsets.maiusculas && onCount === 1}
        onchange={(v) => changeCharset('maiusculas', v)}
      >
        {#snippet description()}A a Z{/snippet}
      </OptionRow>
      <OptionRow
        id="generator-digits"
        title="Dígitos"
        on={charsets.digitos}
        disabled={charsets.digitos && onCount === 1}
        onchange={(v) => changeCharset('digitos', v)}
      >
        {#snippet description()}0 a 9{/snippet}
      </OptionRow>
      <OptionRow
        id="generator-symbols"
        title="Símbolos"
        on={charsets.simbolos}
        disabled={charsets.simbolos && onCount === 1}
        onchange={(v) => changeCharset('simbolos', v)}
      >
        {#snippet description()}<span class="symbols" data-test="symbols">{generated?.simbolos ?? ''}</span>
          Desligue se o painel não aceitar.{/snippet}
      </OptionRow>
    </section>

    <section class="result" aria-labelledby="result-title">
      <header class="header">
        <span class="avatar"><Icon name="generator" size="lg" /></span>
        <h2 id="result-title" class="title-modal">Senha gerada</h2>
      </header>

      <div class="body">
        <SecretField
          id="generator-password"
          label="Senha"
          empty={!generated}
          mask={'•'.repeat(generated?.tamanho ?? 0)}
          revealed={revealed}
          copied={copied}
          onreveal={reveal}
          onhide={hide}
          oncopy={copy}
        />

        {#if strength}
          <div class="strength">
            <div class="row">
              <span class="label">Força</span>
              <span class="strength-value" data-tone={strength.tone}>
                {strength.label} · cerca de {generated.bits} bits
              </span>
            </div>
            <div class="bar" aria-hidden="true">
              {#each [1, 2, 3, 4] as n (n)}
                <span class="segment" data-tone={n <= strength.segments ? strength.tone : undefined}
                ></span>
              {/each}
            </div>
          </div>
        {/if}

        <div class="note">
          <Icon name="clock" />
          <div class="texts">
            <span class="note-title">Não fica no cofre</span>
            <span class="note-text">
              Para guardar uma senha gerada, abra a credencial e use o gerador no campo Senha.
              Copiar limpa a área de transferência em 30 s.
            </span>
          </div>
        </div>

        {#if error}<Toast kind="error" message={error} />{/if}
      </div>

      <footer class="footer">
        <span class="metadata">Gerada agora · não fica no cofre</span>
        <Button variant="secondary" icon="generator" onclick={generate}>Gerar de novo</Button>
        <!-- The copy feedback shows in the field action (the copied variant of the design). -->
        <Button variant="primary" icon="copy" onclick={copy} disabled={!generated}>
          Copiar
        </Button>
      </footer>
    </section>
  </div>
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    flex: 1;
    min-width: 0;
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-8) var(--space-0);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .summary {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .columns {
    display: flex;
    gap: var(--space-16);
    flex: 1;
    min-height: 0;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    flex: none;
    width: var(--width-column-list);
    padding: var(--space-24);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow-y: auto;
  }
  .length {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding: var(--space-20);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-8);
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
  .help {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .symbols {
    overflow-wrap: anywhere;
    /* Inter joins `<=>` into an arrow: the symbols of D35 show one by one. */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
  }
  .result {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
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
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .strength {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .strength-value {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .strength-value[data-tone='danger'] {
    color: var(--color-danger-text);
  }
  .strength-value[data-tone='warning'] {
    color: var(--color-warning-text);
  }
  .strength-value[data-tone='success'] {
    color: var(--color-success-text);
  }
  .bar {
    display: flex;
    gap: var(--space-4);
  }
  .segment {
    flex: 1;
    height: var(--space-6);
    border-radius: var(--radius-pill);
    background: var(--color-surface-hover);
  }
  .segment[data-tone='danger'] {
    background: var(--color-danger-default);
  }
  .segment[data-tone='warning'] {
    background: var(--color-warning-default);
  }
  .segment[data-tone='success'] {
    background: var(--color-success-default);
  }
  .note {
    display: flex;
    align-items: flex-start;
    gap: var(--space-12);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
  }
  .note-title {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .note-text {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
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
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
</style>
