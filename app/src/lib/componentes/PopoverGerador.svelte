<script>
  // `Popover do gerador` (11671), aberto pelo gerar do campo Senha (frame `Gerador de
  // senha / No formulário` 14307): largura/dialogo, padding espaco/20, gap espaco/16,
  // raio cartão, superfície elevada, borda padrão, Sombra/Flutuante. Cabeçalho "Gerar
  // senha" (Texto/Título de seção) com fechar; `Campo secreto` "Senha gerada" com revelar
  // e copiar; tamanho (rótulo e "24 caracteres" em Mono/Pequeno) com o `Controle
  // deslizante`; conjuntos em interruptores compactos; rodapé com "Gerar de novo" e "Usar
  // esta senha".
  //
  // S11/D4 (D65 f): a senha nasce no Rust (`gerar_senha`, tamanho 8–64, símbolos de
  // `ward_core::gerador::SIMBOLOS`) e fica lá. "Usar esta senha" chama
  // `usar_senha_gerada`, que passa a senha ao campo no lado Rust; aqui volta só o
  // tamanho. O valor só entra no "revelar" (30 s, esconder ou fechar).
  import { onDestroy, onMount } from 'svelte';
  import Botao from './Botao.svelte';
  import CampoSecreto from './CampoSecreto.svelte';
  import ControleDeslizante from './ControleDeslizante.svelte';
  import Icone from './Icone.svelte';
  import Interruptor from './Interruptor.svelte';
  import Toast from './Toast.svelte';
  import { chamar } from '../ponte/index.js';

  // Espelho de `ward_app_core::gerador::{TAMANHO_MIN, TAMANHO_MAX, TAMANHO_PADRAO}`.
  const TAMANHO_MIN = 8;
  const TAMANHO_MAX = 64;
  const TAMANHO_PADRAO = 24;
  const PRAZO_REVELAR_MS = 30_000;
  const PRAZO_COPIADO_MS = 2_000;
  // Limiares da Fase 37 (bits): fraca < 60, razoável < 80, forte < 128, muito forte.
  const FORCAS = {
    fraca: 'fraca',
    razoavel: 'razoável',
    forte: 'forte',
    'muito-forte': 'muito forte',
  };

  /** @type {{ campo: string, onusar: (tamanho: number) => void, onfechar: () => void }} */
  let { campo, onusar, onfechar } = $props();

  let tamanho = $state(TAMANHO_PADRAO);
  let conjuntos = $state({ minusculas: true, maiusculas: true, digitos: true, simbolos: true });
  /** @type {{ tamanho: number, bits: number, forca: string, simbolos: string } | null} */
  let gerada = $state(null);
  /** @type {string | null} */
  let revelado = $state(null);
  let copiado = $state(false);
  let erro = $state('');
  let ativa = true;
  let prazoRevelar = 0;
  let prazoCopiado = 0;
  /** @type {HTMLElement | undefined} */
  let caixa = $state();

  const ligados = $derived(Object.values(conjuntos).filter(Boolean).length);

  function esconder() {
    clearTimeout(prazoRevelar);
    revelado = null;
  }

  let gerando = false;
  let deNovo = false;
  async function gerar() {
    esconder();
    copiado = false;
    if (gerando) {
      deNovo = true;
      return;
    }
    gerando = true;
    try {
      do {
        deNovo = false;
        try {
          const r = await chamar('gerar_senha', { tamanho, ...conjuntos });
          if (ativa) {
            gerada = r;
            erro = '';
          }
        } catch (e) {
          if (ativa) {
            gerada = null;
            erro = e.mensagem;
          }
        }
      } while (deNovo && ativa);
    } finally {
      gerando = false;
    }
  }

  async function revelar() {
    try {
      const r = await chamar('revelar_senha_gerada');
      if (!ativa || gerando) return;
      revelado = r.valor;
      clearTimeout(prazoRevelar);
      prazoRevelar = setTimeout(esconder, PRAZO_REVELAR_MS);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  async function copiar() {
    try {
      await chamar('copiar_senha_gerada');
      copiado = true;
      clearTimeout(prazoCopiado);
      prazoCopiado = setTimeout(() => (copiado = false), PRAZO_COPIADO_MS);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  async function usar() {
    esconder();
    try {
      const r = await chamar('usar_senha_gerada', { campo });
      onusar(r.tamanho);
    } catch (e) {
      erro = e.mensagem;
    }
  }

  function mudarTamanho(v) {
    tamanho = Math.min(TAMANHO_MAX, Math.max(TAMANHO_MIN, Math.round(v)));
    gerar();
  }

  function mudarConjunto(nome, ligado) {
    if (!ligado && ligados <= 1) return;
    conjuntos[nome] = ligado;
    gerar();
  }

  function tecla(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onfechar();
    }
  }

  onMount(() => {
    gerar();
    caixa?.focus();
  });

  onDestroy(() => {
    ativa = false;
    esconder();
    clearTimeout(prazoCopiado);
    // A senha do gerador sai do Rust; a que foi "usada" fica até salvar ou cancelar.
    chamar('descartar_senha_gerada').catch(() => {});
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="popover"
  role="dialog"
  aria-modal="false"
  aria-labelledby="popover-gerador-titulo"
  tabindex="-1"
  bind:this={caixa}
  onkeydown={tecla}
>
  <header class="cabecalho">
    <h3 id="popover-gerador-titulo" class="titulo">Gerar senha</h3>
    <button class="fechar" type="button" aria-label="Fechar o gerador" onclick={onfechar}>
      <Icone nome="fechar" />
    </button>
  </header>
  <CampoSecreto
    id="popover-senha"
    rotulo="Senha gerada"
    vazio={!gerada}
    mascara={'•'.repeat(gerada?.tamanho ?? 0)}
    ajuda="A senha só aparece no revelar. Copiar limpa a área de transferência em 30 s."
    {revelado}
    {copiado}
    onrevelar={revelar}
    onesconder={esconder}
    oncopiar={copiar}
  />
  <div class="tamanho">
    <div class="linha">
      <label class="rotulo" for="popover-tamanho">Tamanho</label>
      <span class="valor-tamanho">{tamanho} caracteres</span>
    </div>
    <ControleDeslizante
      id="popover-tamanho"
      rotuloAcessivel="Tamanho da senha"
      valor={tamanho}
      minimo={TAMANHO_MIN}
      maximo={TAMANHO_MAX}
      oninput={mudarTamanho}
    />
    {#if gerada}
      <span class="forca" data-forca={gerada.forca}>
        {FORCAS[gerada.forca] ?? ''} · cerca de {gerada.bits} bits
      </span>
    {/if}
  </div>
  <div class="conjuntos">
    <div class="linha-opcoes">
      <Interruptor
        id="popover-minusculas"
        rotulo="Minúsculas"
        ligado={conjuntos.minusculas}
        desabilitado={conjuntos.minusculas && ligados === 1}
        onmudar={(v) => mudarConjunto('minusculas', v)}
      />
      <Interruptor
        id="popover-maiusculas"
        rotulo="Maiúsculas"
        ligado={conjuntos.maiusculas}
        desabilitado={conjuntos.maiusculas && ligados === 1}
        onmudar={(v) => mudarConjunto('maiusculas', v)}
      />
    </div>
    <div class="linha-opcoes">
      <Interruptor
        id="popover-digitos"
        rotulo="Dígitos"
        ligado={conjuntos.digitos}
        desabilitado={conjuntos.digitos && ligados === 1}
        onmudar={(v) => mudarConjunto('digitos', v)}
      />
      <Interruptor
        id="popover-simbolos"
        rotulo="Símbolos"
        ligado={conjuntos.simbolos}
        desabilitado={conjuntos.simbolos && ligados === 1}
        onmudar={(v) => mudarConjunto('simbolos', v)}
      />
    </div>
    {#if gerada?.simbolos}
      <span class="simbolos" data-teste="simbolos">{gerada.simbolos}</span>
    {/if}
  </div>
  {#if erro}<Toast tipo="erro" mensagem={erro} />{/if}
  <footer class="rodape">
    <Botao variante="fantasma" icone="gerador" onclick={gerar}>Gerar de novo</Botao>
    <Botao variante="primario" onclick={usar} desabilitado={!gerada}>Usar esta senha</Botao>
  </footer>
</div>

<style>
  .popover {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    width: var(--largura-dialogo);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    box-shadow: var(--sombra-flutuante);
  }
  .popover:focus-visible {
    outline: none;
  }
  .cabecalho,
  .linha,
  .rodape {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  .titulo {
    flex: 1;
    margin: 0;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-md);
    line-height: var(--altura-linha-md);
  }
  .fechar {
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
  .fechar:hover {
    background: var(--cor-superficie-hover);
  }
  .tamanho,
  .conjuntos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
  }
  .rotulo {
    flex: 1;
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .valor-tamanho {
    color: var(--cor-texto-secundario);
    font-family: var(--familia-mono);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .forca,
  .simbolos {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
  }
  .forca[data-forca='fraca'] {
    color: var(--cor-perigo-texto);
  }
  .forca[data-forca='razoavel'] {
    color: var(--cor-aviso-texto);
  }
  .forca[data-forca='forte'],
  .forca[data-forca='muito-forte'] {
    color: var(--cor-sucesso-texto);
  }
  .simbolos {
    overflow-wrap: anywhere;
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
  }
  .linha-opcoes {
    display: flex;
    gap: var(--espaco-12);
  }
  .linha-opcoes > :global(*) {
    flex: 1;
  }
  .rodape {
    justify-content: flex-end;
  }
</style>
