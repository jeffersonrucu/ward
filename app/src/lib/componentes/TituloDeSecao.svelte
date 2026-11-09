<script>
  // `Título de seção` (6841): título Texto/Corpo forte (14/500), descrição Texto/Legenda
  // secundária (12), gap espaco/2; ação opcional à direita (botão fantasma).

  /** @type {{ id: string, titulo: string, descricao?: string, acao?: import('svelte').Snippet }} */
  let { id, titulo, descricao = '', acao = undefined } = $props();
</script>

<div class="secao">
  <div class="textos">
    <h3 class="titulo" {id}>{titulo}</h3>
    {#if descricao}<p class="descricao">{descricao}</p>{/if}
  </div>
  {#if acao}{@render acao()}{/if}
</div>

<style>
  .secao {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
    min-width: 0;
  }
  .titulo,
  .descricao {
    margin: 0;
  }
  .titulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .descricao {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
</style>
