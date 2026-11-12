<script>
  // Step indicator (FigCraft set 741; step 1 742, 2 748, 3 754): label in secondary
  // text-label, gap space-12; bars with gap space-8, height space-4, radius pill; done
  // in color-primary-default, to do in color-border-default.

  /** @type {{ step: number, total?: number, name: string }} */
  let { step, total = 3, name } = $props();
</script>

<div class="steps">
  <span class="label">Passo {step} de {total} · {name}</span>
  <div
    class="bars"
    role="progressbar"
    aria-label="Passos do primeiro uso"
    aria-valuemin="1"
    aria-valuemax={total}
    aria-valuenow={step}
    aria-valuetext="Passo {step} de {total}"
  >
    {#each Array.from({ length: total }, (_, i) => i + 1) as n (n)}
      <span class="bar" class:done={n <= step}></span>
    {/each}
  </div>
</div>

<style>
  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    width: 100%;
  }
  .label {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .bars {
    display: flex;
    gap: var(--space-8);
  }
  .bar {
    flex: 1;
    height: var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--color-border-default);
  }
  .done {
    background: var(--color-primary-default);
  }
</style>
