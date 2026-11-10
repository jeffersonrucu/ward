<script>
  // "Trocar" da senha mestra em `Configurações / Geral` (Fase 38, V5.22). Sem frame
  // próprio no FigCraft: monta o `Modal` (611) com três `Campo senha` (17511), o
  // medidor de força do primeiro uso e o `Botão` (263).
  //
  // Pede a senha atual (o daemon a confere: D52) e a nova duas vezes, com a regra do
  // primeiro uso (12 caracteres ou mais; o Rust confere de novo). Os três valores ficam
  // só no estado local deste componente; saem do estado antes da resposta do pedido e
  // ao fechar. Nada vai para armazenamento, URL ou log.
  //
  // Fase 54 (auditoria final): aviso de que os backups antigos seguem abrindo com a
  // senha antiga (cor `aviso/texto`, como a nota do `TrocarPin`).
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import { call } from '../lib/bridge/index.js';
  import { MESSAGES, passwordError, passwordStrength, rustMessage } from './first-use.js';

  /**
   * @type {{ aberto?: boolean, onfechar?: () => void, ontrocada?: () => void,
   *   onfalha?: (codigo: string) => void }}
   */
  let { aberto = false, onfechar = undefined, ontrocada = undefined, onfalha = undefined } = $props();

  let atual = $state('');
  let nova = $state('');
  let confirmacao = $state('');
  let erros = $state({ atual: '', nova: '', confirmacao: '' });
  let falha = $state('');
  let enviando = $state(false);
  let campoAtual = $state();
  let campoNova = $state();
  let campoConfirmacao = $state();

  const forca = $derived(passwordStrength(nova));

  function apagar() {
    atual = '';
    nova = '';
    confirmacao = '';
  }

  function limparErros() {
    erros = { atual: '', nova: '', confirmacao: '' };
    falha = '';
  }

  $effect(() => {
    if (aberto) {
      apagar();
      limparErros();
      tick().then(() => campoAtual?.focus());
    }
  });

  onDestroy(apagar);

  function fechar() {
    apagar();
    limparErros();
    onfechar?.();
  }

  async function enviar(e) {
    e?.preventDefault();
    if (enviando) return;
    limparErros();
    if (!atual) {
      erros.atual = 'Digite a senha mestra atual.';
      campoAtual?.focus();
      return;
    }
    const curta = passwordError(nova);
    if (curta) {
      erros.nova = curta;
      campoNova?.focus();
      return;
    }
    if (confirmacao !== nova) {
      erros.confirmacao = MESSAGES.passwordMismatch;
      campoConfirmacao?.focus();
      return;
    }
    // Os valores saem do estado antes da resposta: a tela não os guarda nem no erro.
    const args = { dados: { atual, nova, confirmacao } };
    apagar();
    enviando = true;
    try {
      const r = await call('trocar_senha_mestra', args);
      if (r?.trocado) {
        ontrocada?.();
        return;
      }
      for (const erro of r?.erros ?? []) {
        if (erro.campo in erros) erros[erro.campo] = rustMessage(erro.motivo);
      }
    } catch (err) {
      if (err?.codigo === 'senha-incorreta') {
        erros.atual = 'Senha mestra incorreta. Nada foi alterado.';
      } else {
        falha = rustMessage(err?.mensagem) || 'Não foi possível trocar a senha mestra.';
        onfalha?.(err?.codigo ?? '');
      }
    } finally {
      args.dados = null;
      enviando = false;
    }
    await tick();
    campoAtual?.focus();
  }
</script>

<Modal title="Trocar senha mestra" open={aberto} onclose={fechar}>
  <form class="corpo" onsubmit={enviar} novalidate data-teste="trocar-senha">
    <p class="explicacao">
      A senha atual confirma a troca. Depois dela, o cofre só abre com a nova.
    </p>
    <p class="aviso" data-teste="trocar-senha-aviso-backups">
      Os backups antigos, no computador e no pendrive, continuam abrindo com a senha antiga.
      Se você troca porque a senha antiga vazou, apague esses backups.
    </p>
    <PasswordField
      label="Senha mestra atual"
      id="trocar-senha-atual"
      bind:value={atual}
      bind:input={campoAtual}
      autocomplete="current-password"
      error={erros.atual}
    />
    <PasswordField
      label="Nova senha mestra"
      id="trocar-senha-nova"
      bind:value={nova}
      bind:input={campoNova}
      help="Mínimo de 12 caracteres."
      error={erros.nova}
    />
    <div class="forca" aria-live="polite">
      <div class="barras" aria-hidden="true">
        {#each [1, 2, 3, 4] as n (n)}
          <span class="barra" data-tone={forca && n <= forca.segments ? forca.tone : undefined}
          ></span>
        {/each}
      </div>
      <span class="legenda" data-tone={forca?.tone}>
        {forca ? `Força: ${forca.label}` : 'Força: digite a senha'}
      </span>
    </div>
    <PasswordField
      label="Confirmar nova senha"
      id="trocar-senha-confirmacao"
      bind:value={confirmacao}
      bind:input={campoConfirmacao}
      error={erros.confirmacao}
    />
    {#if falha}<p class="falha" role="alert">{falha}</p>{/if}
    <button class="visually-hidden" type="submit" tabindex="-1">Trocar senha</button>
  </form>
  {#snippet footer()}
    <Button variant="secondary" onclick={fechar}>Cancelar</Button>
    <Button variant="primary" disabled={enviando} onclick={enviar}>Trocar senha</Button>
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
  .explicacao {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-text-secondary);
  }
  .aviso {
    font-size: var(--size-base);
    line-height: var(--line-height-base);
    color: var(--color-warning-text);
  }
  .falha {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-danger-text);
  }
  .forca {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .barras {
    display: flex;
    gap: var(--space-6);
  }
  .barra {
    flex: 1;
    height: var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--color-border-default);
  }
  .barra[data-tone='danger'] {
    background: var(--color-danger-default);
  }
  .barra[data-tone='warning'] {
    background: var(--color-warning-default);
  }
  .barra[data-tone='success'] {
    background: var(--color-success-default);
  }
  .legenda {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .legenda[data-tone='danger'] {
    color: var(--color-danger-text);
  }
  .legenda[data-tone='warning'] {
    color: var(--color-warning-text);
  }
  .legenda[data-tone='success'] {
    color: var(--color-success-text);
  }
</style>
