<script>
  // Slider (FigCraft set 12604): track of height space-6 in a pill, filled in primary up
  // to the thumb and the rest in color-surface-hover; thumb icon-md (20) in
  // color-text-primary with a line-thick border in color-border-focus. Total height
  // icon-md.
  //
  // It is an `<input type="range">` (keyboard and screen reader). The filled position
  // goes in a CSS property through the CSSOM (`style.setProperty`), which the CSP
  // without 'unsafe-inline' allows; no `style` attribute in the HTML.

  /**
   * @type {{
   *   id: string, accessibleLabel: string, value: number, min: number, max: number,
   *   oninput?: (value: number) => void
   * }}
   */
  let { id, accessibleLabel, value, min, max, oninput = undefined } = $props();

  /** @type {HTMLInputElement | undefined} */
  let input = $state();

  $effect(() => {
    const fraction = max > min ? (value - min) / (max - min) : 0;
    input?.style.setProperty('--progress', String(Math.min(1, Math.max(0, fraction))));
  });
</script>

<input
  bind:this={input}
  {id}
  class="slider"
  type="range"
  {min}
  {max}
  step="1"
  {value}
  aria-label={accessibleLabel}
  oninput={(e) => oninput?.(Number(e.currentTarget.value))}
/>

<style>
  .slider {
    /* Filled fraction, from 0 to 1 (comes from the script). The thumb sits over the end
       of the filled part, as in the design. */
    --progress: 0;
    --point: calc(var(--icon-md) / 2 + (100% - var(--icon-md)) * var(--progress));
    appearance: none;
    width: 100%;
    height: var(--icon-md);
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  .slider::-webkit-slider-runnable-track {
    height: var(--space-6);
    border-radius: var(--radius-pill);
    background: linear-gradient(
      to right,
      var(--color-primary-default) 0 var(--point),
      var(--color-surface-hover) var(--point) 100%
    );
  }
  .slider::-webkit-slider-thumb {
    appearance: none;
    box-sizing: border-box;
    width: var(--icon-md);
    height: var(--icon-md);
    margin-top: calc((var(--space-6) - var(--icon-md)) / 2);
    border-radius: var(--radius-pill);
    border: var(--line-thick) solid var(--color-border-focus);
    background: var(--color-text-primary);
  }
  .slider:focus-visible {
    outline: none;
  }
  .slider:focus-visible::-webkit-slider-thumb {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
</style>
