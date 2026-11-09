<script>
  // `Campo extra` (set 6878; `Segredo=não` 6861, `Segredo=sim` 6879): cartão
  // superfície/padrão, borda sutil, raio item, padding espaco/16, gap espaco/12. Linha do
  // nome (Texto/Corpo forte) com o interruptor "segredo" e a `Ação de campo` Remover
  // (ícone fechar); linha do valor (ou máscara, quando segredo) e `WARD_<NOME>` em
  // Texto/Legenda secundária.
  //
  // No formulário o nome e o valor são editáveis. Extra secreto salvo (`guardado`) mostra
  // só a máscara: o valor fica no cofre e vai como "manter" até o usuário trocar (S11).
  import AcaoDeCampo from './AcaoDeCampo.svelte';
  import Interruptor from './Interruptor.svelte';

  /**
   * @type {{
   *   id: string, nome: string, valor: string, segredo: boolean, guardado?: boolean,
   *   erro?: string, onremover?: () => void, ontrocar?: () => void
   * }}
   */
  let {
    id,
    nome = $bindable(''),
    valor = $bindable(''),
    segredo = $bindable(false),
    guardado = false,
    erro = '',
    onremover = undefined,
    ontrocar = undefined,
  } = $props();

  const variavel = $derived(`WARD_${nome.trim() || '<NOME>'}`);
</script>

<div class="extra" class:com-erro={erro} data-extra={id}>
  <div class="textos">
    <input
      id="{id}-nome"
      class="nome"
      bind:value={nome}
      placeholder="NOME"
      aria-label="Nome do campo extra"
      autocomplete="off"
      spellcheck="false"
      aria-invalid={erro ? 'true' : undefined}
    />
    <div class="linha">
      {#if guardado}
        <button
          type="button"
          class="valor mascara"
          aria-label="Trocar o valor de {nome} (valor oculto)"
          onclick={() => ontrocar?.()}>••••••••</button
        >
      {:else}
        <input
          id="{id}-valor"
          class="valor"
          type={segredo ? 'password' : 'text'}
          bind:value={valor}
          placeholder="Valor"
          aria-label="Valor de {nome || 'campo extra'}"
          autocomplete={segredo ? 'new-password' : 'off'}
          spellcheck="false"
        />
      {/if}
      <span class="variavel">{variavel}</span>
    </div>
    {#if erro}<span class="erro">{erro}</span>{/if}
  </div>
  <Interruptor id="{id}-segredo" rotulo="segredo" ligado={segredo} onmudar={(v) => (segredo = v)} />
  <AcaoDeCampo
    rotulo="Remover"
    icone="fechar"
    onclick={() => onremover?.()}
    aria-label="Remover o campo extra {nome}"
  />
</div>

<style>
  .extra {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    padding: var(--espaco-16);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .extra.com-erro {
    border-color: var(--cor-perigo-borda);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    flex: 1;
    min-width: 0;
  }
  .linha {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  input,
  .mascara {
    flex: 1;
    min-width: 0;
    padding: var(--espaco-0);
    border: none;
    background: transparent;
    color: var(--cor-texto-primario);
    text-align: left;
  }
  input:focus-visible,
  .mascara:focus-visible {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
    border-radius: var(--raio-sm);
  }
  input::placeholder {
    color: var(--cor-texto-secundario);
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .valor {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .mascara {
    cursor: text;
  }
  .variavel,
  .erro {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .erro {
    color: var(--cor-perigo-texto);
  }
</style>
