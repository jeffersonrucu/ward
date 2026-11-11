<script>
  // Level card (FigCraft set 18998; variants free 18986, approval 18999, blocked 19011):
  // horizontal frame, background color-surface-default, border color-border-subtle,
  // radius card, padding space-20, gap space-16, items centered; icon 20 px (icon-md)
  // in the icon tone; `texts` (gap space-4): access level label (text-caption,
  // secondary), level (text-body-strong) and explanation (text-small, secondary);
  // secondary Button to change the level, with the lock icon. Width `fill`.
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import { ACCESS_LEVELS, accessLevel } from './levels.js';

  /**
   * @type {{ level: string, disabled?: boolean, onchange?: () => void }}
   */
  let { level, disabled = false, onchange = undefined } = $props();

  const n = $derived(accessLevel(level) ?? ACCESS_LEVELS['approval']);
</script>

<div class="card" data-level={level}>
  <Icon name={n.icon} />
  <div class="texts">
    <span class="label">Nível de acesso</span>
    <span class="name">{n.label}</span>
    <span class="explanation">{n.card}</span>
  </div>
  <Button
    variant="secondary"
    icon="lock"
    {disabled}
    aria-haspopup="dialog"
    onclick={() => onchange?.()}>Mudar nível</Button
  >
</div>

<style>
  .card {
    display: flex;
    align-items: center;
    gap: var(--space-16);
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
    min-width: 0;
  }
  .label {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .explanation {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
</style>
