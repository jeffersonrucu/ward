<script>
  // Extra field (FigCraft set 6878; not secret 6861, secret 6879): card
  // color-surface-default, subtle border, radius item, padding space-16, gap space-12.
  // Name line (text-body-strong) with the secret switch and the Remove field action
  // (close icon); value line (or mask, when secret) and the `WARD_` variable in secondary
  // text-caption.
  //
  // In the form the name and the value are editable. A saved secret extra (`stored`)
  // shows only the mask: the value stays in the vault and goes as "keep" until the user
  // changes it (S11).
  import FieldAction from './FieldAction.svelte';
  import Switch from './Switch.svelte';

  /**
   * @type {{
   *   id: string, name: string, value: string, secret: boolean, stored?: boolean,
   *   error?: string, onremove?: () => void, onchange?: () => void
   * }}
   */
  let {
    id,
    name = $bindable(''),
    value = $bindable(''),
    secret = $bindable(false),
    stored = false,
    error = '',
    onremove = undefined,
    onchange = undefined,
  } = $props();

  const variable = $derived(`WARD_${name.trim() || '<NOME>'}`);
</script>

<div class="extra" class:has-error={error} data-extra={id}>
  <div class="texts">
    <input
      id="{id}-name"
      class="name"
      bind:value={name}
      placeholder="NOME"
      aria-label="Nome do campo extra"
      autocomplete="off"
      spellcheck="false"
      aria-invalid={error ? 'true' : undefined}
    />
    <div class="line">
      {#if stored}
        <button
          type="button"
          class="value mask"
          aria-label="Trocar o valor de {name} (valor oculto)"
          onclick={() => onchange?.()}>••••••••</button
        >
      {:else}
        <input
          id="{id}-value"
          class="value"
          type={secret ? 'password' : 'text'}
          bind:value
          placeholder="Valor"
          aria-label="Valor de {name || 'campo extra'}"
          autocomplete={secret ? 'new-password' : 'off'}
          spellcheck="false"
        />
      {/if}
      <span class="variable">{variable}</span>
    </div>
    {#if error}<span class="error">{error}</span>{/if}
  </div>
  <Switch id="{id}-secret" label="segredo" on={secret} onchange={(v) => (secret = v)} />
  <FieldAction
    label="Remover"
    icon="close"
    onclick={() => onremove?.()}
    aria-label="Remover o campo extra {name}"
  />
</div>

<style>
  .extra {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .extra.has-error {
    border-color: var(--color-danger-border);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-8);
  }
  input,
  .mask {
    flex: 1;
    min-width: 0;
    padding: var(--space-0);
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
  }
  input:focus-visible,
  .mask:focus-visible {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
    border-radius: var(--radius-sm);
  }
  input::placeholder {
    color: var(--color-text-secondary);
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .value {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .mask {
    cursor: text;
  }
  .variable,
  .error {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .error {
    color: var(--color-danger-text);
  }
</style>
