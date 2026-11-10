<script>
  // "Trocar" do PIN de aprovação em `Configurações / Geral` (Fase 38, V5.22, S9). Frame
  // `Configurações / Trocar PIN` (20193; Fase 39d), com o `Diálogo de troca` › `Tipo=PIN`
  // (19979, instância 20471): `Modal` (611) com três `Campo PIN` (650) e o `Botão` (263).
  //
  // Pede o PIN atual (o daemon o confere; o 5.º erro seguido trava o cofre) e o novo
  // duas vezes. Os valores ficam só no estado local deste componente; saem do estado
  // antes da resposta do pedido e ao fechar. Nada vai para armazenamento, URL ou log.
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import { call } from '../lib/bridge/index.js';
  import { MAX_DIGITOS_PIN, MENSAGENS, mensagemDoRust, pinValido } from './primeiro-uso.js';

  /**
   * @type {{ aberto?: boolean, onfechar?: () => void, ontrocado?: () => void,
   *   ontravado?: () => void, onfalha?: (codigo: string) => void }}
   */
  let {
    aberto = false,
    onfechar = undefined,
    ontrocado = undefined,
    ontravado = undefined,
    onfalha = undefined,
  } = $props();

  const IDS = { atual: 'trocar-pin-atual', novo: 'trocar-pin-novo', confirmacao: 'trocar-pin-confirmacao' };

  let atual = $state('');
  let novo = $state('');
  let confirmacao = $state('');
  let erros = $state({ atual: '', novo: '', confirmacao: '' });
  let pinErrado = $state(false);
  let falha = $state('');
  let enviando = $state(false);

  function apagar() {
    atual = '';
    novo = '';
    confirmacao = '';
  }

  function limparErros() {
    erros = { atual: '', novo: '', confirmacao: '' };
    pinErrado = false;
    falha = '';
  }

  async function focar(campo) {
    await tick();
    document.getElementById(IDS[campo])?.focus();
  }

  $effect(() => {
    if (aberto) {
      apagar();
      limparErros();
      focar('atual');
    }
  });

  onDestroy(apagar);

  function fechar() {
    apagar();
    limparErros();
    onfechar?.();
  }

  // Letra, símbolo ou 7º dígito: o `Campo PIN` não aceita, e a recusa aparece no campo
  // (o mesmo aviso do primeiro uso).
  function recusa(campo) {
    return (e) => {
      const texto = e.data ?? e.dataTransfer?.getData('text/plain') ?? '';
      if (!texto) return;
      const valor = { atual, novo, confirmacao }[campo];
      const semSelecao = e.target?.selectionStart === e.target?.selectionEnd;
      if (/\D/.test(texto)) erros[campo] = MENSAGENS.pinComLetra;
      else if (semSelecao && valor.length + texto.length > MAX_DIGITOS_PIN) erros[campo] = MENSAGENS.pinLongo;
      else erros[campo] = '';
    };
  }

  async function enviar(e) {
    e?.preventDefault();
    if (enviando) return;
    limparErros();
    if (!pinValido(atual)) {
      erros.atual = MENSAGENS.pinInvalido;
      return focar('atual');
    }
    if (!pinValido(novo)) {
      erros.novo = MENSAGENS.pinInvalido;
      return focar('novo');
    }
    if (confirmacao !== novo) {
      erros.confirmacao = MENSAGENS.pinDiferente;
      return focar('confirmacao');
    }
    // Os PINs saem do estado antes da resposta: a tela não os guarda nem no erro.
    const args = { dados: { atual, novo, confirmacao } };
    apagar();
    enviando = true;
    try {
      const r = await call('trocar_pin', args);
      if (r?.trocado) {
        ontrocado?.();
        return;
      }
      for (const erro of r?.erros ?? []) {
        if (erro.campo in erros) erros[erro.campo] = mensagemDoRust(erro.motivo);
      }
    } catch (err) {
      if (err?.codigo === 'pin-incorreto') {
        pinErrado = true;
        erros.atual = 'PIN incorreto. Nada foi alterado.';
        if (await travou()) return;
      } else if (err?.codigo === 'travado') {
        ontravado?.();
        return;
      } else {
        falha = mensagemDoRust(err?.mensagem) || 'Não foi possível trocar o PIN.';
        onfalha?.(err?.codigo ?? '');
      }
    } finally {
      args.dados = null;
      enviando = false;
    }
    await focar('atual');
  }

  /** Depois de um PIN errado: o 5.º seguido trava o cofre (o daemon conta). */
  async function travou() {
    try {
      const r = await call('estado_do_cofre');
      if (r?.estado === 'destravado') return false;
    } catch {
      // Sem resposta, trata como travado: a tela de destravar consulta de novo.
    }
    ontravado?.();
    return true;
  }
</script>

<Modal title="Trocar PIN" open={aberto} onclose={fechar}>
  <form class="corpo" onsubmit={enviar} novalidate data-teste="trocar-pin">
    <p class="explicacao">
      O PIN aprova usos e mudanças de nível; ele não destrava o cofre. O PIN atual confirma
      a troca.
    </p>
    <div class="pin" onbeforeinput={recusa('atual')}>
      <PinField label="PIN atual" id={IDS.atual} bind:value={atual} error={erros.atual} />
    </div>
    <div class="pin" onbeforeinput={recusa('novo')}>
      <PinField
        label="Novo PIN"
        id={IDS.novo}
        bind:value={novo}
        help="4 a 6 dígitos."
        error={erros.novo}
      />
    </div>
    <div class="pin" onbeforeinput={recusa('confirmacao')}>
      <PinField
        label="Confirmar novo PIN"
        id={IDS.confirmacao}
        bind:value={confirmacao}
        help="Digite o mesmo PIN de novo."
        error={erros.confirmacao}
      />
    </div>
    {#if pinErrado}
      <p class="nota aviso" role="alert">O PIN continua o mesmo. 5 erros de PIN seguidos travam o cofre.</p>
    {:else}
      <p class="nota">5 erros de PIN seguidos travam o cofre.</p>
    {/if}
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <button class="visually-hidden" type="submit" tabindex="-1">Trocar PIN</button>
  </form>
  {#snippet footer()}
    <Button variant="secondary" onclick={fechar}>Cancelar</Button>
    <Button variant="primary" disabled={enviando} onclick={enviar}>Trocar PIN</Button>
  {/snippet}
</Modal>

<style>
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    margin: var(--space-0);
    color: var(--color-text-primary);
  }
  p {
    margin: var(--space-0);
  }
  /* Fase 54j: a explicação é `Texto/Corpo` (`8:12`, 14/20) como no frame `20193` ›
     `20478`; a nota é `8:14` (13/18). */
  .explicacao {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-secondary);
  }
  .nota {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .nota.aviso {
    color: var(--color-warning-text);
  }
  .falha {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-danger-text);
  }
</style>
