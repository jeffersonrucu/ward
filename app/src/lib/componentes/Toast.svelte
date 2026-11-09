<script>
  // `Toast` (set 525): sucesso, erro, aviso. Largura largura/toast, raio cartao,
  // padding espaco/16 espaco/12 espaco/16 espaco/16, Texto/Corpo, Sombra/Flutuante.
  import Icone from './Icone.svelte';

  /** @type {{ tipo?: 'sucesso' | 'erro' | 'aviso', mensagem: string }} */
  let { tipo = 'sucesso', mensagem } = $props();
  const icones = { sucesso: 'sucesso', erro: 'erro', aviso: 'alerta' };
  const tons = { sucesso: 'sucesso', erro: 'perigo', aviso: 'aviso' };
</script>

<div class="toast" role={tipo === 'erro' ? 'alert' : 'status'}>
  <Icone nome={icones[tipo]} tom={tons[tipo]} />
  <span>{mensagem}</span>
</div>

<style>
  .toast {
    display: flex;
    align-items: center;
    gap: var(--espaco-12);
    width: var(--largura-toast);
    padding: var(--espaco-16) var(--espaco-12) var(--espaco-16) var(--espaco-16);
    border-radius: var(--raio-cartao);
    border: var(--linha-fina) solid var(--cor-borda-padrao);
    background: var(--cor-superficie-elevada);
    box-shadow: var(--sombra-flutuante);
    font-size: var(--tamanho-base);
    line-height: var(--altura-linha-base);
  }
</style>
