<script>
  // Filters dialog (FigCraft 5222): width width-dialog, radius window, padding space-24,
  // gap space-20, background color-surface-raised, border color-border-default,
  // shadow-modal. `header` (gap space-12): title in title-modal and close; Group, Type
  // and Environment selects; `footer` (padding-top space-4, gap space-12): clear filters
  // (ghost), spacer, cancel (secondary) and apply (primary).
  //
  // Native <dialog>: focus kept in the dialog and Esc closes (like cancel). The
  // chosen values only count on apply.
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';

  /**
   * @typedef {{ group: string, kind: string, environment: string }} Filters
   * @type {{
   *   open?: boolean, filters: Filters,
   *   groups: Array<{ value: string, label: string }>,
   *   kinds: Array<{ value: string, label: string }>,
   *   environments: Array<{ value: string, label: string }>,
   *   onapply?: (f: Filters) => void, onclose?: () => void
   * }}
   */
  let {
    open = false,
    filters,
    groups,
    kinds,
    environments,
    onapply = undefined,
    onclose = undefined,
  } = $props();

  let dialog = $state();
  let group = $state('');
  let kind = $state('');
  let environment = $state('');

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      group = filters.group;
      kind = filters.kind;
      environment = filters.environment;
      dialog.showModal();
    }
    if (!open && dialog.open) dialog.close();
  });

  function clear() {
    group = '';
    kind = '';
    environment = '';
  }

  function apply() {
    onapply?.({ group, kind, environment });
  }
</script>

<dialog
  bind:this={dialog}
  class="dialog"
  aria-labelledby="filters-title"
  onclose={() => onclose?.()}
>
  <div class="header">
    <h2 id="filters-title">Filtrar credenciais</h2>
    <button class="close" type="button" aria-label="Fechar" onclick={() => onclose?.()}>
      <Icon name="close" />
    </button>
  </div>
  <Select label="Grupo" id="filter-group" bind:value={group} options={groups} />
  <Select label="Tipo" id="filter-kind" bind:value={kind} options={kinds} />
  <Select label="Ambiente" id="filter-environment" bind:value={environment} options={environments} />
  <div class="footer">
    <Button variant="ghost" onclick={clear}>Limpar filtros</Button>
    <span class="spacer"></span>
    <Button variant="secondary" onclick={() => onclose?.()}>Cancelar</Button>
    <Button variant="primary" onclick={apply}>Aplicar</Button>
  </div>
</dialog>

<style>
  .dialog {
    width: var(--width-dialog);
    max-width: calc(100vw - 2 * var(--space-24));
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-modal);
  }
  .dialog[open] {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
  }
  .dialog::backdrop {
    background: var(--color-bg-overlay);
  }
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  h2 {
    flex: 1;
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .close {
    display: inline-flex;
    padding: var(--space-0);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding-top: var(--space-4);
  }
  .spacer {
    flex: 1;
  }
</style>
