<script>
  // `Campo senha` (set 17502): vazio, padrão, foco, erro, desabilitado, revelado. Rótulo
  // em Texto/Rótulo, caixa de altura controle/padrao com raio controle, borda
  // borda/sutil (foco borda/foco, erro perigo/texto), fundo superficie/campo, padding
  // 0 × espaco/8 × 0 × espaco/16, gap espaco/8; `Ícone à esquerda` (cadeado), valor,
  // "revelar" 32×32 (raio sm); ajuda ou erro em Texto/Pequeno.
  //
  // A senha fica só no `valor` que a tela dona liga (`bind:valor`) e tira dele ao
  // mandar; o olho mostra o que o próprio usuário digitou, só nesta caixa. Sem
  // `autocomplete` de senha salva: o navegador não guarda nem sugere a senha mestra.
  import Icone from './Icone.svelte';

  /**
   * @type {{
   *   id: string, rotulo?: string, placeholder?: string, ajuda?: string, erro?: string,
   *   valor?: string, somenteLeitura?: boolean, entrada?: HTMLInputElement | null,
   *   oninput?: () => void
   * }}
   */
  let {
    id,
    rotulo = 'Senha mestra',
    placeholder = 'Digite a senha mestra',
    ajuda = 'Só a senha mestra destrava. O PIN não serve aqui.',
    erro = '',
    valor = $bindable(''),
    somenteLeitura = false,
    entrada = $bindable(null),
    oninput = undefined,
  } = $props();

  let visivel = $state(false);
  const descricao = $derived(`${id}-ajuda`);
</script>

<div class="campo" class:com-erro={erro}>
  <label class="rotulo" for={id}>{rotulo}</label>
  <div class="entrada">
    <Icone nome="cadeado" />
    <input
      {id}
      bind:this={entrada}
      class="valor"
      class:visivel
      type={visivel ? 'text' : 'password'}
      bind:value={valor}
      {placeholder}
      readonly={somenteLeitura}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      aria-invalid={erro ? 'true' : undefined}
      aria-describedby={descricao}
      {oninput}
    />
    <button
      class="revelar"
      type="button"
      aria-label={visivel ? 'Ocultar a senha digitada' : 'Mostrar a senha digitada'}
      aria-pressed={visivel}
      onclick={() => (visivel = !visivel)}
    >
      <Icone nome={visivel ? 'olho-fechado' : 'olho'} />
    </button>
  </div>
  {#if erro}
    <span class="ajuda erro" id={descricao} role="alert">{erro}</span>
  {:else}
    <span class="ajuda" id={descricao}>{ajuda}</span>
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
  }
  .entrada {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    height: var(--controle-padrao);
    padding: var(--espaco-0) var(--espaco-8) var(--espaco-0) var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-campo);
  }
  .entrada:focus-within {
    border-color: var(--cor-borda-foco);
  }
  .com-erro .entrada {
    border-color: var(--cor-perigo-texto);
  }
  .valor {
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
  .valor:focus-visible {
    outline: none;
  }
  .valor::placeholder {
    color: var(--cor-texto-secundario);
  }
  .visivel {
    font-family: var(--familia-mono);
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
  }
  .revelar {
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
  .revelar:hover {
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
