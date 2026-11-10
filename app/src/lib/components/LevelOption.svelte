<script>
  // Level option (FigCraft set 909; level × selected: blocked 910/920, approval
  // 931/940, free 950/958): card in `hug`, padding space-20, gap space-16, radius card,
  // items on top. Not selected: background color-surface-field, border
  // color-border-default, a 20 px picker (icon-md, radius-full) with
  // color-border-strong. Selected: background color-primary-subtle, border
  // color-border-focus, picker with color-border-focus and the 8 px dot (space-8) in
  // color-primary-default. `texts` gap space-6: title (gap space-8: 20 px icon and name
  // in text-body-strong) and explanation in secondary text-small.
  //
  // It is a real `<input type="radio">` (arrows change the option, the screen reader
  // reads the group), with the drawing on top.
  import Icon from './Icon.svelte';
  import { accessLevel } from './levels.js';

  /**
   * @type {{ level: string, name: string, checked: boolean, onchoose?: (n: string) => void }}
   */
  let { level, name, checked, onchoose = undefined } = $props();

  const n = $derived(accessLevel(level));
  const id = $derived(`level-option-${level}`);
</script>

{#if n}
  <label class="option" class:checked for={id}>
    <input
      {id}
      class="visually-hidden"
      type="radio"
      {name}
      value={level}
      {checked}
      aria-describedby="{id}-explanation"
      onchange={() => onchoose?.(level)}
    />
    <span class="picker" aria-hidden="true">
      {#if checked}<span class="dot"></span>{/if}
    </span>
    <span class="texts">
      <span class="title">
        <Icon name={n.icon} />
        <span class="name">{n.label}</span>
      </span>
      <span class="explanation" id="{id}-explanation">{n.option}</span>
    </span>
  </label>
{/if}

<style>
  .option {
    display: flex;
    align-items: flex-start;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-field);
    cursor: pointer;
  }
  .option.checked {
    border-color: var(--color-border-focus);
    background: var(--color-primary-subtle);
  }
  .option:has(:focus-visible) {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
  .picker {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--icon-md);
    height: var(--icon-md);
    border-radius: var(--radius-full);
    border: var(--line-thin) solid var(--color-border-strong);
  }
  .checked .picker {
    border-color: var(--color-border-focus);
  }
  .dot {
    width: var(--space-8);
    height: var(--space-8);
    border-radius: var(--radius-full);
    background: var(--color-primary-default);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    flex: 1;
    min-width: 0;
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--space-8);
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
