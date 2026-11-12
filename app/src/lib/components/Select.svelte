<script>
  // Select (FigCraft set 421): default, focus, open, disabled. Uses the native <select>
  // (keyboard and screen reader for free); box like the one of `Field`, min width
  // width-filter-select, chevron-down on the right. `compact` (Phase 38, the Select
  // 10829 of the option row 10825): width width-compact-select and the label only for
  // the screen reader (the title of the row already says what it is).
  import Icon from './Icon.svelte';

  /**
   * @type {{ label: string, id: string, value?: string,
   *   options: Array<{ value: string, label: string }>, disabled?: boolean,
   *   compact?: boolean, onchange?: (value: string) => void }}
   */
  let {
    label,
    id,
    value = $bindable(''),
    options,
    disabled = false,
    compact = false,
    onchange = undefined,
  } = $props();
</script>

<div class="select" class:compact>
  <label class="label" class:visually-hidden={compact} for={id}>{label}</label>
  <div class="box">
    <select {id} bind:value {disabled} onchange={() => onchange?.(value)}>
      {#each options as o (o.value)}
        <option value={o.value}>{o.label}</option>
      {/each}
    </select>
    <span class="chevron"><Icon name="chevron-down" size="sm" /></span>
  </div>
</div>

<style>
  .select {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: var(--width-filter-select);
  }
  .compact {
    width: var(--width-compact-select);
    min-width: var(--width-compact-select);
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .box {
    position: relative;
  }
  select {
    appearance: none;
    width: 100%;
    height: var(--control-default);
    padding: var(--space-0) var(--space-40) var(--space-0) var(--space-16);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-field);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    cursor: pointer;
  }
  select:focus-visible {
    outline: none;
    border-color: var(--color-border-focus);
  }
  select:disabled {
    background: var(--color-surface-disabled);
    color: var(--color-text-disabled);
    cursor: not-allowed;
  }
  option {
    background: var(--color-surface-default);
  }
  .chevron {
    position: absolute;
    right: var(--space-16);
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    pointer-events: none;
  }
</style>
