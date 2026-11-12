<script>
  // Group header (FigCraft 901): height 40, padding space-16 (top) space-4 (sides)
  // space-8 (bottom), gap space-8; an 8×8 `marker` in color-group-<n>-color; group in
  // title-section; count in secondary text-caption.

  /** @type {{ group: string, count: string, color?: 1 | 2 | 3 | 4 | 5, id?: string }} */
  let { group, count, color = 1, id = undefined } = $props();
</script>

<div class="header" {id}>
  <span class="marker color-{color}" aria-hidden="true"></span>
  <span class="group">{group}</span>
  <span class="count">{count}</span>
</div>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    height: var(--control-md);
    padding: var(--space-16) var(--space-4) var(--space-8);
    flex: none;
  }
  .marker {
    flex: none;
    width: var(--space-8);
    height: var(--space-8);
    border-radius: var(--radius-pill);
  }
  .color-1 {
    background: var(--color-group-1-color);
  }
  .color-2 {
    background: var(--color-group-2-color);
  }
  .color-3 {
    background: var(--color-group-3-color);
  }
  .color-4 {
    background: var(--color-group-4-color);
  }
  .color-5 {
    background: var(--color-group-5-color);
  }
  .group {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-semibold);
    font-size: var(--size-md);
    line-height: var(--line-height-md);
  }
  .count {
    flex: none;
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
</style>
