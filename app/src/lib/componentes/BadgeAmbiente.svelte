<script>
  // `Badge ambiente` (set 462): dev/stage/prod × padrão (20 de altura, Texto/Selo) e
  // grande (24, Texto/Selo grande; prod grande sólido). Pílula, sem borda, tint
  // translúcido, sempre em minúsculas.

  /** @type {{ ambiente: 'dev' | 'stage' | 'prod', grande?: boolean }} */
  let { ambiente, grande = false } = $props();
</script>

<span class="badge {ambiente}" class:grande>{ambiente}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    height: calc(var(--altura-linha-xs) + 2 * var(--espaco-2));
    padding: var(--espaco-2) var(--espaco-8);
    border-radius: var(--raio-pilula);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-2xs);
    line-height: var(--altura-linha-xs);
    text-transform: lowercase;
    white-space: nowrap;
  }
  .grande {
    height: calc(var(--altura-linha-xs) + 2 * var(--espaco-4));
    padding: var(--espaco-4) var(--espaco-12);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-xs);
  }
  .dev {
    background: var(--cor-ambiente-dev-tint);
    color: var(--cor-ambiente-dev-texto);
  }
  .stage {
    background: var(--cor-ambiente-stage-tint);
    color: var(--cor-ambiente-stage-texto);
  }
  .prod {
    background: var(--cor-ambiente-prod-tint);
    color: var(--cor-ambiente-prod-texto);
  }
  .prod.grande {
    background: var(--cor-ambiente-prod-destaque);
    color: var(--cor-ambiente-prod-sobre);
  }
</style>
