<script>
  // Tela `Configurações / Geral` (FigCraft 15242, Fase 38; V5.22).
  //
  // Duas colunas: `acesso` (15289: senha mestra, PIN de aprovação e tempo de trava, as
  // três `Linha de opção`) e `backup` (15325: backup local, backup externo em pendrive,
  // kit de emergência e a nota "Backups sempre cifrados").
  //
  // - Trocar a senha mestra pede a atual; trocar o PIN pede o PIN atual (V5.22). Os dois
  //   diálogos guardam o digitado só no estado local deles e o apagam antes da resposta.
  // - Tempo de trava: lido do daemon (`configuracoes`, a configuração do cofre, D30) e
  //   gravado por ele (`definir_tempo_trava`, geração nova, D39). Padrão 4 h.
  // - Backup externo: só exibido (desligado por padrão, D17); ligar é da Fase 46.
  //   Restaurar (Fase 47) ainda não abre nada.
  // - Kit de emergência (Fase 49, V7.10): "Imprimir" abre a folha (`KitEmergencia`).
  //
  // Nenhum valor secreto passa por esta tela: a resposta de `configuracoes` traz só os
  // minutos e o booleano do backup externo. Nada vai para armazenamento, URL ou estado
  // global.
  import { onDestroy, onMount } from 'svelte';
  import Botao from '../lib/componentes/Botao.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import LinhaDeOpcao from '../lib/componentes/LinhaDeOpcao.svelte';
  import Select from '../lib/componentes/Select.svelte';
  import Toast from '../lib/componentes/Toast.svelte';
  import { chamar } from '../lib/ponte/index.js';
  import KitEmergencia from './KitEmergencia.svelte';
  import TrocarPin from './TrocarPin.svelte';
  import TrocarSenha from './TrocarSenha.svelte';
  import { opcoesDeTrava } from './configuracoes.js';
  import { mensagemDoRust } from './primeiro-uso.js';

  /** @type {{ ontravado?: () => void }} */
  let { ontravado = undefined } = $props();

  const SUCESSO_MS = 4_000;

  let config = $state(null);
  let minutos = $state('');
  let erro = $state('');
  let sucesso = $state('');
  let dialogo = $state('');
  let salvando = $state(false);
  let prazoSucesso = null;

  const opcoes = $derived(opcoesDeTrava(config?.minutos_trava));

  function avisar(texto) {
    erro = '';
    sucesso = texto;
    clearTimeout(prazoSucesso);
    prazoSucesso = setTimeout(() => (sucesso = ''), SUCESSO_MS);
  }

  function aplicar(c) {
    config = { minutos_trava: c.minutos_trava, backup_externo: c.backup_externo === true };
    minutos = String(config.minutos_trava);
  }

  /** Erro de um pedido: travado vai para o destravar; o resto aparece no toast. */
  async function falhou(e, padrao) {
    if (e?.codigo === 'travado') {
      ontravado?.();
      return;
    }
    sucesso = '';
    erro = mensagemDoRust(e?.mensagem) || padrao;
    await conferirTrava();
  }

  /** Uma gravação que falha depois de conferida pode travar o cofre no daemon (D53). */
  async function conferirTrava() {
    try {
      const r = await chamar('estado_do_cofre');
      if (r?.estado === 'destravado') return;
    } catch {
      // Sem resposta: o destravar consulta de novo.
    }
    ontravado?.();
  }

  async function carregar() {
    try {
      aplicar(await chamar('configuracoes'));
    } catch (e) {
      await falhou(e, 'Não foi possível ler as configurações.');
    }
  }

  async function mudarTrava(valor) {
    const novo = Number(valor);
    if (!config || salvando || novo === config.minutos_trava) return;
    salvando = true;
    try {
      aplicar(await chamar('definir_tempo_trava', { minutos: novo }));
      avisar('Tempo de trava salvo.');
    } catch (e) {
      minutos = String(config.minutos_trava);
      await falhou(e, 'Não foi possível salvar o tempo de trava.');
    } finally {
      salvando = false;
    }
  }

  function fecharDialogo() {
    dialogo = '';
  }

  onMount(carregar);
  onDestroy(() => clearTimeout(prazoSucesso));
</script>

<section class="principal" aria-labelledby="configuracoes-titulo">
  <header class="topo">
    <h1 id="configuracoes-titulo" class="titulo-pagina">Configurações</h1>
    <p class="resumo">Acesso ao cofre, trava automática e backup.</p>
  </header>

  <div class="colunas">
    <section class="acesso" aria-labelledby="acesso-titulo">
      <h2 id="acesso-titulo" class="titulo-secao">Acesso</h2>
      <LinhaDeOpcao id="config-senha" titulo="Senha mestra">
        {#snippet descricao()}Só ela destrava o cofre. Trocar pede a senha atual.{/snippet}
        {#snippet controle()}
          <Botao variante="secundario" onclick={() => (dialogo = 'senha')} aria-label="Trocar senha mestra"
            >Trocar</Botao
          >
        {/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao id="config-pin" titulo="PIN de aprovação">
        {#snippet descricao()}4 a 6 dígitos. Aprova usos, não destrava. Trocar pede o PIN atual.{/snippet}
        {#snippet controle()}
          <Botao variante="secundario" onclick={() => (dialogo = 'pin')} aria-label="Trocar PIN de aprovação"
            >Trocar</Botao
          >
        {/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao id="config-trava" titulo="Tempo de trava">
        {#snippet descricao()}Sem uso, trava após este tempo (máximo 4 h). Também trava ao bloquear a
          tela e ao suspender.{/snippet}
        {#snippet controle()}
          <Select
            rotulo="Tempo de trava"
            id="config-tempo-trava"
            compacto
            bind:valor={minutos}
            {opcoes}
            desabilitado={!config || salvando}
            onmudar={mudarTrava}
          />
        {/snippet}
      </LinhaDeOpcao>
    </section>

    <section class="backup" aria-labelledby="backup-titulo">
      <h2 id="backup-titulo" class="titulo-secao">Backup</h2>
      <LinhaDeOpcao id="config-backup-local" titulo="Backup local">
        {#snippet descricao()}Automático e cifrado a cada mudança, com 30 versões em
          ~/.local/share/ward/backups/.{/snippet}
        {#snippet controle()}
          <Botao variante="secundario" desabilitado aria-label="Restaurar backup">Restaurar</Botao>
        {/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao
        id="config-backup-externo"
        titulo="Backup externo em pendrive"
        ligado={config?.backup_externo === true}
        desabilitado
      >
        {#snippet descricao()}{#if config?.backup_externo}Ligado. O Ward copia as versões para o
            pendrive escolhido sempre que ele for conectado.{:else}Desligado. Ao ligar, escolha o
            pendrive; o Ward copia as versões sempre que ele for conectado.{/if}{/snippet}
      </LinhaDeOpcao>
      <LinhaDeOpcao id="config-kit" titulo="Kit de emergência">
        {#snippet descricao()}Folha para imprimir com onde ficam o cofre e os backups e um espaço
          para anotar a senha mestra à mão.{/snippet}
        {#snippet controle()}
          <Botao variante="secundario" onclick={() => (dialogo = 'kit')} aria-label="Imprimir kit de emergência"
            >Imprimir</Botao
          >
        {/snippet}
      </LinhaDeOpcao>
      <div class="nota">
        <Icone nome="cadeado" />
        <div class="textos">
          <span class="titulo-nota">Backups sempre cifrados</span>
          <span class="texto-nota">
            Cada versão fica cifrada com a senha mestra que valia quando foi gravada. Restaurar
            pede essa senha e guarda o cofre atual como backup antes da troca.
          </span>
        </div>
      </div>
      <div class="avisos">
        {#if erro}<Toast tipo="erro" mensagem={erro} />{/if}
        {#if sucesso}<Toast tipo="sucesso" mensagem={sucesso} />{/if}
      </div>
    </section>
  </div>
</section>

<KitEmergencia
  aberto={dialogo === 'kit'}
  onfechar={fecharDialogo}
  ontravado={() => {
    dialogo = '';
    ontravado?.();
  }}
/>
<TrocarSenha
  aberto={dialogo === 'senha'}
  onfechar={fecharDialogo}
  ontrocada={() => {
    dialogo = '';
    avisar('Senha mestra trocada.');
  }}
  onfalha={() => conferirTrava()}
/>
<TrocarPin
  aberto={dialogo === 'pin'}
  onfechar={fecharDialogo}
  ontrocado={() => {
    dialogo = '';
    avisar('PIN trocado.');
  }}
  ontravado={() => {
    dialogo = '';
    ontravado?.();
  }}
  onfalha={() => conferirTrava()}
/>

<style>
  .principal {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-16);
    flex: 1;
    min-width: 0;
  }
  .topo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-2);
    padding: var(--espaco-4) var(--espaco-8) var(--espaco-0);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .resumo {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .colunas {
    display: flex;
    gap: var(--espaco-16);
    flex: 1;
    min-height: 0;
  }
  .acesso,
  .backup {
    display: flex;
    flex-direction: column;
    padding: var(--espaco-24);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
    overflow-y: auto;
  }
  .acesso {
    gap: var(--espaco-12);
    flex: none;
    width: var(--largura-coluna-lista);
  }
  .backup {
    gap: var(--espaco-20);
    flex: 1;
    min-width: 0;
  }
  .nota {
    display: flex;
    gap: var(--espaco-12);
    padding: var(--espaco-20);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-superficie-padrao);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
    min-width: 0;
  }
  .titulo-nota {
    font-weight: var(--peso-medio);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
  .texto-nota {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .avisos {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-12);
  }
</style>
