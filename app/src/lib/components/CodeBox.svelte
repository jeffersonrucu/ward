<script>
  // Code box (FigCraft 16340): padding space-16, gap space-4, radius control, background
  // color-bg-default, border color-border-subtle; label in secondary text-caption and
  // content in primary mono code, breaking anywhere (the agent text may have no
  // spaces). The content goes in as text (never HTML).

  /** @type {{ label: string, children: import('svelte').Snippet }} */
  let { label, children } = $props();
</script>

<div class="box">
  <span class="label">{label}</span>
  <div class="content">{@render children()}</div>
</div>

<style>
  .box {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    width: 100%;
    padding: var(--space-16);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-default);
  }
  .label {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .content {
    font-family: var(--font-family-mono);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    /* D51: no ligatures and no shaping; each character as it is. */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
    /* Always left to right, isolated from the rest: the first character of the agent
       does not choose the direction of the block. */
    direction: ltr;
    unicode-bidi: isolate;
  }
</style>
