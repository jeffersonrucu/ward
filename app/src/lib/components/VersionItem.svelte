<script>
  // Version item (FigCraft set 11264; default 11243, selected 11265): `fill`, padding
  // space-16, gap space-12, items centered, radius item, line-thin border. Default:
  // background color-surface-default, border color-border-subtle, 20 px radio (icon-md,
  // radius-full) with a line-thick border in color-border-strong. Selected: background
  // color-surface-selected, border and radio in color-border-focus, 8 px dot (space-8)
  // in color-primary-default. `texts` gap space-2: date in text-body-strong and detail
  // in secondary text-small; "when" in secondary text-caption on the right.
  //
  // In the list it is a real `<input type="radio">` (arrows change the version, the
  // screen reader reads the group), with the drawing on top; with `staticView` (the
  // summary of the confirm step) it is only the drawing of the selected one.
  //
  // Phase 49a: `alert` (the write is older than the date of the name says) paints the
  // detail in color-warning-text; the text already says why (it does not depend on the
  // color alone).

  /**
   * @type {{ id: string, date: string, detail: string, when: string,
   *   name?: string, value?: string, checked?: boolean, staticView?: boolean,
   *   alert?: boolean, onchoose?: (v: string) => void }}
   */
  let {
    id,
    date,
    detail,
    when,
    name = 'versao',
    value = '',
    checked = false,
    staticView = false,
    alert = false,
    onchoose = undefined,
  } = $props();
</script>

{#snippet drawing(selected)}
  <span class="radio" class:selected aria-hidden="true">
    {#if selected}<span class="dot"></span>{/if}
  </span>
  <span class="texts">
    <span class="date">{date}</span>
    <span class="detail" class:alert>{detail}</span>
  </span>
  <span class="when">{when}</span>
{/snippet}

{#if staticView}
  <div class="item selected" {id}>{@render drawing(true)}</div>
{:else}
  <label class="item" class:selected={checked} for={id}>
    <input
      {id}
      class="visually-hidden"
      type="radio"
      {name}
      {value}
      {checked}
      onchange={() => onchoose?.(value)}
    />
    {@render drawing(checked)}
  </label>
{/if}

<style>
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
    color: var(--color-text-primary);
  }
  label.item {
    cursor: pointer;
  }
  .item.selected {
    border-color: var(--color-border-focus);
    background: var(--color-surface-selected);
  }
  /* Inside: the list scrolls and would cut an outline drawn outside. */
  .item:has(:focus-visible) {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: calc(-1 * var(--line-thick));
  }
  .radio {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    box-sizing: border-box;
    width: var(--icon-md);
    height: var(--icon-md);
    border-radius: var(--radius-full);
    border: var(--line-thick) solid var(--color-border-strong);
  }
  .radio.selected {
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
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
  }
  .date {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .detail {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .detail.alert {
    color: var(--color-warning-text);
  }
  .when {
    flex: none;
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
</style>
