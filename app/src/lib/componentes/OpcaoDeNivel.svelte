<script>
  // `Opção de nível` (set 909; Nível × Selecionado: bloqueado 910/920, com aprovação
  // 931/940, livre 950/958): cartão em `hug`, padding espaco/20, gap espaco/16, raio
  // cartao, itens no topo. Não selecionada: fundo superficie/campo, borda borda/padrao,
  // seletor 20 px (icone/md, raio total) com borda borda/forte. Selecionada: fundo
  // primaria/sutil, borda borda/foco, seletor com borda borda/foco e o ponto de 8 px
  // (espaco/8) em primaria/padrao. `textos` gap espaco/6: título (gap espaco/8: ícone
  // 20 px e nome em Texto/Corpo forte) e explicação em Texto/Pequeno secundário.
  //
  // É um `<input type="radio">` de verdade (setas trocam a opção, leitor de tela lê o
  // grupo), com o desenho por cima.
  import Icone from './Icone.svelte';
  import { nivelDe } from './niveis.js';

  /**
   * @type {{ nivel: string, nome: string, marcado: boolean, onescolher?: (n: string) => void }}
   */
  let { nivel, nome, marcado, onescolher = undefined } = $props();

  const n = $derived(nivelDe(nivel));
  const id = $derived(`opcao-nivel-${nivel}`);
</script>

{#if n}
  <label class="opcao" class:marcado for={id}>
    <input
      {id}
      class="visualmente-oculto"
      type="radio"
      name={nome}
      value={nivel}
      checked={marcado}
      aria-describedby="{id}-explicacao"
      onchange={() => onescolher?.(nivel)}
    />
    <span class="seletor" aria-hidden="true">
      {#if marcado}<span class="ponto"></span>{/if}
    </span>
    <span class="textos">
      <span class="titulo">
        <Icone nome={n.icone} />
        <span class="nome">{n.rotulo}</span>
      </span>
      <span class="explicacao" id="{id}-explicacao">{n.opcao}</span>
    </span>
  </label>
{/if}

<style>
  .opcao {
    display: flex;
    align-items: flex-start;
    gap: var(--espaco-16);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-campo);
    cursor: pointer;
  }
  .opcao.marcado {
    border-color: var(--cor-borda-foco);
    background: var(--cor-primaria-sutil);
  }
  .opcao:has(:focus-visible) {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
  .seletor {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--icone-md);
    height: var(--icone-md);
    border-radius: var(--raio-total);
    border: var(--linha-fina) solid var(--cor-borda-forte);
  }
  .marcado .seletor {
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
    gap: var(--espaco-6);
    flex: 1;
    min-width: 0;
  }
  .titulo {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .explicacao {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
</style>
