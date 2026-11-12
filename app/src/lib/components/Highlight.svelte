<script>
  // Highlight (FigCraft 18673, Phase 10c): note card. Padding space-16 × space-20, gap
  // space-12, background color-surface-default, border color-border-subtle, radius
  // card, min width width-field; optional icon and text in secondary text-small.
  // `kind` (Phase 30): warning (18684: background color-warning-subtle, border
  // color-warning-border-soft, warning icon) and info (18679: background
  // color-info-subtle, border color-info-border-soft, brand icon), both with the text in
  // color-text-primary. Phase 36: danger (18689: background color-danger-translucent,
  // border color-danger-border-soft, danger icon) and prod (18694: background
  // color-environment-prod-tint, border color-environment-prod-border and the large prod
  // environment badge in place of the icon), also with the text in color-text-primary.
  import EnvironmentBadge from './EnvironmentBadge.svelte';
  import Icon from './Icon.svelte';

  /**
   * @type {{ icon?: string, kind?: 'neutral' | 'warning' | 'info' | 'danger' | 'prod',
   *   children: import('svelte').Snippet }}
   */
  let { icon = undefined, kind = 'neutral', children } = $props();
  const tones = { neutral: undefined, warning: 'warning', info: 'brand', danger: 'danger' };
</script>

<div class="highlight {kind}">
  {#if kind === 'prod'}
    <EnvironmentBadge environment="prod" large />
  {:else if icon}
    <Icon name={icon} tone={tones[kind]} />
  {/if}
  <p class="text">{@render children()}</p>
</div>

<style>
  .highlight {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    min-width: var(--width-field);
    padding: var(--space-16) var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .text {
    flex: 1;
    margin: var(--space-0);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .warning {
    border-color: var(--color-warning-border-soft);
    background: var(--color-warning-subtle);
  }
  .info {
    border-color: var(--color-info-border-soft);
    background: var(--color-info-subtle);
  }
  .danger {
    border-color: var(--color-danger-border-soft);
    background: var(--color-danger-translucent);
  }
  .prod {
    border-color: var(--color-environment-prod-border);
    background: var(--color-environment-prod-tint);
  }
  .warning .text,
  .info .text,
  .danger .text,
  .prod .text {
    color: var(--color-text-primary);
  }
</style>
