<script>
  // Vitrine dos componentes do M0 (só no build de teste, D7): para o QA conferir no
  // Chrome contra a página Design System do Ward.figcraft. Credenciais falsas.
  import BadgeAmbiente from '../componentes/BadgeAmbiente.svelte';
  import BadgeTipo from '../componentes/BadgeTipo.svelte';
  import Botao from '../componentes/Botao.svelte';
  import Campo from '../componentes/Campo.svelte';
  import CampoPin from '../componentes/CampoPin.svelte';
  import CampoSecreto from '../componentes/CampoSecreto.svelte';
  import ItemDeLista from '../componentes/ItemDeLista.svelte';
  import Modal from '../componentes/Modal.svelte';
  import Select from '../componentes/Select.svelte';
  import Toast from '../componentes/Toast.svelte';
  import { TIPOS } from '../componentes/tipos.js';

  let modalAberto = $state(false);
  let ambiente = $state('stage');
  let pin = $state('');
</script>

<section class="vitrine" aria-label="Componentes">
  <h2>Componentes</h2>
  <div class="linha">
    <Botao>Salvar</Botao>
    <Botao variante="secundario" icone="cadeado">Travar agora</Botao>
    <Botao variante="perigo">Excluir</Botao>
    <Botao variante="fantasma">Cancelar</Botao>
    <Botao desabilitado>Desabilitado</Botao>
  </div>
  <div class="linha">
    <Campo id="v-grupo" rotulo="Grupo" valor="clientex" ajuda="Pasta da entrada." />
    <Campo id="v-host" rotulo="Host" valor="" placeholder="db.stage.clientex.local" erro="Campo obrigatório" />
    <CampoSecreto id="v-senha" rotulo="Senha" />
    <Select
      id="v-ambiente"
      rotulo="Ambiente"
      bind:valor={ambiente}
      opcoes={[
        { valor: 'dev', rotulo: 'dev' },
        { valor: 'stage', rotulo: 'stage' },
        { valor: 'prod', rotulo: 'prod' },
      ]}
    />
  </div>
  <div class="linha">
    <BadgeAmbiente ambiente="dev" />
    <BadgeAmbiente ambiente="stage" />
    <BadgeAmbiente ambiente="prod" />
    <BadgeAmbiente ambiente="prod" grande />
    {#each Object.keys(TIPOS) as t (t)}<BadgeTipo tipo={t} />{/each}
  </div>
  <div class="lista">
    <ItemDeLista nome="clientex-pg-stage" detalhe="db.stage.clientex.local · banco" tipo="banco" ambiente="stage" nivel="com aprovação" />
    <ItemDeLista nome="clientex-ssh-prod" detalhe="deploy@prod.clientex.local" tipo="servidor" ambiente="prod" nivel="com aprovação" selecionado />
  </div>
  <div class="linha">
    <Toast mensagem="Senha copiada. A área de transferência é limpa em 30 s." />
    <Toast tipo="aviso" mensagem="Anexo acima de 256 KiB." />
    <CampoPin id="v-pin" rotulo="PIN" bind:valor={pin} ajuda="4 a 6 dígitos." />
  </div>
  <div class="linha">
    <Botao variante="secundario" onclick={() => (modalAberto = true)}>Abrir modal</Botao>
  </div>
  <Modal titulo="Excluir entrada" perigo aberto={modalAberto} onfechar={() => (modalAberto = false)}>
    A entrada clientex-pg-stage será excluída.
    {#snippet rodape()}
      <Botao variante="fantasma" onclick={() => (modalAberto = false)}>Cancelar</Botao>
      <Botao variante="perigo" onclick={() => (modalAberto = false)}>Excluir</Botao>
    {/snippet}
  </Modal>
</section>

<style>
  .vitrine {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-24);
    width: var(--largura-conteudo);
    padding: var(--espaco-32);
    border-radius: var(--raio-painel);
    border: var(--linha-fina) solid var(--cor-borda-sutil);
    background: var(--cor-fundo-sutil);
  }
  h2 {
    margin: 0;
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-md);
    line-height: var(--altura-linha-md);
  }
  .linha {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--espaco-16);
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-8);
  }
</style>
