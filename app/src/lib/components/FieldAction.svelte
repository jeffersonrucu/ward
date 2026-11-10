<script>
  // Field action (FigCraft set 2882): default and copied; height control-sm, radius sm,
  // padding 0 space-12 0 space-8, gap space-6, icon 16, text-label.
  import Icon from './Icon.svelte';

  /** @type {{ label: string, icon?: string, copied?: boolean, onclick?: () => void, disabled?: boolean, [k: string]: unknown }} */
  let {
    label,
    icon = 'copy',
    copied = false,
    onclick = undefined,
    disabled = false,
    ...rest
  } = $props();
</script>

<button class="action" class:copied type="button" {disabled} {onclick} {...rest}>
  <Icon name={copied ? 'check' : icon} size="sm" tone={copied ? 'success' : undefined} />
  <span>{copied ? 'Copiado' : label}</span>
</button>

<style>
  .action {
    display: inline-flex;
    align-items: center;
    gap: var(--space-6);
    height: var(--control-sm);
    padding: var(--space-0) var(--space-12) var(--space-0) var(--space-8);
    border-radius: var(--radius-sm);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    cursor: pointer;
    white-space: nowrap;
  }
  .action:hover:not(:disabled) {
    background: var(--color-surface-hover);
  }
  .copied {
    background: var(--color-success-subtle);
    border-color: var(--color-success-text);
    color: var(--color-success-text);
  }
  .action:disabled {
    color: var(--color-text-disabled);
    cursor: not-allowed;
  }
</style>
