<script>
  // `Campo` (set 331): vazio, padrão, foco, erro, desabilitado. Largura mínima
  // largura/campo, gap espaco/8; entrada altura controle/padrao, raio controle,
  // padding 0 espaco/8 0 espaco/16, fundo superficie/campo, borda borda/sutil.
  // Só para valor que NÃO é segredo (o segredo usa `CampoSecreto`).

  /**
   * @type {{
   *   rotulo: string, valor?: string, placeholder?: string, ajuda?: string,
   *   erro?: string, desabilitado?: boolean, id: string, tipo?: string,
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
    ...resto
  } = $props();
</script>

<div class="campo" class:com-erro={erro}>
  <label class="rotulo" for={id}>{rotulo}</label>
  <input
    {id}
    class="entrada"
    type={tipo}
    bind:value={valor}
    {placeholder}
    disabled={desabilitado}
    aria-invalid={erro ? 'true' : undefined}
    aria-describedby={erro || ajuda ? `${id}-ajuda` : undefined}
    autocomplete="off"
    spellcheck="false"
    {...resto}
  />
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
  .entrada {
    height: var(--controle-padrao);
    padding: var(--espaco-0) var(--espaco-8) var(--espaco-0) var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
    color: var(--cor-texto-primario);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .entrada::placeholder {
    color: var(--cor-texto-secundario);
  }
  .entrada:focus-visible {
    outline: none;
    border-color: var(--cor-borda-foco);
  }
  .com-erro .entrada {
    border-color: var(--cor-perigo-borda);
  }
  .entrada:disabled {
    background: var(--cor-superficie-desabilitada);
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
