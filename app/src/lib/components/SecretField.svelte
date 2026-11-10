<script>
  // Secret field (FigCraft set 375): hidden, revealed, copied and empty (18609). Same box
  // as `Field`; actions (gap space-4): reveal 32×32, generate (optional, "show
  // generate"), copy field action ("show copy"). Empty: placeholder in secondary text,
  // no reveal and no copy, fixed help.
  //
  // S11/D4: this component never fetches nor keeps the value of the vault. Hidden, it
  // only draws the mask. The value only comes in when the owner screen passes `revealed`
  // (and takes it out again in 30 s, on hide or on leaving). Copy calls `oncopy`,
  // which asks the Rust side: the value does not pass through here.
  //
  // `editable` (form, Phase 33): in the empty state the box is a password input where
  // the user types (`bind:typed`); the eye toggles visible typing only in the field
  // itself. With a saved value, the change value button (`onchange`) turns the box into typing.
  //
  // Phase 39g (S2, S12; 2nd audit of M5): the revealed value cannot reach the clipboard
  // other than through the copy button (which sets a deadline and the `secret` hint). In
  // WebKitGTK, selecting text puts it in PRIMARY at once, and Ctrl+C in CLIPBOARD, with
  // no deadline. `user-select: none` alone is not enough: Ctrl+A, or a selection that
  // starts outside and crosses the field, still takes the text to PRIMARY. So the value
  // is not a text node: it goes in the `data-value` attribute and is drawn by `::before`
  // (`content: attr(...)`), which no selection, copy or drag picks up. `copy`, `cut` and
  // `selectstart` in the field are also cancelled (in the visible typing field, `copy`,
  // `cut` and `dragstart`). Checked in the real WebKitGTK by
  // `dev/tests/revealed_selection.py` (`make test-revealed-selection`).
  import FieldAction from './FieldAction.svelte';
  import Icon from './Icon.svelte';

  /**
   * @type {{
   *   label: string, id: string, revealed?: string | null, copied?: boolean,
   *   help?: string, empty?: boolean, showGenerate?: boolean, mask?: string,
   *   showReveal?: boolean, showCopy?: boolean, editable?: boolean,
   *   typed?: string, placeholder?: string, error?: string,
   *   onreveal?: () => void, onhide?: () => void, oncopy?: () => void,
   *   ongenerate?: () => void, onchange?: () => void
   * }}
   */
  let {
    label,
    id,
    revealed = null,
    copied = false,
    help = 'Revelar e copiar não pedem PIN.',
    empty = false,
    showGenerate = false,
    // The mask does not come from the value: whoever knows the length (the generator)
    // may pass one dot per character; the default has 12.
    mask = '••••••••••••',
    showReveal = true,
    showCopy = true,
    editable = false,
    typed = $bindable(''),
    placeholder = 'Nenhum valor salvo',
    error = '',
    onreveal = undefined,
    onhide = undefined,
    oncopy = undefined,
    ongenerate = undefined,
    onchange = undefined,
  } = $props();

  const open = $derived(revealed !== null);
  // Visible typing only in the field (the value is already in the box, typed by the user).
  let visible = $state(false);
  const description = $derived(error || help ? `${id}-help` : undefined);

  /** @param {Event} e */
  function noCopy(e) {
    e.preventDefault();
  }
</script>

<div class="field" class:has-error={error}>
  {#if editable && empty}
    <label class="label" for={id}>{label}</label>
  {:else}
    <span class="label" id="{id}-label">{label}</span>
  {/if}
  <div
    class="box"
    role={editable && empty ? undefined : 'group'}
    aria-labelledby={editable && empty ? undefined : `${id}-label`}
  >
    {#if editable && empty}
      <input
        {id}
        class="value typeable"
        class:mono={visible && typed}
        type={visible ? 'text' : 'password'}
        bind:value={typed}
        {placeholder}
        autocomplete="new-password"
        spellcheck="false"
        aria-invalid={error ? 'true' : undefined}
        aria-describedby={description}
        oncopy={visible ? noCopy : undefined}
        oncut={visible ? noCopy : undefined}
        ondragstart={noCopy}
      />
    {:else if empty}
      <span class="value placeholder">{placeholder}</span>
    {:else if open}
      <span
        class="value revealed"
        data-value={revealed}
        oncopy={noCopy}
        oncut={noCopy}
        onselectstart={noCopy}
      ></span>
    {:else if onchange}
      <button
        type="button"
        class="value mask-button"
        aria-label="Trocar o valor de {label} (valor oculto)"
        title="Trocar valor"
        onclick={() => onchange?.()}>{mask}</button
      >
    {:else}
      <span class="value" aria-label="valor oculto">{mask}</span>
    {/if}
    <div class="actions">
      {#if editable && empty && typed}
        <button
          class="icon-action"
          type="button"
          aria-label={visible ? 'Ocultar o que foi digitado' : 'Mostrar o que foi digitado'}
          aria-pressed={visible}
          onclick={() => (visible = !visible)}
        >
          <Icon name={visible ? 'eye-closed' : 'eye'} />
        </button>
      {/if}
      {#if !empty && showReveal}
        <button
          class="icon-action"
          type="button"
          aria-label={open ? `Ocultar ${label}` : `Revelar ${label}`}
          aria-pressed={open}
          onclick={() => (open ? onhide?.() : onreveal?.())}
        >
          <Icon name={open ? 'eye-closed' : 'eye'} />
        </button>
      {/if}
      {#if showGenerate}
        <button class="icon-action" type="button" aria-label="Gerar senha" onclick={() => ongenerate?.()}>
          <Icon name="generator" />
        </button>
      {/if}
      {#if !empty && showCopy}
        <FieldAction
          label="Copiar"
          {copied}
          onclick={() => oncopy?.()}
          aria-label="Copiar {label}"
        />
      {/if}
    </div>
  </div>
  {#if error}
    <span class="help error" id="{id}-help">{error}</span>
  {:else if help}
    <span class="help" id="{id}-help">{help}</span>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: var(--width-field);
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
  .value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .typeable {
    height: 100%;
    padding: var(--space-0);
    border: none;
    background: transparent;
    color: var(--color-text-primary);
  }
  .typeable:focus-visible {
    outline: none;
  }
  .typeable::placeholder {
    color: var(--color-text-secondary);
  }
  .mask-button {
    padding: var(--space-0);
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
    cursor: text;
  }
  .mask-button:focus-visible {
    outline: var(--line-thick) solid var(--color-border-focus);
    outline-offset: var(--line-thick);
  }
  /* Phase 39g: the revealed value only exists in `::before`, out of any selection. */
  .revealed {
    user-select: none;
    -webkit-user-select: none;
  }
  .revealed::before {
    content: attr(data-value);
  }
  .revealed,
  .mono {
    font-family: var(--font-family-mono);
    /* The value shows character by character: no ligatures (`<=` or `->` would become a
       single glyph and the password read would not be the one copied). */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
  }
  .placeholder {
    color: var(--color-text-secondary);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .icon-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .icon-action:hover {
    background: var(--color-surface-raised);
  }
  .help {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .error {
    color: var(--color-danger-text);
  }
</style>
