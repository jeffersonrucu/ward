<script>
  // `Campo` (set 331): vazio, padrão, foco, erro, desabilitado. Largura mínima
  // largura/campo, gap espaco/8; entrada altura controle/padrao, raio controle,
  // padding 0 espaco/8 0 espaco/16, gap espaco/8, fundo superficie/campo, borda
  // borda/sutil (foco: borda/foco em linha/grossa). Propriedades do componente:
  // `Ícone à esquerda` (`icone`), `Mostrar copiar` (`oncopiar`, `Ação de campo`).
  // `pilula`: raio/pilula, como a busca de Credenciais (4128). `rotuloOculto`: o rótulo
  // fica só para o leitor de tela (a busca não mostra rótulo).
  // Só para valor que NÃO é segredo (o segredo usa `CampoSecreto`).
  import AcaoDeCampo from './AcaoDeCampo.svelte';
  import Icone from './Icone.svelte';

  /**
   * @type {{
   *   rotulo: string, valor?: string, placeholder?: string, ajuda?: string,
   *   erro?: string, desabilitado?: boolean, id: string, tipo?: string,
   *   icone?: string, pilula?: boolean, rotuloOculto?: boolean, somenteLeitura?: boolean,
   *   oncopiar?: () => void, copiado?: boolean, entrada?: HTMLInputElement,
   *   [k: string]: unknown
   * }}
   */
  let {
    rotulo,
    valor = $bindable(''),
    placeholder = '',
    ajuda = '',
    erro = '',
    desabilitado = false,
    id,
    tipo = 'text',
    icone = undefined,
    pilula = false,
    rotuloOculto = false,
    somenteLeitura = false,
    oncopiar = undefined,
    copiado = false,
    entrada = $bindable(),
    ...resto
  } = $props();
</script>

<div class="campo" class:com-erro={erro}>
  <label class="rotulo" class:visualmente-oculto={rotuloOculto} for={id}>{rotulo}</label>
  <div class="caixa" class:pilula class:desabilitado>
    {#if icone}<Icone nome={icone} tom={desabilitado ? 'desabilitado' : undefined} />{/if}
    <input
      bind:this={entrada}
      {id}
      class="entrada"
      type={tipo}
      bind:value={valor}
      {placeholder}
      disabled={desabilitado}
      readonly={somenteLeitura}
      aria-invalid={erro ? 'true' : undefined}
      aria-describedby={erro || ajuda ? `${id}-ajuda` : undefined}
      autocomplete="off"
      spellcheck="false"
      {...resto}
    />
    {#if oncopiar}
      <AcaoDeCampo rotulo="Copiar" {copiado} onclick={oncopiar} aria-label="Copiar {rotulo}" />
    {/if}
  </div>
  {#if erro}
    <span class="ajuda erro" id="{id}-ajuda">{erro}</span>
  {:else if ajuda}
    <span class="ajuda" id="{id}-ajuda">{ajuda}</span>
  {/if}
</div>

<style>
  .campo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
    min-width: var(--largura-campo);
  }
  .rotulo {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-primario);
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
  .pilula {
    border-radius: var(--raio-pilula);
  }
  .caixa:focus-within {
    border-color: var(--cor-borda-foco);
    box-shadow: inset var(--espaco-0) var(--espaco-0) var(--espaco-0) var(--linha-fina) var(--cor-borda-foco);
  }
  .com-erro .caixa {
    border-color: var(--cor-perigo-borda);
  }
  .desabilitado {
    background: var(--cor-superficie-desabilitada);
  }
  .entrada {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: var(--espaco-0);
    border: none;
    background: transparent;
    color: var(--cor-texto-primario);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    text-overflow: ellipsis;
  }
  .entrada:focus-visible {
    outline: none;
  }
  .entrada::placeholder {
    color: var(--cor-texto-secundario);
  }
  .entrada:disabled {
    color: var(--cor-texto-desabilitado);
  }
  .entrada:disabled::placeholder {
    color: var(--cor-texto-desabilitado);
  }
  .ajuda {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .erro {
    color: var(--cor-perigo-texto);
  }
</style>
