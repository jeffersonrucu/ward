<script>
  // Combobox de grupo (FigCraft 7691): `Campo` com ícone busca; o menu tem fundo
  // superfície elevada, padding espaco/4, gap espaco/2, opções de controle/md com padding
  // 0 espaco/12; a opção encontrada fica selecionada; a última é "Criar grupo “ac”" com o
  // ícone mais. Padrão ARIA combobox com listbox (setas, Enter, Esc).

  import Icone from './Icone.svelte';

  /**
   * @type {{ id: string, valor?: string, grupos: string[], erro?: string }}
   */
  let { id, valor = $bindable(''), grupos, erro = '' } = $props();

  let aberto = $state(false);
  let ativo = $state(0);

  const termo = $derived(valor.trim());
  const achados = $derived(
    grupos.filter((g) => g.toLowerCase().includes(termo.toLowerCase())).slice(0, 8),
  );
  const podeCriar = $derived(termo !== '' && !grupos.includes(termo));
  const opcoes = $derived([
    ...achados.map((g) => ({ valor: g, rotulo: g, criar: false })),
    ...(podeCriar ? [{ valor: termo, rotulo: `Criar grupo “${termo}”`, criar: true }] : []),
  ]);

  function escolher(o) {
    valor = o.valor;
    aberto = false;
  }

  function tecla(e) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      aberto = true;
      ativo = Math.min(ativo + 1, opcoes.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      ativo = Math.max(ativo - 1, 0);
    } else if (e.key === 'Enter' && aberto && opcoes[ativo]) {
      e.preventDefault();
      escolher(opcoes[ativo]);
    } else if (e.key === 'Escape' && aberto) {
      e.preventDefault();
      aberto = false;
    }
  }
</script>

<div class="campo" class:com-erro={erro}>
  <label class="rotulo" for={id}>Grupo</label>
  <div class="caixa">
    <Icone nome="busca" />
    <input
      {id}
      class="entrada"
      role="combobox"
      aria-expanded={aberto && opcoes.length > 0}
      aria-controls="{id}-opcoes"
      aria-autocomplete="list"
      aria-activedescendant={aberto && opcoes[ativo] ? `${id}-opcao-${ativo}` : undefined}
      aria-invalid={erro ? 'true' : undefined}
      aria-describedby={erro ? `${id}-ajuda` : undefined}
      bind:value={valor}
      placeholder="Ex.: acme"
      autocomplete="off"
      spellcheck="false"
      onfocus={() => (aberto = true)}
      oninput={() => {
        aberto = true;
        ativo = 0;
      }}
      onblur={() => (aberto = false)}
      onkeydown={tecla}
    />
  </div>
  {#if aberto && opcoes.length > 0}
    <ul class="menu" id="{id}-opcoes" role="listbox" aria-label="Grupos">
      {#each opcoes as o, i (o.rotulo)}
        <li
          id="{id}-opcao-{i}"
          class="opcao"
          class:ativa={i === ativo}
          class:selecionada={!o.criar && o.valor === termo}
          role="option"
          aria-selected={i === ativo}
          onmousedown={(e) => {
            e.preventDefault();
            escolher(o);
          }}
        >
          {#if o.criar}<Icone nome="mais" />{/if}
          <span>{o.rotulo}</span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if erro}<span class="ajuda erro" id="{id}-ajuda">{erro}</span>{/if}
</div>

<style>
  .campo {
    position: relative;
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
  .caixa:focus-within {
    border-color: var(--cor-borda-foco);
  }
  .com-erro .caixa {
    border-color: var(--cor-perigo-borda);
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
  .menu {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    margin: var(--espaco-4) 0 0;
    padding: var(--espaco-4);
    list-style: none;
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    box-shadow: var(--sombra-flutuante);
  }
  .opcao {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
    height: var(--controle-md);
    padding: var(--espaco-0) var(--espaco-12);
    border-radius: var(--raio-sm);
    cursor: pointer;
    font-size: var(--tamanho-base);
  }
  .opcao.ativa {
    background: var(--cor-superficie-hover);
  }
  .opcao.selecionada {
    background: var(--cor-superficie-selecionada);
  }
  .ajuda {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .erro {
    color: var(--cor-perigo-texto);
  }
</style>
