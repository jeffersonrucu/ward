<script>
  // `Código TOTP` (6909): rótulo, caixa do campo com o código em Mono/Corpo (JetBrains
  // Mono 14), pílula de contagem (ícone relógio 12 e "18 s" Texto/Etiqueta) e a `Ação de
  // campo` Copiar; ajuda "Novo código a cada 30 s.". Vazio ("Sem semente"): caixa
  // desabilitada e ajuda "Aparece com a semente salva.".
  //
  // O código vem do Rust (`codigo_totp`); a semente nunca chega aqui. Copiar pede ao
  // Rust (`copiar_codigo_totp`).
  import AcaoDeCampo from './AcaoDeCampo.svelte';
  import Icone from './Icone.svelte';

  /**
   * @type {{ id: string, codigo?: string | null, restante?: number, copiado?: boolean,
   *   oncopiar?: () => void }}
   */
  let { id, codigo = null, restante = 0, copiado = false, oncopiar = undefined } = $props();
</script>

<div class="campo">
  <span class="rotulo" id="{id}-rotulo">Código atual</span>
  <div class="caixa" class:desabilitado={!codigo} role="group" aria-labelledby="{id}-rotulo">
    {#if codigo}
      <span class="codigo" data-teste="codigo-totp">{codigo}</span>
      <span class="contagem" aria-label="troca em {restante} segundos">
        <Icone nome="relogio" tamanho="xs" />{restante} s
      </span>
      <AcaoDeCampo
        rotulo="Copiar"
        {copiado}
        onclick={() => oncopiar?.()}
        aria-label="Copiar código"
      />
    {:else}
      <span class="codigo vazio">Sem semente</span>
    {/if}
  </div>
  <span class="ajuda">{codigo ? 'Novo código a cada 30 s.' : 'Aparece com a semente salva.'}</span>
</div>

<style>
  .campo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
    min-width: 0;
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .caixa {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    height: var(--controle-padrao);
    padding: var(--espaco-0) var(--espaco-8) var(--espaco-0) var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
  }
  .caixa.desabilitado {
    background: var(--cor-superficie-desabilitada);
  }
  .codigo {
    flex: 1;
    font-family: var(--familia-mono);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    font-variant-ligatures: none;
  }
  .codigo.vazio {
    font-family: var(--familia-texto);
    color: var(--cor-texto-desabilitado);
  }
  .contagem {
    display: inline-flex;
    align-items: center;
    gap: var(--espaco-4);
    padding: var(--espaco-2) var(--espaco-8);
    border-radius: var(--raio-pilula);
    background: var(--cor-selo-neutro-fundo);
    color: var(--cor-texto-secundario);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .ajuda {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
</style>
