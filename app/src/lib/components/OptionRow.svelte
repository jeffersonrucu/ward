<script>
  // Option row (FigCraft set 10379), switch control variant (10368): card
  // color-surface-default, subtle border, radius item, padding space-20, gap space-16;
  // title in text-body-strong, description in secondary text-small (gap space-4).
  // Switch (FigCraft set 6838): 36×20 (width-switch × icon-md), padding space-2, pill;
  // on is primary with a white knob on the right, off is color-surface-hover with a
  // secondary knob on the left.
  //
  // It is a real `<input type="checkbox" role="switch">` (keyboard and screen reader),
  // with the drawing of the switch on top.
  //
  // Phase 38: button control (10380) and select control (10825) variants: with the
  // `control` snippet, the row is a `<div>` and the control (a secondary `Button` or a
  // compact `Select`) sits on the right, in place of the switch; same sizes.

  /**
   * @type {{
   *   title: string, id: string, on?: boolean, disabled?: boolean,
   *   onchange?: (on: boolean) => void, description?: import('svelte').Snippet,
   *   control?: import('svelte').Snippet
   * }}
   */
  let {
    title,
    id,
    on = false,
    disabled = false,
    onchange = undefined,
    description,
    control,
  } = $props();
</script>

{#if control}
  <div class="row with-control" {id}>
    <span class="texts">
      <span class="title">{title}</span>
      {#if description}<span class="description">{@render description()}</span>{/if}
    </span>
    <span class="control">{@render control()}</span>
  </div>
{:else}
<label class="row" for={id}>
  <span class="texts">
    <span class="title">{title}</span>
    {#if description}<span class="description">{@render description()}</span>{/if}
  </span>
  <input
    {id}
    class="visually-hidden"
    type="checkbox"
    role="switch"
    checked={on}
    {disabled}
    onchange={(e) => {
      // Controlled (Phase 48): the click only asks for the change; the state goes back
      // to `on` until whoever uses the row decides (turning on the external backup opens
      // the choice first).
      const requested = e.currentTarget.checked;
      e.currentTarget.checked = on;
      onchange?.(requested);
    }}
  />
  <span class="switch" class:on aria-hidden="true">
    <span class="knob"></span>
  </span>
</label>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
    cursor: pointer;
  }
  .with-control {
    cursor: default;
  }
  .control {
    display: flex;
    flex: none;
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .description {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .switch {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    flex: none;
    width: var(--width-switch);
    height: var(--icon-md);
    padding: var(--space-2);
    border-radius: var(--radius-pill);
    background: var(--color-surface-hover);
  }
  .knob {
    width: var(--icon-sm);
    height: var(--icon-sm);
    border-radius: var(--radius-pill);
    background: var(--color-text-secondary);
  }
  .switch.on {
    justify-content: flex-end;
    background: var(--color-primary-default);
  }
  .on .knob {
    background: var(--color-brand-on-color);
  }
  .row:has(input:disabled) {
    cursor: not-allowed;
  }
  input:focus-visible + .switch {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
</style>
