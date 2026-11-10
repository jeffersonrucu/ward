<script>
  // Environment badge (FigCraft set 462): dev/stage/prod × default (20 high,
  // text-badge) and large (24, text-badge-large; prod large is solid). Pill, no border,
  // translucent tint, always lowercase.

  /** @type {{ environment: 'dev' | 'stage' | 'prod', large?: boolean }} */
  let { environment, large = false } = $props();
</script>

<span class="badge {environment}" class:large>{environment}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    height: calc(var(--line-height-xs) + 2 * var(--space-2));
    padding: var(--space-2) var(--space-8);
    border-radius: var(--radius-pill);
    font-weight: var(--weight-medium);
    font-size: var(--size-2xs);
    line-height: var(--line-height-xs);
    text-transform: lowercase;
    white-space: nowrap;
  }
  .large {
    height: calc(var(--line-height-xs) + 2 * var(--space-4));
    padding: var(--space-4) var(--space-12);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xs);
  }
  .dev {
    background: var(--color-environment-dev-tint);
    color: var(--color-environment-dev-text);
  }
  .stage {
    background: var(--color-environment-stage-tint);
    color: var(--color-environment-stage-text);
  }
  .prod {
    background: var(--color-environment-prod-tint);
    color: var(--color-environment-prod-text);
  }
  .prod.large {
    background: var(--color-environment-prod-highlight);
    color: var(--color-environment-prod-on);
  }
</style>
