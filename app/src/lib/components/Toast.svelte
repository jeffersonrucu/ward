<script>
  // Toast (FigCraft set 525): success, error, warning. Width width-toast, radius card,
  // padding space-16 space-12 space-16 space-16, text-body, shadow-floating.
  import Icon from './Icon.svelte';

  /** @type {{ kind?: 'success' | 'error' | 'warning', message: string }} */
  let { kind = 'success', message } = $props();
  const icons = { success: 'success', error: 'error', warning: 'alert' };
  const tones = { success: 'success', error: 'danger', warning: 'warning' };
</script>

<div class="toast" role={kind === 'error' ? 'alert' : 'status'}>
  <Icon name={icons[kind]} tone={tones[kind]} />
  <span>{message}</span>
</div>

<style>
  .toast {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    width: var(--width-toast);
    padding: var(--space-16) var(--space-12) var(--space-16) var(--space-16);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-floating);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
</style>
