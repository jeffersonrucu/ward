<script>
  // Password field (set of Phase 10b; focus 17524, default 17511): label in text-label,
  // gap space-8; input height control-default, radius control, padding 0 space-8 0
  // space-16, gap space-8, background color-surface-field, border color-border-subtle
  // (focus: color-border-focus in line-thick); left icon lock; 32×32 reveal action
  // (control-sm); help or error in text-caption.
  //
  // For the typed master password (first use, unlock). The value belongs to the user
  // and stays only in the state of the owner component; the eye shows the typing in the
  // field itself and hides it again after 30 s. No autocomplete and no spell checker.
  import { onDestroy } from 'svelte';
  import Icon from './Icon.svelte';

  const VISIBLE_DEADLINE_MS = 30_000;

  /**
   * @type {{
   *   label: string, id: string, value?: string, placeholder?: string, help?: string,
   *   error?: string, disabled?: boolean, input?: HTMLInputElement,
   *   [k: string]: unknown
   * }}
   */
  let {
    label,
    id,
    value = $bindable(''),
    placeholder = '',
    help = '',
    error = '',
    disabled = false,
    input = $bindable(),
    ...rest
  } = $props();

  let visible = $state(false);
  let deadline = null;

  function toggle() {
    visible = !visible;
    clearTimeout(deadline);
    if (visible) deadline = setTimeout(() => (visible = false), VISIBLE_DEADLINE_MS);
  }

  onDestroy(() => clearTimeout(deadline));
  const description = $derived(error || help ? `${id}-help` : undefined);
</script>

<div class="field" class:has-error={error}>
  <label class="label" for={id}>{label}</label>
  <div class="box" class:disabled>
    <Icon name="lock" tone={disabled ? 'disabled' : undefined} />
    <input
      bind:this={input}
      {id}
      class="input"
      type={visible ? 'text' : 'password'}
      bind:value
      {placeholder}
      {disabled}
      autocomplete="new-password"
      autocapitalize="off"
      spellcheck="false"
      aria-invalid={error ? 'true' : undefined}
      aria-describedby={description}
      {...rest}
    />
    <button
      class="icon-action"
      type="button"
      {disabled}
      aria-label={visible ? `Ocultar ${label}` : `Mostrar ${label}`}
      aria-pressed={visible}
      onclick={toggle}
    >
      <Icon name={visible ? 'eye-closed' : 'eye'} />
    </button>
  </div>
  {#if error}
    <span class="help error" id="{id}-help" role="alert">{error}</span>
  {:else if help}
    <span class="help" id="{id}-help">{help}</span>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: var(--width-field);
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-primary);
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
  .box:focus-within {
    border-color: var(--color-border-focus);
    box-shadow: inset var(--space-0) var(--space-0) var(--space-0) var(--line-thin) var(--color-border-focus);
  }
  .has-error .box {
    border-color: var(--color-danger-border);
  }
  .disabled {
    background: var(--color-surface-disabled);
  }
  .input {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: var(--space-0);
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .input:focus-visible {
    outline: none;
  }
  .input::placeholder {
    color: var(--color-text-secondary);
  }
  .icon-action {
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
  .icon-action:hover:not(:disabled) {
    background: var(--color-surface-raised);
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
