<script>
  // Field (FigCraft set 331): empty, default, focus, error, disabled. Min width
  // width-field, gap space-8; input height control-default, radius control, padding 0
  // space-8 0 space-16, gap space-8, background color-surface-field, border
  // color-border-subtle (focus: color-border-focus in line-thick). Component
  // properties: left icon (`icon`), show copy (`oncopy`, field action).
  // `pill`: radius-pill, like the search of the credentials screen (4128).
  // `hiddenLabel`: the label stays only for the screen reader (the search shows no
  // label).
  // Only for a value that is NOT a secret (the secret uses `SecretField`).
  import FieldAction from './FieldAction.svelte';
  import Icon from './Icon.svelte';

  /**
   * @type {{
   *   label: string, value?: string, placeholder?: string, help?: string,
   *   error?: string, disabled?: boolean, id: string, type?: string,
   *   icon?: string, pill?: boolean, hiddenLabel?: boolean, readOnly?: boolean,
   *   oncopy?: () => void, copied?: boolean, input?: HTMLInputElement,
   *   [k: string]: unknown
   * }}
   */
  let {
    label,
    value = $bindable(''),
    placeholder = '',
    help = '',
    error = '',
    disabled = false,
    id,
    type = 'text',
    icon = undefined,
    pill = false,
    hiddenLabel = false,
    readOnly = false,
    oncopy = undefined,
    copied = false,
    input = $bindable(),
    ...rest
  } = $props();
</script>

<div class="field" class:has-error={error}>
  <label class="label" class:visually-hidden={hiddenLabel} for={id}>{label}</label>
  <div class="box" class:pill class:disabled>
    {#if icon}<Icon name={icon} tone={disabled ? 'disabled' : undefined} />{/if}
    <input
      bind:this={input}
      {id}
      class="input"
      {type}
      bind:value
      {placeholder}
      {disabled}
      readonly={readOnly}
      aria-invalid={error ? 'true' : undefined}
      aria-describedby={error || help ? `${id}-help` : undefined}
      autocomplete="off"
      spellcheck="false"
      {...rest}
    />
    {#if oncopy}
      <FieldAction label="Copiar" {copied} onclick={oncopy} aria-label="Copiar {label}" />
    {/if}
  </div>
  {#if error}
    <span class="help error" id="{id}-help">{error}</span>
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
  .pill {
    border-radius: var(--radius-pill);
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
    text-overflow: ellipsis;
  }
  .input:focus-visible {
    outline: none;
  }
  .input::placeholder {
    color: var(--color-text-secondary);
  }
  .input:disabled {
    color: var(--color-text-disabled);
  }
  .input:disabled::placeholder {
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
