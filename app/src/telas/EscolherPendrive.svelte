<script>
  // Escolha do pendrive do backup externo (Fase 48, V7.9; D17). Sem frame próprio no
  // FigCraft: monta o `Modal` (611) com a lista dos pendrives (opções no desenho do
  // cartão `pendrive` 16318: superfície/padrão, raio item, padding espaco/20; a escolhida
  // com borda/foco) e os `Botão` (263).
  //
  // A lista vem do daemon (`listar_pendrives`): UUID, rótulo (do udisks2, não confiável;
  // chega com o invisível em escape, D51, e aqui é só texto) e se está montado. Nenhum
  // caminho de montagem. Ao confirmar, vai só o UUID escolhido; o lado Rust confere de
  // novo que ele está na lista do daemon antes de gravar.
  import Botao from '../lib/componentes/Botao.svelte';
  import Modal from '../lib/componentes/Modal.svelte';
  import { chamar } from '../lib/ponte/index.js';
  import { nomeDoPendrive, situacaoNaLista } from './configuracoes.js';
  import { mensagemDoRust } from './primeiro-uso.js';

  /**
   * @type {{ aberto?: boolean, trocar?: boolean, atual?: string | null,
   *   onfechar?: () => void, onescolhido?: (externo: object) => void,
   *   ontravado?: () => void }}
   */
  let {
    aberto = false,
    trocar = false,
    atual = null,
    onfechar = undefined,
    onescolhido = undefined,
    ontravado = undefined,
  } = $props();

  let pendrives = $state([]);
  let escolhido = $state('');
  let carregando = $state(false);
  let enviando = $state(false);
  let falha = $state('');

  async function carregar() {
    carregando = true;
    falha = '';
    try {
      const lista = await chamar('listar_pendrives');
      pendrives = Array.isArray(lista) ? lista : [];
      if (!pendrives.some((p) => p.uuid === escolhido)) {
        const igual = pendrives.find((p) => atual && p.uuid.toUpperCase() === atual.toUpperCase());
        escolhido = igual ? igual.uuid : '';
      }
    } catch (e) {
      pendrives = [];
      escolhido = '';
      if (e?.codigo === 'travado') {
        ontravado?.();
        return;
      }
      falha = mensagemDoRust(e?.mensagem) || 'Não deu para ver os pendrives.';
    } finally {
      carregando = false;
    }
  }

  $effect(() => {
    if (aberto) {
      escolhido = '';
      pendrives = [];
      carregar();
    }
  });

  async function confirmar() {
    if (!escolhido || enviando) return;
    enviando = true;
    falha = '';
    try {
      const externo = await chamar('definir_backup_externo', { ligado: true, uuid: escolhido });
      onescolhido?.(externo);
    } catch (e) {
      if (e?.codigo === 'travado') {
        ontravado?.();
        return;
      }
      const msg = mensagemDoRust(e?.mensagem) || 'Não foi possível ligar o backup externo.';
      // O pendrive saiu (ou o pedido não bateu com a lista): a lista volta atualizada.
      if (e?.codigo === 'pendrive-nao-conectado') await carregar();
      falha = msg;
    } finally {
      enviando = false;
    }
  }
</script>

<Modal titulo={trocar ? 'Trocar pendrive' : 'Escolher pendrive'} {aberto} onfechar={() => onfechar?.()}>
  <div class="conteudo">
    <p class="explicacao">
      O Ward copia as versões cifradas para a pasta ward-backup/ do pendrive escolhido sempre que
      ele estiver conectado e montado. Ele é reconhecido pelo UUID, não pelo nome.
    </p>
    {#if carregando && pendrives.length === 0}
      <p class="vazio" role="status">Procurando pendrives…</p>
    {:else if pendrives.length === 0 && !falha}
      <p class="vazio" role="status">
        Nenhum pendrive conectado. Conecte o pendrive e toque em Atualizar.
      </p>
    {:else if pendrives.length > 0}
      <div class="lista" role="radiogroup" aria-label="Pendrives conectados">
        {#each pendrives as p (p.uuid)}
          <label class="opcao" class:escolhida={escolhido === p.uuid}>
            <input
              class="visualmente-oculto"
              type="radio"
              name="pendrive"
              value={p.uuid}
              checked={escolhido === p.uuid}
              onchange={() => (escolhido = p.uuid)}
            />
            <span class="textos">
              <span class="nome"><bdi>{nomeDoPendrive(p)}</bdi></span>
              <span class="detalhe">UUID <bdi class="uuid">{p.uuid}</bdi> · {situacaoNaLista(p)}</span>
            </span>
          </label>
        {/each}
      </div>
    {/if}
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
  </div>
  {#snippet rodape()}
    <Botao variante="fantasma" onclick={carregar} desabilitado={carregando || enviando}>Atualizar</Botao>
    <Botao variante="secundario" onclick={() => onfechar?.()}>Cancelar</Botao>
    <Botao onclick={confirmar} desabilitado={!escolhido || enviando}
      >{trocar ? 'Usar este pendrive' : 'Ligar'}</Botao
    >
  {/snippet}
</Modal>

<style>
  .conteudo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
  }
  p {
    margin: 0;
  }
  .explicacao,
  .vazio {
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
  .opcao {
    display: flex;
    align-items: center;
    gap: var(--espaco-16);
    padding: var(--espaco-16) var(--espaco-20);
    border-radius: var(--raio-item);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
    color: var(--cor-texto-primario);
    cursor: pointer;
  }
  .opcao.escolhida {
    border-color: var(--cor-borda-foco);
  }
  .opcao:has(input:focus-visible) {
    outline: var(--linha-grossa) solid var(--cor-borda-foco);
    outline-offset: var(--linha-grossa);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    min-width: 0;
  }
  .nome,
  .detalhe {
    overflow-wrap: anywhere;
  }
  bdi {
    unicode-bidi: isolate;
  }
  .uuid {
    direction: ltr;
  }
  .nome {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .detalhe {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .falha {
    color: var(--cor-perigo-texto);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
</style>
