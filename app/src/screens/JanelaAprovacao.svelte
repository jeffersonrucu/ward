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
  // - Fase 54b (D68): com `entrada_oculta` (a entrada padrão do comando é pipe ou
  //   arquivo), aviso em `Destaque` aviso e um `Interruptor` que libera os três
  //   "Permitir"; marcado, a resposta vai com `ciente`. Quem confere é o lado Rust e o
  //   daemon; a caixa desmarca a cada pedido novo. Frames 22964 (desmarcada) e 23051
  //   (marcada) no FigCraft (Fase 54f).
  import { onDestroy, onMount, tick } from 'svelte';
  import EnvironmentBadge from '../lib/components/EnvironmentBadge.svelte';
  import TypeBadge from '../lib/components/TypeBadge.svelte';
  import Button from '../lib/components/Button.svelte';
  import IconButton from '../lib/components/IconButton.svelte';
  import CodeBox from '../lib/components/CodeBox.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import Countdown from '../lib/components/Countdown.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import Switch from '../lib/components/Switch.svelte';
  import AgentText from '../lib/components/AgentText.svelte';
  import { entryType } from '../lib/components/types.js';
  import simbolo from '../lib/components/ward-symbol.svg';
  import { call, listenNotices } from '../lib/bridge/index.js';
  import {
    ID_NEGAR,
    SEGUNDOS_URGENTE,
    SEPARADOR,
    TEXTOS,
    avisoDaOrigem,
    mensagemDoResultado,
    pedePin,
    pinSuficiente,
  } from './aprovacao.js';

  let pedido = $state(null);
  let carregado = $state(false);
  let pin = $state('');
  let erro = $state('');
  let ocupado = $state(false);
  let falha = $state('');
  let ciente = $state(false);

  const bloqueado = $derived(pedido?.estado === 'bloqueado');
  const prod = $derived(pedido?.ambiente === 'prod');
  // Fase 44c (D9): só a confirmação da origem `http` (sem PIN, "Negar" ou "Confirmar").
  const confirmacao = $derived(pedido?.confirmacao === true);
  const popup = $derived(pedido?.operacao === 'preencher');
  // Fase 54b: a entrada padrão que esta janela não mostra.
  const entradaOculta = $derived(pedido?.entrada_oculta === true);
  const semCiencia = $derived(entradaOculta && !ciente);
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
      const novo = await call('pedido_de_aprovacao');
      const outro = (novo?.id ?? null) !== (pedido?.id ?? null);
      pedido = novo;
      falha = '';
      if (outro) {
        pin = '';
        erro = '';
        ciente = false;
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
    const comPin = pedePin(decisao);
    if (comPin && !pinSuficiente(pin)) {
      erro = TEXTOS.pinFaltando;
      return;
    }
    if (comPin && semCiencia) {
      erro = TEXTOS.entradaFaltando;
      return;
    }
    const enviado = comPin ? pin : null;
    const comCiencia = comPin && entradaOculta && ciente;
    pin = '';
    ocupado = true;
    try {
      const r = await call('responder_aprovacao', {
        id,
        decisao,
        pin: enviado,
        ciente: comCiencia,
      });
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
    const parar = await listenNotices(consultar, 'pedido-de-aprovacao');
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
          <p class="subtitulo">{popup ? TEXTOS.subtituloPopup : TEXTOS.subtitulo}</p>
        </div>
        {#if !bloqueado}<Countdown seconds={pedido.restante} urgent={urgente} />{/if}
        <IconButton icon="close" label={TEXTOS.fecharENegar} onclick={() => responder('negar')} />
      </header>

      {#if bloqueado}
        <Highlight kind="danger" icon="lock">{TEXTOS.bloqueadoDestaque}</Highlight>
      {:else}
        {#if prod}<Highlight kind="prod"
            >{confirmacao ? TEXTOS.prodDestaqueOrigem : TEXTOS.prodDestaque}</Highlight
          >{/if}
        <!-- Fase 44e: o aviso da origem em `Destaque` aviso (frames 22812, 22957). -->
        {#if pedido.origem}<Highlight kind="warning" icon="alert">{avisoDaOrigem(pedido.origem)}</Highlight>{/if}
        <!-- Fase 54b (D68): a entrada padrão que a janela não mostra (frames 22964, 23051). -->
        {#if entradaOculta}<Highlight kind="warning" icon="alert">{TEXTOS.entradaDestaque}</Highlight>{/if}
        {#if urgente}
          <Highlight kind="danger" icon="clock">{TEXTOS.expirando(pedido.restante)}</Highlight>
        {/if}
      {/if}

      <p class="resumo">
        {bloqueado
          ? TEXTOS.bloqueadoResumo
          : confirmacao
            ? TEXTOS.resumoConfirmacao
            : pedido.origem
              ? TEXTOS.resumoOrigem
              : TEXTOS.resumo}
      </p>

      <div class="pedido" class:prod>
        {#each pedido.entradas as entrada, i (i)}
          <div class="entrada">
            <span class="avatar"><Icon name={entryType(entrada.tipo ?? 'generico').icon} /></span>
            <div class="textos">
              <span class="nome"><AgentText text={entrada.nome} /></span>
              <span class="selos">
                {#if entrada.tipo}<TypeBadge kind={entrada.tipo} />{/if}
                <EnvironmentBadge environment={pedido.ambiente} />
              </span>
            </div>
            {#if i === 0}
              <div class="nivel">
                <span class="rotulo-nivel">{TEXTOS.nivel}</span>
                <span class="valor-nivel">{confirmacao ? TEXTOS.nivelConfirmacao : TEXTOS.nivelValor}</span>
              </div>
            {/if}
          </div>
        {/each}
        <!-- Fase 44c: a origem `http` para onde vão usuário e senha (o `login`, o
             `playwright` e o popup não têm comando). -->
        {#if pedido.origem}
          <CodeBox label={TEXTOS.origem}><AgentText text={pedido.origem} /></CodeBox>
        {/if}
        {#if pedido.comando || !pedido.origem}
          <CodeBox label={TEXTOS.comando}><AgentText text={pedido.comando} /></CodeBox>
        {/if}
        <!-- Fase 26a: o comando roda no diretório da CLI; só o `executar` tem um
             (frames 16701, 16810, 16885, 16960, 17035, 22964; Fase 54i). -->
        {#if pedido.diretorio}
          <CodeBox label={TEXTOS.diretorio}><AgentText text={pedido.diretorio} /></CodeBox>
        {/if}
        <!-- A seta é só desenho (outra cor); a que vier num nome já chega como escape. -->
        <CodeBox label={TEXTOS.quemPediu}
          >{#if pedido.cadeia.length === 0}{TEXTOS.cadeiaVazia}{/if}{#each pedido.cadeia as processo, i (i)}{#if i > 0}<span
                class="seta"
                aria-hidden="true">{SEPARADOR}</span
              >{/if}<span class="processo"><AgentText text={processo} /></span>{/each}</CodeBox
        >
      </div>

      {#if confirmacao}
        {#if erro}<p class="falha" role="alert">{erro}</p>{/if}
        <div class="acoes">
          <div class="linha">
            <Button id={ID_NEGAR} variant="secondary" disabled={ocupado} onclick={() => responder('negar')}>
              {TEXTOS.negar}
            </Button>
            <Button variant="primary" disabled={ocupado || bloqueado} onclick={() => responder('confirmar')}>
              {TEXTOS.confirmar}
            </Button>
          </div>
        </div>
      {:else}
        {#if entradaOculta && !bloqueado}
          <Switch
            id="aprovacao-entrada"
            label={TEXTOS.entradaCiente}
            on={ciente}
            disabled={ocupado}
            onchange={(v) => (ciente = v)}
          />
        {/if}
        <PinField
          id="aprovacao-pin"
          label={TEXTOS.pin}
          bind:value={pin}
          help={TEXTOS.pinAjuda}
          error={erro}
          blocked={bloqueado}
        />

        <div class="acoes">
          <div class="linha">
            <Button id={ID_NEGAR} variant="secondary" disabled={ocupado} onclick={() => responder('negar')}>
              {TEXTOS.negar}
            </Button>
            <Button variant="primary" disabled={ocupado || bloqueado || semCiencia} onclick={() => responder('uma_vez')}>
              {TEXTOS.umaVez}
            </Button>
          </div>
          <div class="linha">
            <Button variant="secondary" disabled={ocupado || bloqueado || semCiencia} onclick={() => responder('uma_hora')}>
              {TEXTOS.umaHora}
            </Button>
            <Button variant="secondary" disabled={ocupado || bloqueado || semCiencia} onclick={() => responder('sempre')}>
              {TEXTOS.sempre}
            </Button>
          </div>
        </div>
      {/if}

      <Highlight icon="alert"
        >{bloqueado ? TEXTOS.bloqueadoNota : confirmacao ? TEXTOS.notaConfirmacao : TEXTOS.nota}</Highlight
      >
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
    background: var(--color-bg-default);
  }
  .janela {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    width: var(--width-wide-dialog);
    max-width: 100%;
    padding: var(--space-24);
    border-radius: var(--radius-window);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-default);
    box-shadow: var(--shadow-modal);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .simbolo {
    flex: none;
    width: var(--control-sm);
    height: var(--control-sm);
  }
  .titulos {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
    letter-spacing: var(--letter-spacing-title);
    color: var(--color-text-primary);
  }
  .subtitulo {
    margin: var(--space-0);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .resumo {
    margin: var(--space-0);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-secondary);
  }
  .pedido {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-default);
  }
  .pedido.prod {
    border-color: var(--color-environment-prod-border);
  }
  .entrada {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .avatar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-md);
    height: var(--control-md);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .textos {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-6);
    min-width: 0;
  }
  .nome {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-primary);
    overflow-wrap: anywhere;
    /* D51: da esquerda para a direita, isolado; letras RTL isoladas no TextoDoAgente. */
    direction: ltr;
    unicode-bidi: isolate;
  }
  .selos {
    display: flex;
    align-items: center;
    gap: var(--space-6);
  }
  .nivel {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-2);
  }
  .rotulo-nivel {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .valor-nivel {
    font-weight: var(--weight-medium);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-primary);
  }
  .seta {
    color: var(--color-text-secondary);
  }
  .acoes {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  .linha {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-12);
  }
  .linha :global(.button) {
    width: 100%;
  }
  .falha {
    margin: var(--space-0);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-danger-text);
  }
</style>
