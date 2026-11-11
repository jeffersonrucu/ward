<script>
  // Side navigation (FigCraft set 782): credentials `783`, password generator `3037`,
  // settings `3077`. Width width-sidebar, radius panel, color-bg-subtle, subtle border,
  // padding space-24 × space-16, gap space-32. Item: height control-md, radius control,
  // padding space-8 × space-12, gap space-12; active with raised surface and default
  // border. Footer: card color-surface-default with the vault status and the lock now
  // button.
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  /**
   * @type {{
   *   active: string,
   *   onnavigate?: (screen: string) => void,
   *   onlock?: () => void,
   * }}
   */
  let { active, onnavigate = undefined, onlock = undefined } = $props();

  const ITEMS = [
    { screen: 'credentials', label: 'Credenciais', icon: 'key' },
    { screen: 'generator', label: 'Gerador de senha', icon: 'generator' },
    { screen: 'settings', label: 'Configurações', icon: 'gear' },
  ];
</script>

<nav class="side" aria-label="Navegação principal">
  <div class="brand"><Logo /></div>
  <ul class="items">
    {#each ITEMS as item (item.screen)}
      <li>
        <button
          type="button"
          class="item"
          class:active={item.screen === active}
          aria-current={item.screen === active ? 'page' : undefined}
          onclick={() => onnavigate?.(item.screen)}
        >
          <Icon name={item.icon} tone={item.screen === active ? 'brand' : undefined} />
          <span>{item.label}</span>
        </button>
      </li>
    {/each}
  </ul>
  <div class="spacer"></div>
  <div class="footer">
    <div class="status">
      <span class="indicator" aria-hidden="true"></span>
      <span>Cofre destravado</span>
    </div>
    <div class="lock">
      <Button variant="secondary" icon="lock" onclick={() => onlock?.()}>Travar agora</Button>
    </div>
  </div>
</nav>

<style>
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    flex: none;
    width: var(--width-sidebar);
    padding: var(--space-24) var(--space-16);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .brand {
    display: flex;
    padding: var(--space-0) var(--space-8);
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    width: 100%;
    height: var(--control-md);
    padding: var(--space-8) var(--space-12);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid transparent;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    text-align: left;
    cursor: pointer;
  }
  .item:hover:not(.active) {
    background: var(--color-surface-default);
  }
  .item.active {
    border-color: var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    font-weight: var(--weight-medium);
  }
  .spacer {
    flex: 1;
  }
  .footer {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    padding: var(--space-16);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .status {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .indicator {
    width: var(--space-8);
    height: var(--space-8);
    border-radius: var(--radius-pill);
    background: var(--color-success-default);
  }
  /* One-column grid: the button stretches to the width of the card, as in the design. */
  .lock {
    display: grid;
  }
</style>
