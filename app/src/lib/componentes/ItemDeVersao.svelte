<script>
  // `Item de versão` (set 11264; padrão 11243, selecionado 11265): `fill`, padding
  // espaco/16, gap espaco/12, itens no centro, raio item, borda linha/fina. Padrão: fundo
  // superficie/padrao, borda borda/sutil, rádio 20 px (icone/md, raio total) com borda
  // linha/grossa em borda/forte. Selecionado: fundo superficie/selecionada, borda e rádio
  // em borda/foco, ponto de 8 px (espaco/8) em primaria/padrao. `textos` gap espaco/2:
  // data em Texto/Corpo forte e detalhe em Texto/Pequeno secundário; "quando" em
  // Texto/Legenda secundário à direita.
  //
  // Na lista é um `<input type="radio">` de verdade (setas trocam a versão, leitor de tela
  // lê o grupo), com o desenho por cima; com `estatico` (o resumo do `Confirmar`) é só o
  // desenho do selecionado.
  //
  // Fase 49a: `alerta` (a gravação é mais antiga do que a data do nome diz) pinta o
  // detalhe em aviso/texto; o texto já diz o motivo (não depende só da cor).

  /**
   * @type {{ id: string, data: string, detalhe: string, quando: string,
   *   nome?: string, valor?: string, marcado?: boolean, estatico?: boolean,
   *   alerta?: boolean, onescolher?: (v: string) => void }}
   */
  let {
    id,
    data,
    detalhe,
    quando,
    nome = 'versao',
    valor = '',
    marcado = false,
    estatico = false,
    alerta = false,
    onescolher = undefined,
  } = $props();
</script>

{#snippet desenho(selecionado)}
  <span class="radio" class:selecionado aria-hidden="true">
    {#if selecionado}<span class="ponto"></span>{/if}
  </span>
  <span class="textos">
    <span class="data">{data}</span>
    <span class="detalhe" class:alerta>{detalhe}</span>
  </span>
  <span class="quando">{quando}</span>
{/snippet}

{#if estatico}
  <div class="item selecionado" {id}>{@render desenho(true)}</div>
{:else}
  <label class="item" class:selecionado={marcado} for={id}>
    <input
      {id}
      class="visualmente-oculto"
      type="radio"
      name={nome}
      value={valor}
      checked={marcado}
      onchange={() => onescolher?.(valor)}
    />
    {@render desenho(marcado)}
  </label>
{/if}

<style>
  .item {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    padding: var(--espaco-16);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
    color: var(--cor-texto-primario);
  }
  label.item {
    cursor: pointer;
  }
  .item.selecionado {
    border-color: var(--cor-borda-foco);
    background: var(--cor-superficie-selecionada);
  }
  /* Por dentro: a lista rola e cortaria o contorno de fora. */
  .item:has(:focus-visible) {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: calc(-1 * var(--linha-grossa));
  }
  .radio {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    box-sizing: border-box;
    width: var(--icone-md);
    height: var(--icone-md);
    border-radius: var(--raio-total);
    border: var(--linha-grossa) solid var(--cor-borda-forte);
  }
  .radio.selecionado {
    border-color: var(--cor-borda-foco);
  }
  .ponto {
    width: var(--espaco-8);
    height: var(--espaco-8);
    border-radius: var(--raio-total);
    background: var(--cor-primaria-padrao);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
    min-width: 0;
  }
  .data {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .detalhe {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .detalhe.alerta {
    color: var(--cor-aviso-texto);
  }
  .quando {
    flex: none;
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
</style>
