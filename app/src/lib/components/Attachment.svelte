<script>
  // Attachment (FigCraft 6849): card color-surface-default, subtle border, radius item,
  // padding space-16, gap space-12. Avatar control-sm (radius control, raised surface,
  // default border) with the paperclip icon 16; name in text-body-strong and detail
  // (size and the delivered variable) in secondary text-caption; Remove field action
  // (close icon). The content of the file never shows (S11, V5.12).
  import FieldAction from './FieldAction.svelte';
  import Icon from './Icon.svelte';

  /** @type {{ name: string, detail: string, onremove?: () => void }} */
  let { name, detail, onremove = undefined } = $props();
</script>

<div class="attachment" data-attachment={name}>
  <span class="avatar"><Icon name="paperclip" size="sm" /></span>
  <div class="texts">
    <span class="name">{name}</span>
    <span class="detail">{detail}</span>
  </div>
  {#if onremove}
    <FieldAction label="Remover" icon="close" aria-label="Remover {name}" onclick={onremove} />
  {/if}
</div>

<style>
  .attachment {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-16);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .avatar {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
</style>
