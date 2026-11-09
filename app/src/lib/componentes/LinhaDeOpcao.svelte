<script>
  // `Linha de opção` (set 10379), variante `Controle=interruptor` (10368): cartão
  // superfície/padrão, borda sutil, raio item, padding espaco/20, gap espaco/16; título
  // Texto/Corpo forte, descrição Texto/Pequeno secundária (gap espaco/4).
  // `Interruptor` (set 6838): 36×20 (largura/interruptor × icone/md), padding espaco/2,
  // pílula; ligado primária com botão branco à direita, desligado superfície/hover com
  // botão secundário à esquerda.
  //
  // É um `<input type="checkbox" role="switch">` de verdade (teclado e leitor), com o
  // desenho do interruptor por cima.

  /**
   * @type {{
   *   titulo: string, id: string, ligado: boolean, desabilitado?: boolean,
   *   onmudar?: (ligado: boolean) => void, descricao?: import('svelte').Snippet
   * }}
   */
  let { titulo, id, ligado, desabilitado = false, onmudar = undefined, descricao } = $props();
</script>

<label class="linha" for={id}>
  <span class="textos">
    <span class="titulo">{titulo}</span>
    {#if descricao}<span class="descricao">{@render descricao()}</span>{/if}
  </span>
  <input
    {id}
    class="visualmente-oculto"
    type="checkbox"
    role="switch"
    checked={ligado}
    disabled={desabilitado}
    onchange={(e) => onmudar?.(e.currentTarget.checked)}
  />
  <span class="interruptor" class:ligado aria-hidden="true">
    <span class="botao"></span>
  </span>
</label>

<style>
  .linha {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-20);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
    cursor: pointer;
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .descricao {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .interruptor {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    flex: none;
    width: var(--largura-interruptor);
    height: var(--icone-md);
    padding: var(--espaco-2);
    border-radius: var(--raio-pilula);
    background: var(--cor-superficie-hover);
  }
  .botao {
    width: var(--icone-sm);
    height: var(--icone-sm);
    border-radius: var(--raio-pilula);
    background: var(--cor-texto-secundario);
  }
  .interruptor.ligado {
    justify-content: flex-end;
    background: var(--cor-primaria-padrao);
  }
  .ligado .botao {
    background: var(--cor-marca-sobre-cor);
  }
  .linha:has(input:disabled) {
    cursor: not-allowed;
  }
  input:focus-visible + .interruptor {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
</style>
