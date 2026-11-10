<script>
  // Horizontal logo (FigCraft set 3300): symbol 40×40 (control-md) + "ward" in
  // title-highlight, gap space-8. The symbol is the SVG exported from the logo symbol
  // component (same drawing as the one in docs/design/logo/), served as a file
  // (img-src 'self').
  //
  // `print`: the name in the ink of the printed sheet (color-print-text), like the
  // override of instance `16781` in the printed emergency kit (16773).
  import symbol from './ward-symbol.svg';

  /** @type {{ print?: boolean }} */
  let { print = false } = $props();
</script>

<span class="logo" class:print>
  <img class="symbol" src={symbol} alt="" />
  <span class="word">ward</span>
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: var(--space-8);
  }
  .symbol {
    width: var(--control-md);
    height: var(--control-md);
  }
  .word {
    display: inline-flex;
    align-items: center;
    height: var(--control-md);
    padding-bottom: var(--space-4);
    font-weight: var(--weight-semibold);
    font-size: var(--size-3xl);
    line-height: var(--line-height-3xl);
    letter-spacing: var(--letter-spacing-title);
    color: var(--color-text-primary);
  }
  .print .word {
    color: var(--color-print-text);
  }
</style>
