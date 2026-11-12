<script>
  // List item (FigCraft set 553): wide/compact × default/hover/selected. Radius item,
  // padding space-16; avatar 40×40 (control-md) with the icon of the type.
  // - wide (gap space-16): fixed columns of type, environment and level (D23);
  // - compact (4002/4028/4045, Phase 4f; gap space-12): `line 1` = name + environment
  //   badge, `line 2` = type badge + detail, gap space-8, texts gap space-4; no level.
  //   Selected with a color-border-focus border inside (FigCraft `inside` stroke: does
  //   not change the height of 76).
  import EnvironmentBadge from './EnvironmentBadge.svelte';
  import TypeBadge from './TypeBadge.svelte';
  import { entryType } from './types.js';
  import Icon from './Icon.svelte';

  /**
   * @type {{ name: string, detail?: string, kind: string, environment: 'dev' | 'stage' | 'prod',
   *   level?: string, selected?: boolean, compact?: boolean, onclick?: () => void,
   *   [k: string]: unknown }}
   */
  let {
    name,
    detail = '',
    kind,
    environment,
    level = '',
    selected = false,
    compact = false,
    onclick = undefined,
    ...rest
  } = $props();
</script>

<button
  class="item"
  class:selected
  class:compact
  type="button"
  aria-current={selected || undefined}
  {onclick}
  {...rest}
>
  <span class="avatar"><Icon name={entryType(kind).icon} /></span>
  {#if compact}
    <span class="texts">
      <span class="line">
        <span class="name">{name}</span>
        <EnvironmentBadge {environment} />
      </span>
      <span class="line">
        <TypeBadge {kind} />
        {#if detail}<span class="detail">{detail}</span>{/if}
      </span>
    </span>
  {:else}
    <span class="texts">
      <span class="name">{name}</span>
      {#if detail}<span class="detail">{detail}</span>{/if}
    </span>
    <span class="column-type"><TypeBadge {kind} /></span>
    <span class="column-environment"><EnvironmentBadge {environment} /></span>
    {#if level}<span class="column-level">{level}</span>{/if}
  {/if}
</button>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    width: 100%;
    padding: var(--space-16);
    border-radius: var(--radius-item);
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .compact {
    gap: var(--space-12);
  }
  .item:hover {
    background: var(--color-surface-raised);
  }
  .selected,
  .selected:hover {
    background: var(--color-surface-selected);
    box-shadow: inset var(--space-0) var(--space-0) var(--space-0) var(--line-thin)
      var(--color-border-focus);
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--control-md);
    height: var(--control-md);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .texts {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .compact .texts {
    gap: var(--space-4);
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }
  .name,
  .detail {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .detail {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .column-type,
  .column-environment,
  .column-level {
    display: flex;
    align-items: center;
  }
  .column-type {
    flex: none;
    width: var(--width-column-type);
  }
  .column-environment {
    flex: none;
    width: var(--width-column-environment);
  }
  .column-level {
    flex: none;
    width: var(--width-column-level);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
</style>
