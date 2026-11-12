<script>
  // Group combobox (FigCraft 7691): `Field` with the search icon; the menu has the raised
  // surface background, padding space-4, gap space-2, options of control-md with padding
  // 0 space-12; the found option is selected; the last one creates the typed group, with
  // the plus icon. ARIA combobox pattern with a listbox (arrows, Enter, Esc).

  import Icon from './Icon.svelte';

  /**
   * @type {{ id: string, value?: string, groups: string[], error?: string }}
   */
  let { id, value = $bindable(''), groups, error = '' } = $props();

  let open = $state(false);
  let active = $state(0);

  const term = $derived(value.trim());
  const found = $derived(
    groups.filter((g) => g.toLowerCase().includes(term.toLowerCase())).slice(0, 8),
  );
  const canCreate = $derived(term !== '' && !groups.includes(term));
  const options = $derived([
    ...found.map((g) => ({ value: g, label: g, create: false })),
    ...(canCreate ? [{ value: term, label: `Criar grupo “${term}”`, create: true }] : []),
  ]);

  function choose(o) {
    value = o.value;
    open = false;
  }

  function onKey(e) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      open = true;
      active = Math.min(active + 1, options.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (e.key === 'Enter' && open && options[active]) {
      e.preventDefault();
      choose(options[active]);
    } else if (e.key === 'Escape' && open) {
      e.preventDefault();
      open = false;
    }
  }
</script>

<div class="field" class:has-error={error}>
  <label class="label" for={id}>Grupo</label>
  <div class="box">
    <Icon name="search" />
    <input
      {id}
      class="input"
      role="combobox"
      aria-expanded={open && options.length > 0}
      aria-controls="{id}-options"
      aria-autocomplete="list"
      aria-activedescendant={open && options[active] ? `${id}-option-${active}` : undefined}
      aria-invalid={error ? 'true' : undefined}
      aria-describedby={error ? `${id}-help` : undefined}
      bind:value
      placeholder="Ex.: acme"
      autocomplete="off"
      spellcheck="false"
      onfocus={() => (open = true)}
      oninput={() => {
        open = true;
        active = 0;
      }}
      onblur={() => (open = false)}
      onkeydown={onKey}
    />
  </div>
  {#if open && options.length > 0}
    <ul class="menu" id="{id}-options" role="listbox" aria-label="Grupos">
      {#each options as o, i (o.label)}
        <li
          id="{id}-option-{i}"
          class="option"
          class:active={i === active}
          class:selected={!o.create && o.value === term}
          role="option"
          aria-selected={i === active}
          onmousedown={(e) => {
            e.preventDefault();
            choose(o);
          }}
        >
          {#if o.create}<Icon name="plus" />{/if}
          <span>{o.label}</span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if error}<span class="help error" id="{id}-help">{error}</span>{/if}
</div>

<style>
  .field {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: 0;
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .box {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    height: var(--control-default);
    padding: var(--space-0) var(--space-8) var(--space-0) var(--space-16);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-field);
  }
  .box:focus-within {
    border-color: var(--color-border-focus);
  }
  .has-error .box {
    border-color: var(--color-danger-border);
  }
  .input {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: var(--space-0);
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .input:focus-visible {
    outline: none;
  }
  .input::placeholder {
    color: var(--color-text-secondary);
  }
  .menu {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: var(--space-4) 0 0;
    padding: var(--space-4);
    list-style: none;
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-floating);
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    height: var(--control-md);
    padding: var(--space-0) var(--space-12);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: var(--size-base);
  }
  .option.active {
    background: var(--color-surface-hover);
  }
  .option.selected {
    background: var(--color-surface-selected);
  }
  .help {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .error {
    color: var(--color-danger-text);
  }
</style>
