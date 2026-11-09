<script>
  // `Nova credencial / Escolher tipo` (17657; detalhe 17851): cabeçalho com avatar "mais",
  // "Nova credencial" e "Escolha o tipo. Ele não muda depois de salvo.", Cancelar; grade
  // de 3 colunas (gap espaco/12) com os 9 `Cartão de tipo`; nota do nível no fim.
  // Os tipos banco, servidor e ftp/sftp chegam na Fase 34 e ficam desabilitados.
  import Botao from '../lib/componentes/Botao.svelte';
  import CartaoDeTipo from '../lib/componentes/CartaoDeTipo.svelte';
  import Icone from '../lib/componentes/Icone.svelte';
  import { CARTOES_TIPO } from './formulario.js';

  /** @type {{ onescolher: (tipo: string) => void, oncancelar: () => void }} */
  let { onescolher, oncancelar } = $props();
</script>

<header class="cabecalho">
  <span class="avatar"><Icone nome="mais" tamanho="lg" /></span>
  <div class="identificacao">
    <h2 class="nome">Nova credencial</h2>
    <p class="subtitulo">Escolha o tipo. Ele não muda depois de salvo.</p>
  </div>
  <Botao variante="secundario" onclick={oncancelar}>Cancelar</Botao>
</header>
<div class="corpo">
  <div class="grade" role="radiogroup" aria-label="Tipo da credencial">
    {#each CARTOES_TIPO as c (c.tipo)}
      <CartaoDeTipo
        tipo={c.tipo}
        nome={c.nome}
        descricao={c.descricao}
        desabilitado={!!c.depois}
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
    gap: var(--espaco-16);
    padding: var(--espaco-24);
    border-bottom: var(--linha-fina) solid var(--cor-borda-sutil);
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--controle-lg);
    height: var(--controle-lg);
    border-radius: var(--raio-controle);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
  }
  .identificacao {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-4);
    flex: 1;
  }
  .nome,
  .subtitulo,
  .nota {
    margin: 0;
  }
  .nome {
    font-weight: var(--peso-semibold);
    font-size: var(--tamanho-lg);
    line-height: var(--altura-linha-lg);
  }
  .subtitulo,
  .nota {
    color: var(--cor-texto-secundario);
    font-size: var(--tamanho-sm);
    line-height: var(--altura-linha-sm);
  }
  .corpo {
    display: flex;
    flex-direction: column;
    gap: var(--espaco-20);
    flex: 1;
    min-height: 0;
    padding: var(--espaco-24);
    overflow-y: auto;
  }
  .grade {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--espaco-12);
  }
</style>
