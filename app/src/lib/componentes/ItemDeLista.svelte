<script>
  // `Item de lista` (set 553): larga/compacta × padrão/hover/selecionado. Raio item,
  // padding espaco/16, gap espaco/16; avatar 40×40 (controle/md) com o ícone do tipo;
  // colunas fixas de tipo, ambiente e nível para os badges alinharem.
  import BadgeAmbiente from './BadgeAmbiente.svelte';
  import BadgeTipo from './BadgeTipo.svelte';
  import { TIPOS } from './tipos.js';
  import Icone from './Icone.svelte';

  /**
   * @type {{ nome: string, detalhe?: string, tipo: string, ambiente: 'dev' | 'stage' | 'prod',
   *   nivel?: string, selecionado?: boolean, compacta?: boolean, onclick?: () => void }}
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
  } = $props();
</script>

<button class="item" class:selecionado class:compacta type="button" aria-current={selecionado || undefined} {onclick}>
  <span class="avatar"><Icone nome={TIPOS[tipo].icone} /></span>
  <span class="textos">
    <span class="nome">{nome}</span>
    {#if detalhe}<span class="detalhe">{detalhe}</span>{/if}
  </span>
  {#if !compacta}
    <span class="coluna-tipo"><BadgeTipo {tipo} /></span>
  {/if}
  <span class="coluna-ambiente"><BadgeAmbiente {ambiente} /></span>
  {#if nivel && !compacta}<span class="coluna-nivel">{nivel}</span>{/if}
</button>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    width: 100%;
    padding: var(--espaco-16);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid transparent;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--cor-superficie-elevada);
  }
  .selecionado,
  .selecionado:hover {
    background: var(--cor-superficie-selecionada);
    border-color: var(--cor-borda-foco);
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
  .nome,
  .detalhe {
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
