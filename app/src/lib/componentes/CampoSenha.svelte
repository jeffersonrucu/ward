<script>
  // `Campo senha` (set da Fase 10b; foco 17524, padrão 17511): rótulo Texto/Rótulo, gap
  // espaco/8; entrada altura controle/padrao, raio controle, padding 0 espaco/8 0
  // espaco/16, gap espaco/8, fundo superficie/campo, borda borda/sutil (foco: borda/foco
  // em linha/grossa); `Ícone à esquerda` cadeado; `ação revelar` 32×32 (controle/sm);
  // ajuda ou erro em Texto/Legenda.
  //
  // Para a senha mestra digitada (primeiro uso, destravar). O valor é do usuário e fica
  // só no estado do componente dono; o olho mostra a digitação no próprio campo e volta a
  // ocultar em 30 s. Sem autocompletar e sem corretor.
  import { onDestroy } from 'svelte';
  import Icone from './Icone.svelte';

  const PRAZO_VISIVEL_MS = 30_000;

  /**
   * @type {{
   *   rotulo: string, id: string, valor?: string, placeholder?: string, ajuda?: string,
   *   erro?: string, desabilitado?: boolean, entrada?: HTMLInputElement,
   *   [k: string]: unknown
   * }}
   */
  let {
    rotulo,
    id,
    valor = $bindable(''),
    placeholder = '',
    ajuda = '',
    erro = '',
    desabilitado = false,
    entrada = $bindable(),
    ...resto
  } = $props();

  let visivel = $state(false);
  let prazo = null;

  function alternar() {
    visivel = !visivel;
    clearTimeout(prazo);
    if (visivel) prazo = setTimeout(() => (visivel = false), PRAZO_VISIVEL_MS);
  }

  onDestroy(() => clearTimeout(prazo));
  const descricao = $derived(erro || ajuda ? `${id}-ajuda` : undefined);
</script>

<div class="campo" class:com-erro={erro}>
  <label class="rotulo" for={id}>{rotulo}</label>
  <div class="caixa" class:desabilitado>
    <Icone nome="cadeado" tom={desabilitado ? 'desabilitado' : undefined} />
    <input
      bind:this={entrada}
      {id}
      class="entrada"
      type={visivel ? 'text' : 'password'}
      bind:value={valor}
      {placeholder}
      disabled={desabilitado}
      autocomplete="new-password"
      autocapitalize="off"
      spellcheck="false"
      aria-invalid={erro ? 'true' : undefined}
      aria-describedby={descricao}
      {...resto}
    />
    <button
      class="acao-icone"
      type="button"
      disabled={desabilitado}
      aria-label={visivel ? `Ocultar ${rotulo}` : `Mostrar ${rotulo}`}
      aria-pressed={visivel}
      onclick={alternar}
    >
      <Icone nome={visivel ? 'olho-fechado' : 'olho'} />
    </button>
  </div>
  {#if erro}
    <span class="ajuda erro" id="{id}-ajuda" role="alert">{erro}</span>
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
  }
  .entrada:focus-visible {
    outline: none;
  }
  .entrada::placeholder {
    color: var(--cor-texto-secundario);
  }
  .acao-icone {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-sm);
    height: var(--controle-sm);
    border-radius: var(--raio-sm);
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .acao-icone:hover:not(:disabled) {
    background: var(--cor-superficie-elevada);
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
