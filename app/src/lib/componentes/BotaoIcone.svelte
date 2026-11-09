<script>
  // `Botão ícone` (18832): 32×32 (controle/sm), raio controle (raio/md), ícone 20
  // (icone/md) no centro, fundo transparente; hover com superfície/padrao. O rótulo vai
  // só para leitura de tela e dica (`title`).
  import Icone from './Icone.svelte';

  /** @type {{ icone: string, rotulo: string, onclick?: (e: MouseEvent) => void }} */
  let { icone, rotulo, onclick = undefined } = $props();
</script>

<button class="botao-icone" type="button" aria-label={rotulo} title={rotulo} {onclick}>
  <Icone nome={icone} />
</button>

<style>
  .botao-icone {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--controle-sm);
    height: var(--controle-sm);
    padding: var(--espaco-0);
    border: none;
    border-radius: var(--raio-controle);
    background: transparent;
    cursor: pointer;
  }
  .botao-icone:hover {
    background: var(--cor-superficie-padrao);
  }
</style>
