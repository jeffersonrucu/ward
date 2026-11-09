<script>
  // `Diálogo de nível` (set 967): Escolher (968), PIN (1027), PIN errado (1078). Largura
  // largura/dialogo-largo, `hug`, raio janela, padding espaco/24, gap espaco/20, fundo
  // superficie/elevada, borda borda/padrao, Sombra/Modal.
  // - `cabeçalho` (gap espaco/12): título (Título/Modal) e fechar 32 px (controle/sm,
  //   raio sm) com `Ícone/fechar`;
  // - `entrada` (padding espaco/12, gap espaco/12, raio cartao, fundo superficie/padrao,
  //   borda borda/sutil): `Ícone/chave`, nome (Texto/Corpo forte), `Badge tipo` e
  //   `Badge ambiente`;
  // - Escolher: `corpo` gap espaco/12 com a explicação, as três `Opção de nível` e
  //   "Nível atual: …" (Texto/Legenda); rodapé "Cancelar" e "Continuar";
  // - PIN: `corpo` gap espaco/20 com "De … para …" (gap espaco/8; o novo em texto/marca),
  //   `Campo PIN` e a nota (5 erros travam); rodapé "Voltar" e "Confirmar". No PIN
  //   errado, o erro no campo e a nota em aviso/texto ("O nível continua …").
  //
  // S9: o PIN fica só no estado local deste componente, sai dele antes da resposta do
  // pedido (o campo volta vazio) e some ao fechar. Nada vai para armazenamento, URL ou
  // log. O 5º erro seguido trava o cofre no daemon: depois de cada PIN errado o diálogo
  // consulta o estado e, travado, chama `ontravado`.
  import { tick } from 'svelte';
  import BadgeAmbiente from './BadgeAmbiente.svelte';
  import BadgeTipo from './BadgeTipo.svelte';
  import Botao from './Botao.svelte';
  import CampoPin from './CampoPin.svelte';
  import Icone from './Icone.svelte';
  import OpcaoDeNivel from './OpcaoDeNivel.svelte';
  import { ORDEM_DOS_NIVEIS, nivelDe } from './niveis.js';
  import { chamar } from '../ponte/index.js';

  /**
   * @typedef {{ nome: string, tipo: string, ambiente: 'dev' | 'stage' | 'prod', nivel: string }} Entrada
   * @type {{
   *   aberto?: boolean, entrada: Entrada | null,
   *   onfechar?: () => void, onmudado?: (nivel: string) => void, ontravado?: () => void
   * }}
   */
  let {
    aberto = false,
    entrada,
    onfechar = undefined,
    onmudado = undefined,
    ontravado = undefined,
  } = $props();

  const ID_PIN = 'nivel-pin';

  let dialogo = $state();
  /** 'escolher' | 'pin' */
  let etapa = $state('escolher');
  let escolhido = $state('');
  let pin = $state('');
  let pinErrado = $state(false);
  let erro = $state('');
  let enviando = $state(false);

  const atual = $derived(nivelDe(entrada?.nivel) ?? nivelDe('com-aprovacao'));
  const novo = $derived(nivelDe(escolhido));

  function reiniciar() {
    etapa = 'escolher';
    escolhido = entrada?.nivel ?? '';
    pin = '';
    pinErrado = false;
    erro = '';
    enviando = false;
  }

  $effect(() => {
    if (!dialogo) return;
    if (aberto && !dialogo.open) {
      reiniciar();
      dialogo.showModal();
    }
    if (!aberto && dialogo.open) dialogo.close();
  });

  function fechar() {
    pin = '';
    onfechar?.();
  }

  async function continuar() {
    if (!novo || escolhido === entrada?.nivel) return;
    etapa = 'pin';
    pin = '';
    pinErrado = false;
    erro = '';
    await tick();
    document.getElementById(ID_PIN)?.focus();
  }

  function voltar() {
    pin = '';
    pinErrado = false;
    erro = '';
    etapa = 'escolher';
  }

  async function confirmar() {
    if (enviando || pin.length < 4 || !entrada || !novo) return;
    // O PIN sai do estado antes da resposta: a tela não o guarda nem em caso de erro.
    const args = { entrada: entrada.nome, nivel: escolhido, pin };
    pin = '';
    enviando = true;
    erro = '';
    try {
      const r = await chamar('mudar_nivel', args);
      args.pin = '';
      enviando = false;
      onmudado?.(r.nivel);
      return;
    } catch (e) {
      args.pin = '';
      enviando = false;
      if (e.codigo === 'pin-incorreto') {
        pinErrado = true;
        if (await travou()) return;
      } else if (e.codigo === 'travado') {
        ontravado?.();
        return;
      } else {
        pinErrado = false;
        erro = e.mensagem;
      }
    }
    await tick();
    document.getElementById(ID_PIN)?.focus();
  }

  /** Depois de um PIN errado: o 5º seguido trava o cofre (o daemon conta). */
  async function travou() {
    try {
      const r = await chamar('estado_do_cofre');
      if (r?.estado === 'destravado') return false;
    } catch {
      // Sem resposta, trata como travado: a tela de destravar consulta de novo.
    }
    ontravado?.();
    return true;
  }
</script>

<dialog
  bind:this={dialogo}
  class="dialogo"
  aria-labelledby="nivel-titulo"
  data-etapa={pinErrado ? 'pin-errado' : etapa}
  onclose={fechar}
>
  <div class="cabecalho">
    <h2 id="nivel-titulo">{etapa === 'escolher' ? 'Nível de acesso' : 'Confirmar com o PIN'}</h2>
    <button class="fechar" type="button" aria-label="Fechar" onclick={fechar}>
      <Icone nome="fechar" />
    </button>
  </div>

  {#if entrada}
    <div class="entrada">
      <Icone nome="chave" />
      <span class="nome-entrada">{entrada.nome}</span>
      <BadgeTipo tipo={entrada.tipo} />
      <BadgeAmbiente ambiente={entrada.ambiente} />
    </div>
  {/if}

  {#if etapa === 'escolher'}
    <div class="corpo escolher" role="radiogroup" aria-labelledby="nivel-titulo">
      <p class="explicacao">
        O ambiente não muda as regras: dev, stage e prod têm os mesmos três níveis. Mudar o
        nível pede o PIN.
      </p>
      {#each ORDEM_DOS_NIVEIS as n (n)}
        <OpcaoDeNivel
          nivel={n}
          nome="nivel-de-acesso"
          marcado={escolhido === n}
          onescolher={(v) => (escolhido = v)}
        />
      {/each}
      <p class="atual">Nível atual: {atual.minusculo}.</p>
    </div>
    <div class="rodape">
      <Botao variante="secundario" onclick={fechar}>Cancelar</Botao>
      <Botao
        variante="primario"
        desabilitado={!novo || escolhido === entrada?.nivel}
        onclick={continuar}>Continuar</Botao
      >
    </div>
  {:else}
    <form
      class="corpo pin"
      onsubmit={(e) => {
        e.preventDefault();
        confirmar();
      }}
    >
      <p class="mudanca">
        <span class="fraco">De</span>
        <span class="forte">{atual.minusculo}</span>
        <span class="fraco">para</span>
        <span class="forte marca">{novo?.minusculo}</span>
      </p>
      <CampoPin
        rotulo="PIN"
        id={ID_PIN}
        bind:valor={pin}
        ajuda="O PIN confirma a mudança. Ele não destrava o cofre."
        erro={pinErrado ? 'PIN incorreto. Nada foi alterado.' : erro}
      />
      {#if pinErrado}
        <p class="nota aviso" role="alert">
          O nível continua {atual.minusculo}. 5 erros de PIN seguidos travam o cofre.
        </p>
      {:else}
        <p class="nota">5 erros de PIN seguidos travam o cofre.</p>
      {/if}
    </form>
    <div class="rodape">
      <Botao variante="secundario" onclick={voltar}>Voltar</Botao>
      <Botao variante="primario" desabilitado={enviando || pin.length < 4} onclick={confirmar}
        >Confirmar</Botao
      >
    </div>
  {/if}
</dialog>

<style>
  .dialogo {
    width: var(--largura-dialogo-largo);
    max-width: calc(100vw - 2 * var(--espaco-24));
    padding: var(--espaco-24);
    border-radius: var(--raio-janela);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    color: var(--cor-texto-primario);
    box-shadow: var(--sombra-modal);
  }
  .dialogo[open] {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
  }
  .dialogo::backdrop {
    background: var(--cor-fundo-sobreposicao);
  }
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
  }
  h2 {
    flex: 1;
    margin: var(--espaco-0);
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
    letter-spacing: var(--espacamento-letra-titulo);
  }
  .fechar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-sm);
    height: var(--controle-sm);
    padding: var(--espaco-0);
    border: none;
    border-radius: var(--raio-sm);
    background: transparent;
    cursor: pointer;
  }
  .entrada {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    padding: var(--espaco-12);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .nome-entrada {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    margin: var(--espaco-0);
  }
  .corpo.escolher {
    gap: var(--espaco-12);
  }
  .corpo.pin {
    gap: var(--espaco-20);
  }
  p {
    margin: var(--espaco-0);
  }
  .explicacao,
  .nota {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .nota.aviso {
    color: var(--cor-aviso-texto);
  }
  .atual {
    font-size: var(--tamanho-xs);
    line-height: var(--altura-linha-xs);
    color: var(--cor-texto-secundario);
  }
  .mudanca {
    display: flex;
    align-items: center;
    gap: var(--espaco-8);
  }
  .fraco {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
    color: var(--cor-texto-secundario);
  }
  .forte {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .marca {
    color: var(--cor-texto-marca);
  }
  .rodape {
    display: flex;
    justify-content: flex-end;
    gap: var(--espaco-12);
    padding-top: var(--espaco-4);
  }
</style>
