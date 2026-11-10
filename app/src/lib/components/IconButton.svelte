<script>
  // Icon button (FigCraft 18832): 32×32 (control-sm), radius control (radius-md), icon
  // 20 (icon-md) in the center, transparent background; hover with
  // color-surface-default. The label goes only to the screen reader and the tooltip
  // (`title`).
  import Icon from './Icon.svelte';

  /** @type {{ icon: string, label: string, onclick?: (e: MouseEvent) => void }} */
  let { icon, label, onclick = undefined } = $props();
</script>

<button class="icon-button" type="button" aria-label={label} title={label} {onclick}>
  <Icon name={icon} />
</button>

<style>
  .icon-button {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    padding: var(--space-0);
    border: none;
    border-radius: var(--radius-control);
    background: transparent;
    cursor: pointer;
  }
  .icon-button:hover {
    background: var(--color-surface-default);
  }
</style>
