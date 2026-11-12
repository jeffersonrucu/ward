<script>
  // TOTP code (FigCraft 6909): label, field box with the code in mono-body (JetBrains
  // Mono 14), countdown pill (clock icon 12 and "18 s" in text-tag) and the Copy field
  // action; help about the 30 s period. Empty (no seed): disabled box and a help
  // that says the code shows once the seed is saved.
  //
  // The code comes from the Rust side (the TOTP code command); the seed never gets here.
  // Copy asks the Rust side (the copy TOTP code command).
  //
  // Phase 65b: the label is a property (the current-code text in the form, the TOTP code
  // text in the detail; the label property of FigCraft 6909), and the 6 digits show in
  // two groups of 3, as in the component; copy takes the digits only, from Rust.
  import FieldAction from './FieldAction.svelte';
  import Icon from './Icon.svelte';

  /**
   * @type {{ id: string, label?: string, code?: string | null, remaining?: number,
   *   copied?: boolean, oncopy?: () => void, emptyText?: string, emptyHelp?: string }}
   */
  let {
    id,
    label = 'Código atual',
    code = null,
    remaining = 0,
    copied = false,
    oncopy = undefined,
    emptyText = 'Sem semente',
    emptyHelp = 'Aparece com a semente salva.',
  } = $props();

  const shown = $derived(
    typeof code === 'string' && /^[0-9]{6}$/.test(code) ? `${code.slice(0, 3)} ${code.slice(3)}` : code,
  );
</script>

<div class="field">
  <span class="label" id="{id}-label">{label}</span>
  <div class="box" class:disabled={!code} role="group" aria-labelledby="{id}-label">
    {#if code}
      <span class="code" data-test="codigo-totp">{shown}</span>
      <span class="countdown" aria-label="troca em {remaining} segundos">
        <Icon name="clock" size="xs" />{remaining} s
      </span>
      <FieldAction
        label="Copiar"
        {copied}
        onclick={() => oncopy?.()}
        aria-label="Copiar código"
      />
    {:else}
      <span class="code empty">{emptyText}</span>
    {/if}
  </div>
  <span class="help">{code ? 'Novo código a cada 30 s.' : emptyHelp}</span>
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: 0;
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .box {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    height: var(--control-default);
    padding: var(--space-0) var(--space-8) var(--space-0) var(--space-16);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-field);
  }
  .box.disabled {
    background: var(--color-surface-disabled);
  }
  .code {
    flex: 1;
    font-family: var(--font-family-mono);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    font-variant-ligatures: none;
  }
  .code.empty {
    font-family: var(--font-family-text);
    color: var(--color-text-disabled);
  }
  .countdown {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-8);
    border-radius: var(--radius-pill);
    background: var(--color-badge-neutral-bg);
    color: var(--color-text-secondary);
    font-weight: var(--weight-medium);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .help {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
</style>
