<script>
  // Button (FigCraft set 263): primary, secondary, danger, ghost; height control-default,
  // radius control, padding 0 × space-20, gap space-8, text-body-strong.
  import Icon from './Icon.svelte';

  /**
   * @type {{
   *   variant?: 'primary' | 'secondary' | 'danger' | 'ghost',
   *   type?: 'button' | 'submit',
   *   icon?: string,
   *   disabled?: boolean,
   *   onclick?: (e: MouseEvent) => void,
   *   children?: import('svelte').Snippet,
   *   [k: string]: unknown
   * }}
   */
  let {
    variant = 'primary',
    type = 'button',
    icon = undefined,
    disabled = false,
    onclick = undefined,
    children,
    ...rest
  } = $props();

  const tones = { primary: 'on-color', secondary: 'strong', danger: 'danger', ghost: 'strong' };
</script>

<button class="button {variant}" {type} {disabled} {onclick} {...rest}>
  {#if icon}<Icon name={icon} tone={disabled ? 'disabled' : tones[variant]} />{/if}
  {#if children}<span>{@render children()}</span>{/if}
</button>

<style>
  .button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-8);
    height: var(--control-default);
    padding: var(--space-0) var(--space-20);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid transparent;
    font-family: var(--font-family-text);
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    white-space: nowrap;
    cursor: pointer;
  }
  .primary {
    background: var(--color-primary-default);
    color: var(--color-primary-on);
    box-shadow: var(--shadow-glow-primary);
  }
  .primary:hover:not(:disabled) {
    background: var(--color-primary-hover);
  }
  .secondary {
    background: var(--color-surface-raised);
    border-color: var(--color-border-default);
    color: var(--color-text-primary);
  }
  .secondary:hover:not(:disabled) {
    background: var(--color-surface-hover);
  }
  .danger {
    background: var(--color-danger-translucent);
    border-color: var(--color-danger-border);
    color: var(--color-danger-text);
  }
  .danger:hover:not(:disabled) {
    background: var(--color-danger-translucent-hover);
  }
  .ghost {
    background: transparent;
    color: var(--color-text-primary);
  }
  .ghost:hover:not(:disabled) {
    background: var(--color-surface-default);
  }
  .button:disabled {
    background: var(--color-surface-disabled);
    border-color: transparent;
    color: var(--color-text-disabled);
    box-shadow: none;
    cursor: not-allowed;
  }
</style>
