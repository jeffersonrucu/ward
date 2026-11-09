<script>
  // `Destaque` (18673, Fase 10c): cartão de nota. Padding espaco/16 × espaco/20, gap
  // espaco/12, fundo superficie/padrao, borda borda/sutil, raio cartao, largura mínima
  // largura/campo; ícone opcional (`Mostrar ícone`) e texto em Texto/Pequeno
  // secundário. `tipo` (Fase 30): aviso (18684: fundo aviso/sutil, borda
  // aviso/borda-suave, ícone aviso) e info (18679: fundo info/sutil, borda
  // info/borda-suave, ícone marca), os dois com o texto em texto/primario.
  import Icone from './Icone.svelte';

  /** @type {{ icone?: string, tipo?: 'neutro' | 'aviso' | 'info', children: import('svelte').Snippet }} */
  let { icone = undefined, tipo = 'neutro', children } = $props();
  const tons = { neutro: undefined, aviso: 'aviso', info: 'marca' };
</script>

<div class="destaque {tipo}">
  {#if icone}<Icone nome={icone} tom={tons[tipo]} />{/if}
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
  .aviso {
    border-color: var(--cor-aviso-borda-suave);
    background: var(--cor-aviso-sutil);
  }
  .info {
    border-color: var(--cor-info-borda-suave);
    background: var(--cor-info-sutil);
  }
  .aviso .texto,
  .info .texto {
    color: var(--cor-texto-primario);
  }
</style>
