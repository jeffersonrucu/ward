<script>
  // `Indicador de passos` (set 741; Passo=1 742, 2 748, 3 754): rótulo Texto/Rótulo
  // secundário, gap espaco/12; barras com gap espaco/8, altura espaco/4, raio pílula;
  // feitas em primaria/padrao, por fazer em borda/padrao.

  /** @type {{ passo: number, total?: number, nome: string }} */
  let { passo, total = 3, nome } = $props();
</script>

<div class="passos">
  <span class="rotulo">Passo {passo} de {total} · {nome}</span>
  <div
    class="barras"
    role="progressbar"
    aria-label="Passos do primeiro uso"
    aria-valuemin="1"
    aria-valuemax={total}
    aria-valuenow={passo}
    aria-valuetext="Passo {passo} de {total}"
  >
    {#each Array.from({ length: total }, (_, i) => i + 1) as n (n)}
      <span class="barra" class:feita={n <= passo}></span>
    {/each}
  </div>
</div>

<style>
  .passos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
    width: 100%;
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .barras {
    display: flex;
    gap: var(--espaco-8);
  }
  .barra {
    flex: 1;
    height: var(--espaco-4);
    border-radius: var(--raio-pilula);
    background: var(--cor-borda-padrao);
  }
  .feita {
    background: var(--cor-primaria-padrao);
  }
</style>
