<script>
  // Type card (FigCraft 17607): padding space-20 space-16, gap space-12, radius card;
  // avatar control-md with the icon of the type; name in text-body-strong; description
  // in secondary text-small. Hover: raised surface and default border; selected:
  // selected surface and focus border.
  import Icon from './Icon.svelte';
  import { entryType } from './types.js';

  /**
   * @type {{ kind: string, name: string, description: string, selected?: boolean,
   *   disabled?: boolean, onclick?: () => void }}
   */
  let {
    kind,
    name,
    description,
    selected = false,
    disabled = false,
    onclick = undefined,
  } = $props();
</script>

<button
  type="button"
  class="card"
  class:selected
  role="radio"
  aria-checked={selected}
  {disabled}
  data-type={kind}
  onclick={() => onclick?.()}
>
  <span class="avatar">
    <Icon name={entryType(kind).icon} tone={selected ? 'brand' : undefined} />
  </span>
  <span class="name">{name}</span>
  <span class="description">{description}</span>
</button>

<style>
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-12);
    padding: var(--space-20) var(--space-16);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
  }
  .card:hover:not(:disabled) {
    background: var(--color-surface-raised);
    border-color: var(--color-border-default);
  }
  .card.selected {
    background: var(--color-surface-selected);
    border-color: var(--color-border-focus);
  }
  .card:disabled {
    cursor: not-allowed;
    color: var(--color-text-disabled);
  }
  .card:focus-visible {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-md);
    height: var(--control-md);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .description {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .card:disabled .description {
    color: var(--color-text-disabled);
  }
</style>
