<script>
  // Vitrine dos componentes do M0 (só no build de teste, D7): para o QA conferir no
  // Chrome contra a página Design System do Ward.figcraft. Credenciais falsas.
  import EnvironmentBadge from '../components/EnvironmentBadge.svelte';
  import TypeBadge from '../components/TypeBadge.svelte';
  import Button from '../components/Button.svelte';
  import Field from '../components/Field.svelte';
  import PinField from '../components/PinField.svelte';
  import SecretField from '../components/SecretField.svelte';
  import ListItem from '../components/ListItem.svelte';
  import Modal from '../components/Modal.svelte';
  import Select from '../components/Select.svelte';
  import Toast from '../components/Toast.svelte';
  import { TYPES } from '../components/types.js';

  let modalAberto = $state(false);
  let ambiente = $state('stage');
  let pin = $state('');
</script>

<section class="vitrine" aria-label="Componentes">
  <h2>Componentes</h2>
  <div class="linha">
    <Button>Salvar</Button>
    <Button variant="secondary" icon="lock">Travar agora</Button>
    <Button variant="danger">Excluir</Button>
    <Button variant="ghost">Cancelar</Button>
    <Button disabled>Desabilitado</Button>
  </div>
  <div class="linha">
    <Field id="v-grupo" label="Grupo" value="clientex" help="Pasta da entrada." />
    <Field id="v-host" label="Host" value="" placeholder="db.stage.clientex.local" error="Campo obrigatório" />
    <SecretField id="v-senha" label="Senha" />
    <Select
      id="v-ambiente"
      label="Ambiente"
      bind:value={ambiente}
      options={[
        { value: 'dev', label: 'dev' },
        { value: 'stage', label: 'stage' },
        { value: 'prod', label: 'prod' },
      ]}
    />
  </div>
  <div class="linha">
    <EnvironmentBadge environment="dev" />
    <EnvironmentBadge environment="stage" />
    <EnvironmentBadge environment="prod" />
    <EnvironmentBadge environment="prod" large />
    {#each Object.keys(TYPES) as t (t)}<TypeBadge kind={t} />{/each}
  </div>
  <div class="lista">
    <ListItem name="clientex-pg-stage" detail="db.stage.clientex.local · banco" kind="banco" environment="stage" level="com aprovação" />
    <ListItem name="clientex-ssh-prod" detail="deploy@prod.clientex.local" kind="servidor" environment="prod" level="com aprovação" selected />
  </div>
  <div class="linha">
    <Toast message="Senha copiada. A área de transferência é limpa em 30 s." />
    <Toast kind="warning" message="Anexo acima de 256 KiB." />
    <PinField id="v-pin" label="PIN" bind:value={pin} help="4 a 6 dígitos." />
  </div>
  <div class="linha">
    <Button variant="secondary" onclick={() => (modalAberto = true)}>Abrir modal</Button>
  </div>
  <Modal title="Excluir entrada" danger open={modalAberto} onclose={() => (modalAberto = false)}>
    A entrada clientex-pg-stage será excluída.
    {#snippet footer()}
      <Button variant="ghost" onclick={() => (modalAberto = false)}>Cancelar</Button>
      <Button variant="danger" onclick={() => (modalAberto = false)}>Excluir</Button>
    {/snippet}
  </Modal>
</section>

<style>
  .vitrine {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
    width: var(--width-content);
    padding: var(--space-32);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
  }
  h2 {
    margin: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--size-md);
    line-height: var(--line-height-md);
  }
  .linha {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-16);
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
</style>
