<script>
  // Modal (FigCraft set 611): default and danger. Width width-dialog, radius window,
  // padding space-24, gap space-20, footer gap space-12, shadow-modal. Uses the native
  // <dialog>: focus kept in the modal and Esc closes (V5.25). Header (gap space-12):
  // danger icon, title (title-modal, fill) and a 32 px `close` (control-sm, radius-sm)
  // with the close icon (615/634). The "X" closes like Esc: `close()` and `onclose` by
  // the `close` event, the same path as the Cancel of each dialog.
  import Icon from './Icon.svelte';

  /**
   * @type {{ title: string, open?: boolean, danger?: boolean, onclose?: () => void,
   *   children?: import('svelte').Snippet, footer?: import('svelte').Snippet }}
   */
  let { title, open = false, danger = false, onclose = undefined, children, footer } = $props();

  let dialog = $state();

  function close() {
    dialog?.close();
  }

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog bind:this={dialog} class="modal" class:danger aria-labelledby="modal-title" onclose={() => onclose?.()}>
  <div class="header">
    {#if danger}<Icon name="alert" tone="danger" />{/if}
    <h2 id="modal-title">{title}</h2>
    <button class="close" type="button" aria-label="Fechar" onclick={close}>
      <Icon name="close" />
    </button>
  </div>
  {#if children}<div class="text">{@render children()}</div>{/if}
  {#if footer}<div class="footer">{@render footer()}</div>{/if}
</dialog>

<style>
  .modal {
    width: var(--width-dialog);
    max-width: calc(100vw - 2 * var(--space-24));
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-default);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-modal);
  }
  .modal[open] {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
  }
  .modal::backdrop {
    background: var(--color-bg-overlay);
  }
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  h2 {
    flex: 1;
    min-width: 0;
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
  }
  .close {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    padding: var(--space-0);
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
  }
  .text {
    color: var(--color-text-secondary);
  }
  .footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-12);
  }
</style>
