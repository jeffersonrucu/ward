<script>
  // `Interruptor` (set 6838): 36×20 (largura/interruptor × icone/md), padding espaco/2,
  // pílula; ligado primária com botão branco à direita, desligado superfície/hover com
  // botão secundário à esquerda. Versão compacta (rótulo ao lado, gap espaco/8) usada
  // no `Campo extra` (6878) e nas opções do `Popover do gerador` (11671).
  // É um `<input type="checkbox" role="switch">` de verdade.

  /**
   * @type {{ id: string, rotulo: string, ligado: boolean, desabilitado?: boolean,
   *   onmudar?: (ligado: boolean) => void }}
   */
  let { id, rotulo, ligado, desabilitado = false, onmudar = undefined } = $props();
</script>

<label class="opcao" for={id}>
  <input
    {id}
    class="visualmente-oculto"
    type="checkbox"
    role="switch"
    checked={ligado}
    disabled={desabilitado}
    onchange={(e) => onmudar?.(e.currentTarget.checked)}
  />
  <span class="interruptor" class:ligado aria-hidden="true"><span class="botao"></span></span>
  <span class="rotulo">{rotulo}</span>
</label>

<style>
  .opcao {
    display: inline-flex;
    align-items: center;
    gap: var(--espaco-8);
    cursor: pointer;
  }
  .opcao:has(input:disabled) {
    cursor: not-allowed;
    color: var(--cor-texto-desabilitado);
  }
  .rotulo {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
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
  input:focus-visible + .interruptor {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
</style>
