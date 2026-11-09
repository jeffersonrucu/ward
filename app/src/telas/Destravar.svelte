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
  // - o pedido do agente vem do lado Rust (`estado_do_cofre` › `pedido`): só um id e os
  //   segundos que faltam. O aviso do protocolo não traz entrada nem comando.
  import { onDestroy, onMount, tick } from 'svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import CampoSenha from '../lib/componentes/CampoSenha.svelte';
  import Destaque from '../lib/componentes/Destaque.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import PainelDeMarca from '../lib/componentes/PainelDeMarca.svelte';
  import { chamar } from '../lib/ponte/index.js';

  /** Espera máxima de D20, em segundos. */
  const ESPERA_MAXIMA = 30;

  /**
   * @type {{
   *   pedido?: { id: number, restante: number } | null,
   *   ondestravado: () => void,
   *   onerro?: (mensagem: string) => void
   * }}
   */
  let { pedido = null, ondestravado, onerro = undefined } = $props();

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
      await chamar('destravar', { senha: enviada });
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
  <PainelDeMarca />
  <main class="conteudo">
    <form class="formulario" class:com-pedido={comPedido} onsubmit={destravar} novalidate>
      <header class="cabecalho">
        {#if !comPedido}
          <div class="selo"><Icone nome="cadeado" tamanho="lg" tom="marca" /></div>
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
              <Icone nome="relogio" tamanho="xs" tom="aviso" />{contagem(restante)}
            </span>
          </div>
          <div class="sem-detalhe">
            <span class="rotulo-detalhe">Pedido</span>
            <p>
              Um comando do agente espera o cofre destravar. Ele segue sozinho quando você
              destravar; se não, é negado quando a contagem acabar.
            </p>
          </div>
        </section>
      {/if}

      <CampoSenha
        id="senha-mestra"
        rotulo="Senha mestra"
        placeholder="Digite a senha mestra"
        ajuda="Só a senha mestra destrava. O PIN não serve aqui."
        bind:valor={senha}
        bind:entrada
        {erro}
        readonly={enviando}
        autocomplete="off"
        oninput={digitou}
      />

      {#if comPedido}
        <div class="acoes">
          <Botao variante="secundario" onclick={() => (dispensado = pedidoId)}>Agora não</Botao>
          <Botao tipo="submit" desabilitado={enviando || espera > 0}>{rotuloBotao}</Botao>
        </div>
      {:else}
        <Botao tipo="submit" desabilitado={enviando || espera > 0}>{rotuloBotao}</Botao>
      {/if}

      {#if comPedido}
        <Destaque icone="alerta">
          Destravar não aprova o pedido. Se a entrada pedir aprovação, a janela de aprovação
          abre em seguida, com PIN. “Agora não”, fechar ou deixar expirar nega o pedido.
        </Destaque>
      {:else if errosSeguidos > 0}
        <Destaque icone="relogio">
          Cada tentativa errada aumenta a espera da próxima, até 30 s. O Ward não mostra
          quantas tentativas já houve.
        </Destaque>
      {:else}
        <Destaque icone="relogio">
          O cofre trava sozinho quando você bloqueia a tela, suspende o computador ou fica 4 h
          sem usar.
        </Destaque>
      {/if}
    </form>
  </main>
</div>

<style>
  .janela {
    display: flex;
    gap: var(--espaco-16);
    height: 100%;
    padding: var(--espaco-16);
    background: var(--cor-fundo-padrao);
  }
  .conteudo {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: var(--espaco-48);
    overflow-y: auto;
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-32);
    width: var(--largura-coluna-acesso);
    margin: auto;
  }
  .formulario.com-pedido {
    gap: var(--espaco-24);
  }
  .formulario > :global(.botao) {
    width: 100%;
  }
  .cabecalho {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
  }
  .selo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-lg);
    height: var(--controle-lg);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-primaria-sutil);
  }
  h1 {
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-xl);
    line-height: var(--altura-linha-xl);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .descricao {
    margin: var(--espaco-0);
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
  .topo {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .rotulo-pedido {
    font-weight: var(--peso-medio);
  }
  .contagem {
    display: inline-flex;
    align-items: center;
    gap: var(--espaco-4);
    padding: var(--espaco-2) var(--espaco-8) var(--espaco-2) var(--espaco-6);
    border-radius: var(--raio-pilula);
    background: var(--cor-aviso-sutil);
    color: var(--cor-aviso-texto);
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    font-variant-numeric: tabular-nums;
  }
  .sem-detalhe {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    padding: var(--espaco-16);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-padrao);
  }
  .rotulo-detalhe {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .sem-detalhe p {
    margin: var(--espaco-0);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .acoes {
    display: flex;
    gap: var(--espaco-12);
  }
  .acoes > :global(.botao) {
    flex: 1;
  }
</style>
