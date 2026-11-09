<script>
  // `Cabeçalho de grupo` (901): altura 40, padding espaco/16 (topo) espaco/4 (lados)
  // espaco/8 (baixo), gap espaco/8; `marcador` 8×8 na cor grupo/<n>/cor; grupo em
  // Título/Seção; contagem em Texto/Legenda secundário.

  /** @type {{ grupo: string, contagem: string, cor?: 1 | 2 | 3 | 4 | 5, id?: string }} */
  let { grupo, contagem, cor = 1, id = undefined } = $props();
</script>

<div class="cabecalho" {id}>
  <span class="marcador cor-{cor}" aria-hidden="true"></span>
  <span class="grupo">{grupo}</span>
  <span class="contagem">{contagem}</span>
</div>

<style>
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    height: var(--controle-md);
    padding: var(--espaco-16) var(--espaco-4) var(--espaco-8);
    flex: none;
  }
  .marcador {
    flex: none;
    width: var(--espaco-8);
    height: var(--espaco-8);
    border-radius: var(--raio-pilula);
  }
  .cor-1 {
    background: var(--cor-grupo-1-cor);
  }
  .cor-2 {
    background: var(--cor-grupo-2-cor);
  }
  .cor-3 {
    background: var(--cor-grupo-3-cor);
  }
  .cor-4 {
    background: var(--cor-grupo-4-cor);
  }
  .cor-5 {
    background: var(--cor-grupo-5-cor);
  }
  .grupo {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-md);
    line-height: var(--altura-linha-md);
  }
  .contagem {
    flex: none;
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
</style>
