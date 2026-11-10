<script>
  // `Nova credencial / Escolher tipo` (17657; detalhe 17851): cabeçalho com avatar "mais",
  // "Nova credencial" e "Escolha o tipo. Ele não muda depois de salvo.", Cancelar; grade
  // de 3 colunas (gap espaco/12) com os 9 `Cartão de tipo`; nota do nível no fim.
  // Os tipos banco, servidor e ftp/sftp chegam na Fase 34 e ficam desabilitados.
  import Button from '../lib/components/Button.svelte';
  import TypeCard from '../lib/components/TypeCard.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import { CARTOES_TIPO } from './formulario.js';

  /** @type {{ onescolher: (tipo: string) => void, oncancelar: () => void }} */
  let { onescolher, oncancelar } = $props();
</script>

<header class="cabecalho">
  <span class="avatar"><Icon name="plus" size="lg" /></span>
  <div class="identificacao">
    <h2 class="nome">Nova credencial</h2>
    <p class="subtitulo">Escolha o tipo. Ele não muda depois de salvo.</p>
  </div>
  <Button variant="secondary" onclick={oncancelar}>Cancelar</Button>
</header>
<div class="corpo">
  <div class="grade" role="radiogroup" aria-label="Tipo da credencial">
    {#each CARTOES_TIPO as c (c.tipo)}
      <TypeCard
        kind={c.tipo}
        name={c.nome}
        description={c.descricao}
        disabled={!!c.depois}
        onclick={() => onescolher(c.tipo)}
      />
    {/each}
  </div>
  <p class="nota">Toda entrada nova começa com aprovação. Mudar o nível depois pede o PIN.</p>
</div>

<style>
  .cabecalho {
    display: flex;
    align-items: center;
    gap: var(--space-16);
    padding: var(--space-24);
    border-bottom: var(--line-thin) solid var(--color-border-subtle);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    border-radius: var(--radius-control);
    border: var(--line-thin) solid var(--color-border-default);
    background: var(--color-surface-raised);
  }
  .identificacao {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
  }
  .nome,
  .subtitulo,
  .nota {
    margin: 0;
  }
  .nome {
    font-weight: var(--weight-semibold);
    font-size: var(--size-lg);
    line-height: var(--line-height-lg);
  }
  .subtitulo,
  .nota {
    color: var(--color-text-secondary);
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
    flex: 1;
    min-height: 0;
    padding: var(--space-24);
    overflow-y: auto;
  }
  .grade {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-12);
  }
</style>
