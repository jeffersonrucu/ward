<script>
  // Switch (FigCraft set 6838): 36×20 (width-switch × icon-md), padding space-2, pill;
  // on is primary with a white knob on the right, off is color-surface-hover with a
  // secondary knob on the left. Compact version (label beside it, gap space-8) used in
  // the extra field (6878) and in the options of the generator popover (11671).
  // It is a real `<input type="checkbox" role="switch">`.

  /**
   * @type {{ id: string, label: string, on: boolean, disabled?: boolean,
   *   onchange?: (on: boolean) => void }}
   */
  let { id, label, on, disabled = false, onchange = undefined } = $props();
</script>

<label class="option" for={id}>
  <input
    {id}
    class="visually-hidden"
    type="checkbox"
    role="switch"
    checked={on}
    {disabled}
    onchange={(e) => onchange?.(e.currentTarget.checked)}
  />
  <span class="switch" class:on aria-hidden="true"><span class="knob"></span></span>
  <span class="label">{label}</span>
</label>

<style>
  .option {
    display: inline-flex;
    align-items: center;
    gap: var(--space-8);
    cursor: pointer;
  }
  .option:has(input:disabled) {
    cursor: not-allowed;
    color: var(--color-text-disabled);
  }
  .label {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
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
  input:focus-visible + .switch {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
</style>
