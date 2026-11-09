<script>
  // Janela de aprovação (Fase 36; S9, S10, D51). Frames `Aprovação / Normal` (16701),
  // `/ prod` (16810), `/ PIN errado` (16885), `/ PIN bloqueado` (16960) e `/ Expirando`
  // (17035): painel de 560 (largura/dialogo-largo), padding espaco/24, gap espaco/20,
  // raio janela, borda borda/sutil, Sombra/Modal.
  //
  // - O pedido vem do lado Rust (`pedido_de_aprovacao`) com o texto do agente já
  //   neutralizado (D51) e entra aqui só como texto: nada de HTML. A janela consulta ao
  //   abrir, a cada aviso `pedido-de-aprovacao` do canal (`ouvir_avisos`, o mesmo da
  //   Fase 31, um por janela), depois de responder e quando a contagem chega a zero. A
  //   contagem anda aqui só para mostrar; quem expira o pedido é o daemon (S10). Nada
  //   fica fora do componente.
  // - Foco inicial em "Negar"; Esc nega; Enter no PIN não permite nada (V5.19). Fechar
  //   (o X ou a janela) nega (S10). O PIN fica só no estado do `Campo PIN` e é limpo a
  //   cada envio.
  // - Só esta janela responde: no app, a capability `aprovacao` (a principal não tem os
  //   dois comandos) e o comando confere o rótulo da janela de novo.
  import { onDestroy, onMount, tick } from 'svelte';
  import BadgeAmbiente from '../lib/componentes/BadgeAmbiente.svelte';
  import BadgeTipo from '../lib/componentes/BadgeTipo.svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import BotaoIcone from '../lib/componentes/BotaoIcone.svelte';
  import CaixaDeCodigo from '../lib/componentes/CaixaDeCodigo.svelte';
  import CampoPin from '../lib/componentes/CampoPin.svelte';
  import Contagem from '../lib/componentes/Contagem.svelte';
  import Destaque from '../lib/componentes/Destaque.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import { tipoDe } from '../lib/componentes/tipos.js';
  import simbolo from '../lib/componentes/ward-simbolo.svg';
  import { chamar, ouvirAvisos } from '../lib/ponte/index.js';
  import {
    ID_NEGAR,
    SEGUNDOS_URGENTE,
    SEPARADOR,
    TEXTOS,
    mensagemDoResultado,
    pinSuficiente,
  } from './aprovacao.js';

  let pedido = $state(null);
  let carregado = $state(false);
  let pin = $state('');
  let erro = $state('');
  let ocupado = $state(false);
  let falha = $state('');

  const bloqueado = $derived(pedido?.estado === 'bloqueado');
  const prod = $derived(pedido?.ambiente === 'prod');
  const urgente = $derived(!bloqueado && pedido !== null && pedido.restante <= SEGUNDOS_URGENTE);

  let consultando = false;
  let relogio = null;
  let pararDeOuvir = null;
  let saiu = false;

  async function focarNegar() {
    await tick();
    const b = document.getElementById(ID_NEGAR);
    if (b) b.focus({ focusVisible: true });
  }

  async function consultar() {
    if (consultando || ocupado) return;
    consultando = true;
    try {
      const novo = await chamar('pedido_de_aprovacao');
      const outro = (novo?.id ?? null) !== (pedido?.id ?? null);
      pedido = novo;
      falha = '';
      if (outro) {
        pin = '';
        erro = '';
        if (novo) focarNegar();
      }
    } catch (e) {
      falha = e.mensagem;
    } finally {
      carregado = true;
      consultando = false;
    }
  }

  async function responder(decisao) {
    if (!pedido || ocupado) return;
    const id = pedido.id;
    const comPin = decisao !== 'negar';
    if (comPin && !pinSuficiente(pin)) {
      erro = TEXTOS.pinFaltando;
      return;
    }
    const enviado = comPin ? pin : null;
    pin = '';
    ocupado = true;
    try {
      const r = await chamar('responder_aprovacao', { id, decisao, pin: enviado });
      erro = mensagemDoResultado(r?.resultado);
    } catch (e) {
      erro = e.mensagem;
    } finally {
      ocupado = false;
    }
    await consultar();
  }

  function aoTeclar(e) {
    // V5.19: Esc nega. Enter não tem atalho: só o botão com foco responde.
    if (e.key === 'Escape') {
      e.preventDefault();
      responder('negar');
    }
  }

  // Só a exibição: um segundo a menos; no zero, pergunta de novo (o pedido expirado já
  // saiu do lado Rust).
  function contar() {
    if (!pedido || bloqueado) return;
    if (pedido.restante > 0) pedido.restante -= 1;
    if (pedido.restante === 0) consultar();
  }

  onMount(async () => {
    relogio = setInterval(contar, 1000);
    await consultar();
    const parar = await ouvirAvisos(consultar, 'pedido-de-aprovacao');
    if (saiu) parar();
    else pararDeOuvir = parar;
  });

  onDestroy(() => {
    saiu = true;
    if (relogio) clearInterval(relogio);
    if (pararDeOuvir) pararDeOuvir();
    pin = '';
  });
</script>

<svelte:window onkeydown={aoTeclar} />

<main class="fundo">
  {#if pedido}
    <section class="janela" aria-labelledby="aprovacao-titulo">
      <header class="cabecalho">
        <img class="simbolo" src={simbolo} alt="" />
        <div class="titulos">
          <h1 id="aprovacao-titulo">{TEXTOS.titulo}</h1>
          <p class="subtitulo">{TEXTOS.subtitulo}</p>
        </div>
        {#if !bloqueado}<Contagem segundos={pedido.restante} {urgente} />{/if}
        <BotaoIcone icone="fechar" rotulo={TEXTOS.fecharENegar} onclick={() => responder('negar')} />
      </header>

      {#if bloqueado}
        <Destaque tipo="perigo" icone="cadeado">{TEXTOS.bloqueadoDestaque}</Destaque>
      {:else}
        {#if prod}<Destaque tipo="prod">{TEXTOS.prodDestaque}</Destaque>{/if}
        {#if urgente}
          <Destaque tipo="perigo" icone="relogio">{TEXTOS.expirando(pedido.restante)}</Destaque>
        {/if}
      {/if}

      <p class="resumo">{bloqueado ? TEXTOS.bloqueadoResumo : TEXTOS.resumo}</p>

      <div class="pedido" class:prod>
        {#each pedido.entradas as entrada, i (i)}
          <div class="entrada">
            <span class="avatar"><Icone nome={tipoDe(entrada.tipo ?? 'generico').icone} /></span>
            <div class="textos">
              <span class="nome">{entrada.nome}</span>
              <span class="selos">
                {#if entrada.tipo}<BadgeTipo tipo={entrada.tipo} />{/if}
                <BadgeAmbiente ambiente={pedido.ambiente} />
              </span>
            </div>
            {#if i === 0}
              <div class="nivel">
                <span class="rotulo-nivel">{TEXTOS.nivel}</span>
                <span class="valor-nivel">{TEXTOS.nivelValor}</span>
              </div>
            {/if}
          </div>
        {/each}
        <CaixaDeCodigo rotulo={TEXTOS.comando}>{pedido.comando}</CaixaDeCodigo>
        <!-- A seta é só desenho (outra cor); a que vier num nome já chega como escape. -->
        <CaixaDeCodigo rotulo={TEXTOS.quemPediu}
          >{#if pedido.cadeia.length === 0}{TEXTOS.cadeiaVazia}{/if}{#each pedido.cadeia as processo, i (i)}{#if i > 0}<span
                class="seta"
                aria-hidden="true">{SEPARADOR}</span
              >{/if}<span class="processo">{processo}</span>{/each}</CaixaDeCodigo
        >
      </div>

      <CampoPin
        id="aprovacao-pin"
        rotulo={TEXTOS.pin}
        bind:valor={pin}
        ajuda={TEXTOS.pinAjuda}
        {erro}
        {bloqueado}
      />

      <div class="acoes">
        <div class="linha">
          <Botao id={ID_NEGAR} variante="secundario" desabilitado={ocupado} onclick={() => responder('negar')}>
            {TEXTOS.negar}
          </Botao>
          <Botao variante="primario" desabilitado={ocupado || bloqueado} onclick={() => responder('uma_vez')}>
            {TEXTOS.umaVez}
          </Botao>
        </div>
        <div class="linha">
          <Botao variante="secundario" desabilitado={ocupado || bloqueado} onclick={() => responder('uma_hora')}>
            {TEXTOS.umaHora}
          </Botao>
          <Botao variante="secundario" desabilitado={ocupado || bloqueado} onclick={() => responder('sempre')}>
            {TEXTOS.sempre}
          </Botao>
        </div>
      </div>

      <Destaque icone="alerta">{bloqueado ? TEXTOS.bloqueadoNota : TEXTOS.nota}</Destaque>
      {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    </section>
  {:else if carregado}
    <section class="janela vazia">
      <p class="resumo">{falha || TEXTOS.semPedido}</p>
    </section>
  {/if}
</main>

<style>
  .fundo {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    min-height: 100%;
    background: var(--cor-fundo-padrao);
  }
  .janela {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
    width: var(--largura-dialogo-largo);
    max-width: 100%;
    padding: var(--espaco-24);
    border-radius: var(--raio-janela);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-padrao);
    box-shadow: var(--sombra-modal);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  .simbolo {
    flex: none;
    width: var(--controle-sm);
    height: var(--controle-sm);
  }
  .titulos {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--espaco-2);
    min-width: 0;
  }
  h1 {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
    color: var(--cor-texto-primario);
  }
  .subtitulo {
    margin: var(--espaco-0);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .resumo {
    margin: var(--espaco-0);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    color: var(--cor-texto-secundario);
  }
  .pedido {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-padrao);
  }
  .pedido.prod {
    border-color: var(--cor-ambiente-prod-borda);
  }
  .entrada {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  .avatar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--controle-md);
    height: var(--controle-md);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .textos {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--espaco-6);
    min-width: 0;
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
    color: var(--cor-texto-primario);
    overflow-wrap: anywhere;
  }
  .selos {
    display: flex;
    align-items: center;
    gap: var(--espaco-6);
  }
  .nivel {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--espaco-2);
  }
  .rotulo-nivel {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .valor-nivel {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-primario);
  }
  .seta {
    color: var(--cor-texto-secundario);
  }
  .acoes {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
  }
  .linha {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--espaco-12);
  }
  .linha :global(.botao) {
    width: 100%;
  }
  .falha {
    margin: var(--espaco-0);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-perigo-texto);
  }
</style>
