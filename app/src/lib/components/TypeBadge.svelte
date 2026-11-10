<script>
  // Type badge (FigCraft set 478): the 9 types. Pill of 20, background
  // color-badge-neutral-bg, padding space-2 space-8 space-2 space-6, gap space-4, icon
  // 12, text-tag.
  import Icon from './Icon.svelte';
  import { entryType } from './types.js';

  /** @type {{ kind: string }} */
  let { kind } = $props();
  const t = $derived(entryType(kind));
</script>

<span class="badge"><Icon name={t.icon} size="xs" />{t.label}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    height: calc(var(--line-height-xs) + 2 * var(--space-2));
    padding: var(--space-2) var(--space-8) var(--space-2) var(--space-6);
    border-radius: var(--radius-pill);
    background: var(--color-badge-neutral-bg);
    color: var(--color-text-secondary);
    font-weight: var(--weight-medium);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    white-space: nowrap;
  }
</style>
