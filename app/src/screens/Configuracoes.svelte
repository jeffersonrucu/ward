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
  // - Backup externo (Fase 48, V7.9; FigCraft 16178): desligado por padrão (D17).
  //   Ligar abre a escolha do pendrive (`EscolherPendrive`); ligado, o cartão do
  //   pendrive (16318) mostra o escolhido e "Trocar". Desligar mantém o UUID guardado e
  //   não apaga nada do pendrive. Lido por `backup_externo` (sem caminho de montagem).
  //   Restaurar (Fase 47, V7.7) abre `RestaurarBackup` (16349/16512).
  // - Fase 49a: a pasta do backup local insegura (o daemon grava o cofre sem backup)
  //   vira um `Destaque` aviso logo abaixo da linha "Backup local"; só um sim/não do
  //   daemon (`backup_local`), nenhum caminho.
  // - Kit de emergência (Fase 49, V7.10): "Imprimir" abre a folha (`KitEmergencia`).
  //
  // Nenhum valor secreto passa por esta tela: as respostas trazem só os minutos, o
  // backup externo (ligado, UUID do pendrive, rótulo já neutralizado no Rust e se está
  // montado). Nada vai para armazenamento, URL ou estado global.
  import { onDestroy, onMount } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import UsbDriveCard from '../lib/components/UsbDriveCard.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import OptionRow from '../lib/components/OptionRow.svelte';
  import Select from '../lib/components/Select.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';
  import KitEmergencia from './KitEmergencia.svelte';
  import TrocarPin from './TrocarPin.svelte';
  import TrocarSenha from './TrocarSenha.svelte';
  import EscolherPendrive from './EscolherPendrive.svelte';
  import RestaurarBackup from './RestaurarBackup.svelte';
  import { mensagemRestaurado } from './restaurar.js';
  import { AVISO_BACKUP_LOCAL, cartaoDoPendrive, opcoesDeTrava } from './configuracoes.js';
  import { mensagemDoRust } from './primeiro-uso.js';

  /** @type {{ onlocked?: () => void }} */
  let { onlocked: ontravado = undefined } = $props();

  const SUCESSO_MS = 4_000;

  let config = $state(null);
  let minutos = $state('');
  let erro = $state('');
  let sucesso = $state('');
  let dialogo = $state('');
  let salvando = $state(false);
  let externo = $state(null);
  let mudandoExterno = $state(false);
  let prazoSucesso = null;

  const opcoes = $derived(opcoesDeTrava(config?.minutos_trava));
  const ligadoExterno = $derived(externo ? externo.ligado === true : config?.backup_externo === true);
  const cartao = $derived(cartaoDoPendrive(externo));

  function avisar(texto) {
    erro = '';
    sucesso = texto;
    clearTimeout(prazoSucesso);
    prazoSucesso = setTimeout(() => (sucesso = ''), SUCESSO_MS);
  }

  function aplicar(c) {
    config = {
      minutos_trava: c.minutos_trava,
      backup_externo: c.backup_externo === true,
      backup_local: c.backup_local !== false,
    };
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
      const r = await call('estado_do_cofre');
      if (r?.estado === 'destravado') return;
    } catch {
      // Sem resposta: o destravar consulta de novo.
    }
    ontravado?.();
  }

  async function carregar() {
    try {
      aplicar(await call('configuracoes'));
      externo = await call('backup_externo');
    } catch (e) {
      await falhou(e, 'Não foi possível ler as configurações.');
    }
  }

  /** Ligar abre a escolha do pendrive; desligar grava na hora (o UUID fica guardado). */
  async function mudarExterno(ligar) {
    if (!externo || mudandoExterno) return;
    if (ligar) {
      dialogo = 'pendrive';
      return;
    }
    mudandoExterno = true;
    try {
      externo = await call('definir_backup_externo', { ligado: false, uuid: null });
      config = config && { ...config, backup_externo: false };
      avisar('Backup externo desligado. As cópias no pendrive continuam lá.');
    } catch (e) {
      await falhou(e, 'Não foi possível desligar o backup externo.');
    } finally {
      mudandoExterno = false;
    }
  }

  function pendriveEscolhido(novo) {
    const trocou = externo?.ligado === true;
    dialogo = '';
    externo = novo;
    config = config && { ...config, backup_externo: novo?.ligado === true };
    avisar(trocou ? 'Pendrive do backup externo trocado.' : 'Backup externo ligado.');
  }

  async function mudarTrava(valor) {
    const novo = Number(valor);
    if (!config || salvando || novo === config.minutos_trava) return;
    salvando = true;
    try {
      aplicar(await call('definir_tempo_trava', { minutos: novo }));
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
    <h1 id="configuracoes-titulo" class="title-page">Configurações</h1>
    <p class="resumo">Acesso ao cofre, trava automática e backup.</p>
  </header>

  <div class="colunas">
    <section class="acesso" aria-labelledby="acesso-titulo">
      <h2 id="acesso-titulo" class="title-section">Acesso</h2>
      <OptionRow id="config-senha" title="Senha mestra">
        {#snippet description()}Só ela destrava o cofre. Trocar pede a senha atual.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialogo = 'senha')} aria-label="Trocar senha mestra"
            >Trocar</Button
          >
        {/snippet}
      </OptionRow>
      <OptionRow id="config-pin" title="PIN de aprovação">
        {#snippet description()}4 a 6 dígitos. Aprova usos, não destrava. Trocar pede o PIN atual.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialogo = 'pin')} aria-label="Trocar PIN de aprovação"
            >Trocar</Button
          >
        {/snippet}
      </OptionRow>
      <OptionRow id="config-trava" title="Tempo de trava">
        {#snippet description()}Sem uso, trava após este tempo (máximo 4 h). Também trava ao bloquear a
          tela e ao suspender.{/snippet}
        {#snippet control()}
          <Select
            label="Tempo de trava"
            id="config-tempo-trava"
            compact
            bind:value={minutos}
            options={opcoes}
            disabled={!config || salvando}
            onchange={mudarTrava}
          />
        {/snippet}
      </OptionRow>
    </section>

    <section class="backup" aria-labelledby="backup-titulo">
      <h2 id="backup-titulo" class="title-section">Backup</h2>
      <OptionRow id="config-backup-local" title="Backup local">
        {#snippet description()}Automático e cifrado a cada mudança, com 30 versões em
          ~/.local/share/ward/backups/.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialogo = 'restaurar')} aria-label="Restaurar backup"
            >Restaurar</Button
          >
        {/snippet}
      </OptionRow>
      {#if config && !config.backup_local}
        <div id="config-backup-local-aviso" role="status">
          <Highlight kind="warning" icon="alert">{AVISO_BACKUP_LOCAL}</Highlight>
        </div>
      {/if}
      <OptionRow
        id="config-backup-externo"
        title="Backup externo em pendrive"
        on={ligadoExterno}
        disabled={!externo || mudandoExterno}
        onchange={mudarExterno}
      >
        {#snippet description()}{#if ligadoExterno}Ligado. O Ward copia as versões para o pendrive
            escolhido sempre que ele for conectado, também com 30 versões.{:else}Desligado. Ao
            ligar, escolha o pendrive; o Ward copia as versões sempre que ele for conectado.{/if}{/snippet}
      </OptionRow>
      {#if externo?.ligado && externo.uuid}
        <UsbDriveCard
          name={cartao.nome}
          uuid={externo.uuid}
          situation={cartao.situacao}
          status={cartao.estado}
          mounted={cartao.montado}
          disabled={mudandoExterno}
          onchange={() => (dialogo = 'pendrive')}
        />
      {/if}
      <OptionRow id="config-kit" title="Kit de emergência">
        {#snippet description()}Folha para imprimir com onde ficam o cofre e os backups e um espaço
          para anotar a senha mestra à mão.{/snippet}
        {#snippet control()}
          <Button variant="secondary" onclick={() => (dialogo = 'kit')} aria-label="Imprimir kit de emergência"
            >Imprimir</Button
          >
        {/snippet}
      </OptionRow>
      <div class="nota">
        <Icon name="lock" />
        <div class="textos">
          <span class="titulo-nota">Backups sempre cifrados</span>
          <span class="texto-nota">
            Cada versão fica cifrada com a senha mestra que valia quando foi gravada. Restaurar
            pede essa senha e guarda o cofre atual como backup antes da troca.
          </span>
        </div>
      </div>
      <div class="avisos">
        {#if erro}<Toast kind="error" message={erro} />{/if}
        {#if sucesso}<Toast kind="success" message={sucesso} />{/if}
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
<EscolherPendrive
  aberto={dialogo === 'pendrive'}
  trocar={externo?.ligado === true}
  atual={externo?.uuid ?? null}
  onfechar={fecharDialogo}
  onescolhido={pendriveEscolhido}
  ontravado={() => {
    dialogo = '';
    ontravado?.();
  }}
/>
<RestaurarBackup
  aberto={dialogo === 'restaurar'}
  onfechar={fecharDialogo}
  onrestaurado={(r) => {
    dialogo = '';
    avisar(mensagemRestaurado(r.instante, r.entradas));
    carregar();
  }}
  ontravado={() => {
    dialogo = '';
    ontravado?.();
  }}
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
    gap: var(--space-16);
    flex: 1;
    min-width: 0;
  }
  .topo {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-8) var(--space-0);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .resumo {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .colunas {
    display: flex;
    gap: var(--space-16);
    flex: 1;
    min-height: 0;
  }
  .acesso,
  .backup {
    display: flex;
    flex-direction: column;
    padding: var(--space-24);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow-y: auto;
  }
  .acesso {
    gap: var(--space-12);
    flex: none;
    width: var(--width-column-list);
  }
  .backup {
    gap: var(--space-20);
    flex: 1;
    min-width: 0;
  }
  .nota {
    display: flex;
    gap: var(--space-12);
    padding: var(--space-20);
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    min-width: 0;
  }
  .titulo-nota {
    font-weight: var(--weight-medium);
    font-size: var(--size-base);
    line-height: var(--line-height-base);
  }
  .texto-nota {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .avisos {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
</style>
