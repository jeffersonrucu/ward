<script>
  // `Item de lista` (set 553): larga/compacta × padrão/hover/selecionado. Raio item,
  // padding espaco/16; avatar 40×40 (controle/md) com o ícone do tipo.
  // - larga (gap espaco/16): colunas fixas de tipo, ambiente e nível (D23);
  // - compacta (4002/4028/4045, Fase 4f; gap espaco/12): `linha 1` = nome + `Badge
  //   ambiente`, `linha 2` = `Badge tipo` + detalhe, gap espaco/8, textos gap espaco/4;
  //   sem nível. Selecionado com borda borda/foco por dentro (traço `inside` do
  //   FigCraft: não muda a altura de 76).
  import BadgeAmbiente from './BadgeAmbiente.svelte';
  import BadgeTipo from './BadgeTipo.svelte';
  import { tipoDe } from './tipos.js';
  import Icone from './Icone.svelte';

  /**
   * @type {{ nome: string, detalhe?: string, tipo: string, ambiente: 'dev' | 'stage' | 'prod',
   *   nivel?: string, selecionado?: boolean, compacta?: boolean, onclick?: () => void,
   *   [k: string]: unknown }}
   */
  let {
    nome,
    detalhe = '',
    tipo,
    ambiente,
    nivel = '',
    selecionado = false,
    compacta = false,
    onclick = undefined,
    ...resto
  } = $props();
</script>

<button
  class="item"
  class:selecionado
  class:compacta
  type="button"
  aria-current={selecionado || undefined}
  {onclick}
  {...resto}
>
  <span class="avatar"><Icone nome={tipoDe(tipo).icone} /></span>
  {#if compacta}
    <span class="textos">
      <span class="linha">
        <span class="nome">{nome}</span>
        <BadgeAmbiente {ambiente} />
      </span>
      <span class="linha">
        <BadgeTipo {tipo} />
        {#if detalhe}<span class="detalhe">{detalhe}</span>{/if}
      </span>
    </span>
  {:else}
    <span class="textos">
      <span class="nome">{nome}</span>
      {#if detalhe}<span class="detalhe">{detalhe}</span>{/if}
    </span>
    <span class="coluna-tipo"><BadgeTipo {tipo} /></span>
    <span class="coluna-ambiente"><BadgeAmbiente {ambiente} /></span>
    {#if nivel}<span class="coluna-nivel">{nivel}</span>{/if}
  {/if}
</button>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    width: 100%;
    padding: var(--espaco-16);
    border-radius: var(--raio-item);
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .compacta {
    gap: var(--espaco-12);
  }
  .item:hover {
    background: var(--cor-superficie-elevada);
  }
  .selecionado,
  .selecionado:hover {
    background: var(--cor-superficie-selecionada);
    box-shadow: inset var(--espaco-0) var(--espaco-0) var(--espaco-0) var(--linha-fina)
      var(--cor-borda-foco);
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--controle-md);
    height: var(--controle-md);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .textos {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .compacta .textos {
    gap: var(--espaco-4);
  }
  .linha {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    min-width: 0;
  }
  .nome,
  .detalhe {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .detalhe {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .coluna-tipo,
  .coluna-ambiente,
  .coluna-nivel {
    display: flex;
    align-items: center;
  }
  .coluna-tipo {
    flex: none;
    width: var(--largura-coluna-tipo);
  }
  .coluna-ambiente {
    flex: none;
    width: var(--largura-coluna-ambiente);
  }
  .coluna-nivel {
    flex: none;
    width: var(--largura-coluna-nivel);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
</style>
