<script>
  // USB drive card of the settings screen with the external backup on (FigCraft 16318,
  // Phase 48): color-surface-default, border color-border-focus (6:71), radius item,
  // padding space-20, gap space-16. Avatar 40 (control-md) in color-surface-raised with
  // color-border-default and radius control; texts with gap space-2: name in
  // text-body-strong, detail in secondary text-small, status in text-caption with
  // color-success-text (6:86). Secondary change Button on the right.
  //
  // The label comes from udisks2 (untrusted): it arrives from the Rust side with the
  // invisible characters escaped (D51) and here it is only text (Svelte interpolation,
  // never HTML), isolated in direction (`<bdi>`). The design shows the mount path; the
  // protocol does not send it to the interface (Phase 46), so the detail only says
  // whether it is connected and mounted.
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';

  /**
   * @type {{ name: string, uuid: string, situation: string, status: string,
   *   mounted?: boolean, onchange?: () => void, disabled?: boolean }}
   */
  let {
    name,
    uuid,
    situation,
    status,
    mounted = false,
    onchange = undefined,
    disabled = false,
  } = $props();
</script>

<div class="card" id="config-usb-drive">
  <span class="avatar" aria-hidden="true">
    <Icon name={mounted ? 'success' : 'clock'} tone={mounted ? 'success' : undefined} />
  </span>
  <span class="texts">
    <span class="name"><bdi>{name}</bdi></span>
    <span class="detail">UUID <bdi class="uuid">{uuid}</bdi> · {situation}</span>
    <span class="status" class:mounted>{status}</span>
  </span>
  <Button variant="secondary" onclick={() => onchange?.()} {disabled} aria-label="Trocar pendrive"
    >Trocar</Button
  >
</div>

<style>
  .card {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-item);
    border: var(--line-thin) solid var(--color-border-focus);
    background: var(--color-surface-default);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--control-md);
    height: var(--control-md);
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
  .name,
  .detail,
  .status {
    overflow-wrap: anywhere;
  }
  bdi {
    unicode-bidi: isolate;
  }
  .name {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .detail {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .uuid {
    direction: ltr;
  }
  .status {
    color: var(--color-text-secondary);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
  }
  .status.mounted {
    color: var(--color-success-text);
  }
</style>
