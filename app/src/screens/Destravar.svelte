<script>
  // Telas `Destravar / Padrão` (3342), `/ Senha errada` (3393) e `/ Pedido do agente`
  // (3450). Janela com o `Painel de marca` e o painel `conteúdo` (fundo/sutil, borda
  // borda/sutil, raio painel, padding espaco/48) com a coluna `formulário` centrada
  // (largura/coluna-acesso; gap espaco/32, ou espaco/24 com o pedido).
  //
  // Segurança (Fase 31; S1, S11, S13, D20):
  // - a senha fica só em `senha` (estado local desta tela) e sai dele no clique, antes
  //   da resposta; nunca vai a estado global, armazenamento do navegador ou URL. Senha
  //   errada esvazia o campo e mantém o foco nele (V5.8);
  // - a espera de D20 é do daemon, antes de olhar a senha. A contagem do botão ("Tente
  //   de novo em N s") é a mesma regra (dobra a partir de 1 s, até 30 s), contada aqui só
  //   com os erros desta tela: a resposta não diz quantas tentativas houve;
  // - o pedido do agente vem do lado Rust (`estado_do_cofre` › `pedido`): id, segundos
  //   que faltam pelo prazo do daemon e (Fase 39a) a entrada pedida, a operação e o
  //   comando exato, já com escape visível (D51: o daemon formata e o Rust do app passa
  //   por `neutralizar`). Vêm do agente: entram só como texto (nunca HTML), isolados da
  //   direção do texto ao redor e sem ligaduras. Sem tipo nem ambiente: com o cofre
  //   travado o daemon não os sabe, então o cartão não tem os selos do frame 3450.
  import { onDestroy, onMount, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import CodeBox from '../lib/components/CodeBox.svelte';
  import AgentText from '../lib/components/AgentText.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import BrandPanel from '../lib/components/BrandPanel.svelte';
  import { call } from '../lib/bridge/index.js';

  /** Espera máxima de D20, em segundos. */
  const ESPERA_MAXIMA = 30;

  /** Como o agente pediu, pela operação do protocolo (`Operacao::nome`). */
  const COMANDO_DA_OPERACAO = Object.freeze({
    executar: 'ward run',
    ssh: 'ward ssh',
    login: 'ward login',
    playwright: 'ward playwright',
  });
  const comandoDaOperacao = (op) =>
    Object.hasOwn(COMANDO_DA_OPERACAO, op) ? COMANDO_DA_OPERACAO[op] : '';

  /**
   * @type {{
   *   request?: {
   *     id: number,
   *     restante: number,
   *     entrada: string,
   *     operacao: string,
   *     comando: string
   *   } | null,
   *   onunlocked: () => void,
   *   onerror?: (mensagem: string) => void
   * }}
   */
  let { request: pedido = null, onunlocked: ondestravado, onerror: onerro = undefined } = $props();

  let senha = $state('');
  let erro = $state('');
  let errosSeguidos = $state(0);
  let espera = $state(0);
  let enviando = $state(false);
  /** @type {HTMLInputElement | null} */
  let entrada = $state(null);

  // Pedido do agente: contagem local a partir do `restante` que o Rust mandou; "Agora
  // não" esconde só aquele pedido (pelo id).
  let restante = $state(0);
  let pedidoId = $state(-1);
  let dispensado = $state(-1);
  $effect(() => {
    if (pedido && pedido.id !== pedidoId) {
      pedidoId = pedido.id;
      restante = pedido.restante;
    } else if (pedido && pedido.id === pedidoId && Math.abs(pedido.restante - restante) > 2) {
      restante = pedido.restante;
    }
  });
  const comPedido = $derived(pedido !== null && restante > 0 && dispensado !== pedidoId);

  let relogio = null;
  onMount(() => {
    relogio = setInterval(() => {
      if (espera > 0) espera -= 1;
      if (restante > 0) restante -= 1;
    }, 1000);
    entrada?.focus();
  });
  onDestroy(() => {
    clearInterval(relogio);
    senha = '';
  });

  function contagem(s) {
    const m = Math.floor(s / 60);
    const ss = String(s % 60).padStart(2, '0');
    return `${m}:${ss} restantes`;
  }

  async function destravar(ev) {
    ev?.preventDefault();
    if (enviando || espera > 0) return;
    if (!senha) {
      erro = 'Digite a senha mestra.';
      entrada?.focus();
      return;
    }
    enviando = true;
    // A senha sai do estado da tela antes da resposta.
    const enviada = senha;
    senha = '';
    try {
      await call('destravar', { senha: enviada });
      erro = '';
      errosSeguidos = 0;
      ondestravado();
    } catch (e) {
      if (e.codigo === 'senha-incorreta') {
        errosSeguidos += 1;
        espera = Math.min(2 ** (errosSeguidos - 1), ESPERA_MAXIMA);
        erro = e.mensagem;
      } else if (e.codigo === 'travado' || e.codigo === 'pedido-invalido') {
        erro = e.codigo === 'pedido-invalido' ? 'Senha mestra fora do formato.' : e.mensagem;
      } else {
        onerro?.(e.mensagem);
      }
    } finally {
      enviando = false;
      await tick();
      entrada?.focus();
    }
  }

  function digitou() {
    if (erro && espera === 0) erro = '';
  }

  const rotuloBotao = $derived(
    enviando ? 'Destravando…' : espera > 0 ? `Tente de novo em ${espera} s` : 'Destravar',
  );
</script>

<div class="janela">
  <BrandPanel />
  <main class="conteudo">
    <form class="formulario" class:com-pedido={comPedido} onsubmit={destravar} novalidate>
      <header class="cabecalho">
        {#if !comPedido}
          <div class="selo"><Icon name="lock" size="lg" tone="brand" /></div>
        {/if}
        <h1>Cofre travado</h1>
        <p class="descricao">
          {comPedido
            ? 'Um agente está esperando uma entrada. Destrave para continuar.'
            : 'Digite a senha mestra para destravar o Ward neste computador.'}
        </p>
      </header>

      {#if comPedido}
        <section class="pedido" aria-label="Pedido do agente">
          <div class="topo">
            <span class="rotulo-pedido">Pedido do agente</span>
            <span class="contagem" role="timer" aria-live="off">
              <Icon name="clock" size="xs" tone="warning" />{contagem(restante)}
            </span>
          </div>
          <div class="entrada">
            <span class="avatar"><Icon name="key" /></span>
            <div class="textos">
              <span class="nome"><AgentText text={pedido.entrada} /></span>
              <span class="operacao">{comandoDaOperacao(pedido.operacao)}</span>
            </div>
          </div>
          {#if pedido.comando}
            <CodeBox label="Comando"><AgentText text={pedido.comando} /></CodeBox>
          {/if}
        </section>
      {/if}

      <PasswordField
        id="senha-mestra"
        label="Senha mestra"
        placeholder="Digite a senha mestra"
        help="Só a senha mestra destrava. O PIN não serve aqui."
        bind:value={senha}
        bind:input={entrada}
        error={erro}
        readonly={enviando}
        autocomplete="off"
        oninput={digitou}
      />

      {#if comPedido}
        <div class="acoes">
          <Button variant="secondary" onclick={() => (dispensado = pedidoId)}>Agora não</Button>
          <Button type="submit" disabled={enviando || espera > 0}>{rotuloBotao}</Button>
        </div>
      {:else}
        <Button type="submit" disabled={enviando || espera > 0}>{rotuloBotao}</Button>
      {/if}

      {#if comPedido}
        <Highlight icon="alert">
          Destravar não aprova o pedido. Se a entrada pedir aprovação, a janela de aprovação
          abre em seguida, com PIN. “Agora não”, fechar ou deixar expirar nega o pedido.
        </Highlight>
      {:else if errosSeguidos > 0}
        <Highlight icon="clock">
          Cada tentativa errada aumenta a espera da próxima, até 30 s. O Ward não mostra
          quantas tentativas já houve.
        </Highlight>
      {:else}
        <Highlight icon="clock">
          O cofre trava sozinho quando você bloqueia a tela, suspende o computador ou fica 4 h
          sem usar.
        </Highlight>
      {/if}
    </form>
  </main>
</div>

<style>
  .janela {
    display: flex;
    gap: var(--space-16);
    height: 100%;
    padding: var(--space-16);
    background: var(--color-bg-default);
  }
  .conteudo {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: var(--space-48);
    overflow-y: auto;
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: var(--space-32);
    width: var(--width-column-access);
    margin: auto;
  }
  .formulario.com-pedido {
    gap: var(--space-24);
  }
  .formulario > :global(.button) {
    width: 100%;
  }
  .cabecalho {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  .selo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-primary-subtle);
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .descricao {
    margin: var(--space-0);
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
  .topo {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .rotulo-pedido {
    font-weight: var(--weight-medium);
  }
  .contagem {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-8) var(--space-2) var(--space-6);
    border-radius: var(--radius-pill);
    background: var(--color-warning-subtle);
    color: var(--color-warning-text);
    font-weight: var(--weight-medium);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    font-variant-numeric: tabular-nums;
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
    color: var(--color-text-secondary);
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
    /* D51: o nome vem do agente; cada caractere como é, isolado da direção ao redor. */
    font-variant-ligatures: none;
    font-feature-settings: 'liga' 0, 'calt' 0;
    direction: ltr;
    unicode-bidi: isolate;
  }
  .operacao {
    font-family: var(--font-family-mono);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .acoes {
    display: flex;
    gap: var(--space-12);
  }
  .acoes > :global(.button) {
    flex: 1;
  }
</style>
