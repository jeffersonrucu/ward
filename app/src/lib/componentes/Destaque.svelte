<script>
  // `Destaque` (18673, Fase 10c): cartão de nota. Padding espaco/16 × espaco/20, gap
  // espaco/12, fundo superficie/padrao, borda borda/sutil, raio cartao, largura mínima
  // largura/campo; ícone opcional (`Mostrar ícone`) e texto em Texto/Pequeno
  // secundário.
  import Icone from './Icone.svelte';

  /** @type {{ icone?: string, children: import('svelte').Snippet }} */
  let { icone = undefined, children } = $props();
</script>

<div class="destaque">
  {#if icone}<Icone nome={icone} />{/if}
  <p class="texto">{@render children()}</p>
</div>

<style>
  .destaque {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    min-width: var(--largura-campo);
    padding: var(--espaco-16) var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .texto {
    flex: 1;
    margin: var(--espaco-0);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
</style>
