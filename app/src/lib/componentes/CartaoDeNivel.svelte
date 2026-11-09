<script>
  // `Cartão de nível` (set 18998; variantes `Nível=livre` 18986, `Nível=com aprovação`
  // 18999, `Nível=bloqueado` 19011): frame horizontal, fundo superficie/padrao, borda
  // borda/sutil, raio cartao, padding espaco/20, gap espaco/16, itens ao centro; `ícone`
  // 20 px (icone/md) no tom do ícone; `textos` (gap espaco/4): rótulo "Nível de acesso"
  // (Texto/Legenda, secundário), nível (Texto/Corpo forte) e explicação (Texto/Pequeno,
  // secundário); `Botão` secundário "Mudar nível" com `Ícone/cadeado`. Largura `fill`.
  import Botao from './Botao.svelte';
  import Icone from './Icone.svelte';
  import { NIVEIS_DE_ACESSO, nivelDe } from './niveis.js';

  /**
   * @type {{ nivel: string, desabilitado?: boolean, onmudar?: () => void }}
   */
  let { nivel, desabilitado = false, onmudar = undefined } = $props();

  const n = $derived(nivelDe(nivel) ?? NIVEIS_DE_ACESSO['com-aprovacao']);
</script>

<div class="cartao" data-nivel={nivel}>
  <Icone nome={n.icone} />
  <div class="textos">
    <span class="rotulo">Nível de acesso</span>
    <span class="nome">{n.rotulo}</span>
    <span class="explicacao">{n.cartao}</span>
  </div>
  <Botao
    variante="secundario"
    icone="cadeado"
    {desabilitado}
    aria-haspopup="dialog"
    onclick={() => onmudar?.()}>Mudar nível</Botao
  >
</div>

<style>
  .cartao {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
    min-width: 0;
  }
  .rotulo {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
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
