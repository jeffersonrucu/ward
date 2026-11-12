<script>
  // Countdown (FigCraft set 16348): pill of 20 (padding space-2 space-8 space-2
  // space-6, gap space-4, radius pill), clock icon 12 and the time in text-tag.
  // Normal state (16335): background color-warning-subtle, text and icon warning.
  // Urgent state (16343): background color-danger-subtle, text and icon danger.
  import Icon from './Icon.svelte';

  /** @type {{ seconds: number, urgent?: boolean }} */
  let { seconds, urgent = false } = $props();

  const text = $derived.by(() => {
    const s = Math.max(0, Math.floor(seconds));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')} restantes`;
  });
</script>

<span class="countdown" class:urgent role="timer" aria-live="off">
  <Icon name="clock" size="xs" tone={urgent ? 'danger' : 'warning'} />
  {text}
</span>

<style>
  .countdown {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-4);
    height: calc(var(--line-height-xs) + 2 * var(--space-2));
    padding: var(--space-2) var(--space-8) var(--space-2) var(--space-6);
    border-radius: var(--radius-pill);
    background: var(--color-warning-subtle);
    color: var(--color-warning-text);
    font-weight: var(--weight-medium);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .urgent {
    background: var(--color-danger-subtle);
    color: var(--color-danger-text);
  }
</style>
