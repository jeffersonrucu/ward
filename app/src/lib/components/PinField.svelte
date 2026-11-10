<script>
  // PIN field (FigCraft set 650): empty, typing, complete, error, blocked. Digits 48×48
  // (control-lg), radius control, gap space-12, mono-pin mask. A full digit with
  // color-border-default and the error with color-danger-text (1055 and 1106, checked in
  // Phase 35).
  //
  // The PIN is a single <input> (numeric inputmode, no autocomplete); the boxes are only
  // drawing. The value stays in the local state of the component and the owner screen
  // clears it after sending. Accepts only digits and at most `digits` (4 to 6).

  /**
   * @type {{ label: string, id: string, value?: string, digits?: 4 | 5 | 6,
   *   help?: string, error?: string, blocked?: boolean, onsubmit?: (pin: string) => void }}
   */
  let {
    label,
    id,
    value = $bindable(''),
    digits = 6,
    help = '',
    error = '',
    blocked = false,
    onsubmit = undefined,
  } = $props();

  function onType(e) {
    const clean = e.currentTarget.value.replace(/\D/g, '').slice(0, digits);
    e.currentTarget.value = clean;
    value = clean;
  }

  function onKey(e) {
    if (e.key === 'Enter' && value.length >= 4) onsubmit?.(value);
  }
</script>

<div class="pin" class:has-error={error} class:blocked>
  <label class="label" for={id}>{label}</label>
  <div class="boxes">
    <input
      {id}
      class="input"
      type="password"
      inputmode="numeric"
      autocomplete="off"
      maxlength={digits}
      {value}
      disabled={blocked}
      aria-invalid={error ? 'true' : undefined}
      oninput={onType}
      onkeydown={onKey}
    />
    {#each Array.from({ length: digits }, (_, i) => i) as i (i)}
      <span class="digit" class:full={i < value.length} class:current={i === value.length} aria-hidden="true">
        {i < value.length ? '•' : ''}
      </span>
    {/each}
  </div>
  {#if blocked}
    <span class="help error">PIN bloqueado: o cofre foi travado.</span>
  {:else if error}
    <span class="help error">{error}</span>
  {:else if help}
    <span class="help">{help}</span>
  {/if}
</div>

<style>
  .pin {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .boxes {
    position: relative;
    display: flex;
    gap: var(--space-12);
  }
  .input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: text;
  }
  .digit {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-field);
    font-family: var(--font-family-mono);
    font-weight: var(--weight-bold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
  }
  .digit.full {
    border-color: var(--color-border-default);
  }
  .boxes:focus-within .current,
  .boxes:focus-within .full:last-of-type {
    /* V5.25: with every digit full, the focus stays visible on the last one. */
    border-color: var(--color-border-focus);
  }
  .has-error .digit {
    border-color: var(--color-danger-text);
  }
  .blocked .digit {
    background: var(--color-surface-disabled);
    color: var(--color-text-disabled);
  }
  .help {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .error {
    color: var(--color-danger-text);
  }
</style>
