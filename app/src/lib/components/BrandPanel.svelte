<script>
  // Brand panel (FigCraft 717): left column of the first use and of the unlock screen.
  // Padding space-48, radius panel, border color-border-subtle, background
  // color-surface-default; horizontal logo on top, message (title-highlight, secondary
  // text-body and the three guarantees with the check icon, gap space-16 and space-12)
  // and footer in text-caption.
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';

  const GUARANTEES = [
    'Offline: nada sai desta máquina.',
    'Cifrado com Argon2id e XChaCha20-Poly1305.',
    'Uso sensível só com a sua aprovação e PIN.',
  ];
</script>

<aside class="panel">
  <Logo />
  <div class="message">
    <p class="title">Credenciais de trabalho, fora da conversa.</p>
    <p class="text">
      O Claude recebe o acesso, nunca a senha. Cada uso respeita o nível de acesso da entrada.
    </p>
    <ul class="guarantees">
      {#each GUARANTEES as g (g)}
        <li><Icon name="check" /><span>{g}</span></li>
      {/each}
    </ul>
  </div>
  <p class="footer">Só credenciais de trabalho · Linux</p>
</aside>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    flex: none;
    /* 560 in FigCraft (no bound variable; the value is the one of width-wide-dialog). */
    width: var(--width-wide-dialog);
    padding: var(--space-48);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .message {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
  }
  .title {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-3xl);
    line-height: var(--line-height-3xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .text {
    margin: var(--space-0);
    color: var(--color-text-secondary);
  }
  .guarantees {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    margin: var(--space-0);
    padding: var(--space-8) var(--space-0);
    list-style: none;
  }
  .guarantees li {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }
  .footer {
    margin: var(--space-0);
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
</style>
