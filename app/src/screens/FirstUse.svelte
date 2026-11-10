<script>
  // First use screens: create vault (3537), master password (3619) and PIN (3733).
  // Window with the brand panel and the content panel (bg-subtle, border-subtle, radius
  // panel, padding space-48) with the form column centered (width column-access; gap
  // space-32 on step 1, space-24 on the others).
  //
  // The master password and the PIN are typed here and live only in the state of this
  // component, until the create vault command: the Rust side checks everything again
  // (length, digits and both confirmations) and the daemon stores only the PIN hash
  // (D30). After sending, the values are cleared here; on leaving the screen, the
  // component goes away with them. Nothing goes to browser storage, the URL or global
  // state.
  import { onDestroy, tick } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import PinField from '../lib/components/PinField.svelte';
  import PasswordField from '../lib/components/PasswordField.svelte';
  import Highlight from '../lib/components/Highlight.svelte';
  import Icon from '../lib/components/Icon.svelte';
  import StepIndicator from '../lib/components/StepIndicator.svelte';
  import BrandPanel from '../lib/components/BrandPanel.svelte';
  import Toast from '../lib/components/Toast.svelte';
  import { call } from '../lib/bridge/index.js';
  import {
    MAX_PIN_DIGITS,
    MESSAGES,
    passwordError,
    passwordStrength,
    rustMessage,
    isValidPin,
  } from './first-use.js';

  /** @type {{ oncreated: () => void }} */
  let { oncreated } = $props();

  const STEP_NAMES = { 1: 'Criar cofre', 2: 'Senha mestra', 3: 'PIN' };
  const ITEMS = [
    {
      icon: 'lock',
      title: 'Cifrado em repouso',
      text: 'Argon2id e XChaCha20-Poly1305. Sem a senha mestra, o arquivo não abre.',
    },
    {
      icon: 'type-server',
      title: 'Fica só nesta máquina',
      text: 'Em ~/.local/share/ward/, sem nuvem e sem sincronizar.',
    },
    {
      icon: 'clock',
      title: 'Trava sozinho',
      text: 'Ao bloquear a tela, ao suspender o computador e depois de 4 h sem uso.',
    },
  ];

  let step = $state(1);
  let password = $state('');
  let confirmation = $state('');
  let pin = $state('');
  let pinConfirmation = $state('');
  // The error keys are the wire ids of the fields of the create vault command.
  let errors = $state({ 'senha': '', 'confirmacao': '', pin: '', 'confirmacao_pin': '' });
  let failure = $state('');
  let sending = $state(false);

  let passwordInput = $state();
  let confirmationInput = $state();

  const strength = $derived(passwordStrength(password));

  function clearErrors() {
    errors = { 'senha': '', 'confirmacao': '', pin: '', 'confirmacao_pin': '' };
    failure = '';
  }

  function clearValues() {
    password = '';
    confirmation = '';
    pin = '';
    pinConfirmation = '';
  }

  onDestroy(clearValues);

  async function focusOn(id) {
    await tick();
    document.getElementById(id)?.focus();
  }

  async function goTo(n) {
    clearErrors();
    step = n;
    await focusOn({ 1: 'first-use-create', 2: 'first-use-password', 3: 'first-use-pin' }[n]);
  }

  // Step 2: Enter (or "Continue") moves on. With a valid password and an empty
  // confirmation, the focus goes down to the confirmation without an error.
  async function continueStep(e) {
    e.preventDefault();
    clearErrors();
    const short = passwordError(password);
    if (short) {
      errors.senha = short;
      passwordInput?.focus();
      return;
    }
    if (!confirmation) {
      confirmationInput?.focus();
      return;
    }
    if (confirmation !== password) {
      errors.confirmacao = MESSAGES.passwordMismatch;
      confirmationInput?.focus();
      return;
    }
    await goTo(3);
  }

  // A letter, a symbol or a 7th digit in the PIN: the box does not take it (the PIN
  // field only keeps digits, up to 6) and the refusal shows in the field.
  function pinRefusal(field) {
    return (e) => {
      const text = e.data ?? e.dataTransfer?.getData('text/plain') ?? '';
      if (!text) return;
      const target = e.target;
      const value = field === 'pin' ? pin : pinConfirmation;
      const noSelection = target?.selectionStart === target?.selectionEnd;
      if (/\D/.test(text)) {
        errors[field] = MESSAGES.pinWithLetter;
      } else if (noSelection && value.length + text.length > MAX_PIN_DIGITS) {
        errors[field] = MESSAGES.pinLong;
      } else {
        errors[field] = '';
      }
    };
  }

  async function finish(e) {
    e.preventDefault();
    if (sending) return;
    clearErrors();
    if (!isValidPin(pin)) {
      errors.pin = MESSAGES.pinInvalid;
      await focusOn('first-use-pin');
      return;
    }
    if (!pinConfirmation) {
      await focusOn('first-use-confirm-pin');
      return;
    }
    if (pinConfirmation !== pin) {
      errors.confirmacao_pin = MESSAGES.pinMismatch;
      await focusOn('first-use-confirm-pin');
      return;
    }
    sending = true;
    try {
      const r = await call('criar_cofre', {
        'dados': {
          'senha': password,
          'confirmacao': confirmation,
          pin,
          'confirmacao_pin': pinConfirmation,
        },
      });
      if (r?.criado) {
        clearValues();
        oncreated();
        return;
      }
      for (const error of r?.erros ?? []) {
        if (error.campo in errors) errors[error.campo] = rustMessage(error.motivo);
      }
      if (errors.senha || errors.confirmacao) {
        const ofPassword = { 'senha': errors.senha, 'confirmacao': errors.confirmacao };
        step = 2;
        await tick();
        errors = { ...errors, ...ofPassword };
        (errors.senha ? passwordInput : confirmationInput)?.focus();
      } else {
        await focusOn(errors.pin ? 'first-use-pin' : 'first-use-confirm-pin');
      }
    } catch (err) {
      failure = rustMessage(err?.mensagem) || 'Não foi possível criar o cofre.';
    } finally {
      sending = false;
    }
  }

  focusOn('first-use-create');
</script>

<div class="window">
  <BrandPanel />
  <main class="content">
    {#if step === 1}
      <div class="form wide">
        <StepIndicator step={1} name={STEP_NAMES[1]} />
        <header class="header">
          <h1>Criar seu cofre</h1>
          <p class="description">
            O Ward guarda suas credenciais de trabalho num arquivo cifrado. Leva menos de um
            minuto.
          </p>
        </header>
        <ul class="card">
          {#each ITEMS as item (item.title)}
            <li class="item">
              <span class="avatar"><Icon name={item.icon} size="sm" tone="brand" /></span>
              <span class="texts">
                <span class="item-title">{item.title}</span>
                <span class="item-text">{item.text}</span>
              </span>
            </li>
          {/each}
        </ul>
        <Button id="first-use-create" onclick={() => goTo(2)}>
          Criar cofre
        </Button>
      </div>
    {:else if step === 2}
      <form class="form" onsubmit={continueStep} novalidate>
        <StepIndicator step={2} name={STEP_NAMES[2]} />
        <header class="header">
          <h1>Senha mestra</h1>
          <p class="description">
            É a única chave do cofre. Use uma frase longa que você não usa em nenhum outro
            lugar.
          </p>
        </header>
        <PasswordField
          id="first-use-password"
          label="Senha mestra"
          placeholder="Digite a senha mestra"
          help="Mínimo de 12 caracteres."
          bind:value={password}
          bind:input={passwordInput}
          error={errors.senha}
          oninput={() => (errors.senha = '')}
        />
        <div class="strength" aria-live="polite">
          <div class="bars" aria-hidden="true">
            {#each [1, 2, 3, 4] as n (n)}
              <span class="bar" data-tone={strength && n <= strength.segments ? strength.tone : undefined}
              ></span>
            {/each}
          </div>
          <span class="caption" data-tone={strength?.tone}>
            {strength ? `Força: ${strength.label}` : 'Força: digite a senha'}
          </span>
        </div>
        <PasswordField
          id="first-use-confirm"
          label="Confirmar senha mestra"
          placeholder="Digite a senha mestra de novo"
          bind:value={confirmation}
          bind:input={confirmationInput}
          error={errors.confirmacao}
          oninput={() => (errors.confirmacao = '')}
        />
        <Highlight kind="warning" icon="alert">
          A senha mestra não tem recuperação: se você esquecer, o cofre não abre. Depois,
          imprima o kit de emergência em Configurações.
        </Highlight>
        <div class="actions">
          <Button variant="secondary" onclick={() => goTo(1)}>Voltar</Button>
          <Button type="submit">Continuar</Button>
        </div>
      </form>
    {:else}
      <form class="form" onsubmit={finish} novalidate>
        <StepIndicator step={3} name={STEP_NAMES[3]} />
        <header class="header">
          <h1>PIN de aprovação</h1>
          <p class="description">
            Com o PIN você aprova usos sensíveis e mudanças de nível de acesso. Use de 4 a 6
            dígitos.
          </p>
        </header>
        <div class="pin" onbeforeinput={pinRefusal('pin')}>
          <PinField
            id="first-use-pin"
            label="PIN"
            help="4 a 6 dígitos."
            bind:value={pin}
            error={errors.pin}
          />
        </div>
        <div class="pin" onbeforeinput={pinRefusal('confirmacao_pin')}>
          <PinField
            id="first-use-confirm-pin"
            label="Confirmar PIN"
            help="Digite o mesmo PIN de novo."
            bind:value={pinConfirmation}
            error={errors.confirmacao_pin}
          />
        </div>
        <Highlight kind="info" icon="lock">
          O PIN não destrava o cofre. Ele só aprova usos e mudanças de nível. Cinco erros
          seguidos travam o cofre.
        </Highlight>
        {#if failure}<Toast kind="error" message={failure} />{/if}
        <div class="actions">
          <Button variant="secondary" disabled={sending} onclick={() => goTo(2)}>
            Voltar
          </Button>
          <Button type="submit" disabled={sending}>
            {sending ? 'Criando o cofre…' : 'Concluir'}
          </Button>
        </div>
      </form>
    {/if}
  </main>
</div>

<style>
  .window {
    display: flex;
    gap: var(--space-16);
    height: 100%;
    padding: var(--space-16);
    background: var(--color-bg-default);
  }
  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-width: 0;
    padding: var(--space-48);
    border-radius: var(--radius-panel);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-bg-subtle);
    overflow-y: auto;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
    width: var(--width-column-access);
    margin: var(--space-0);
  }
  .wide {
    gap: var(--space-32);
  }
  .header {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }
  h1 {
    margin: var(--space-0);
    font-weight: var(--weight-semibold);
    font-size: var(--size-xl);
    line-height: var(--line-height-xl);
    letter-spacing: var(--letter-spacing-title);
  }
  .description {
    margin: var(--space-0);
    color: var(--color-text-secondary);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-16);
    margin: var(--space-0);
    padding: var(--space-20);
    list-style: none;
    border-radius: var(--radius-card);
    border: var(--line-thin) solid var(--color-border-subtle);
    background: var(--color-surface-default);
  }
  .item {
    display: flex;
    gap: var(--space-12);
  }
  .avatar {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--control-sm);
    height: var(--control-sm);
    border-radius: var(--radius-control);
    background: var(--color-primary-subtle);
  }
  .texts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .item-title {
    font-weight: var(--weight-medium);
  }
  .item-text {
    font-size: var(--size-sm);
    line-height: var(--line-height-sm);
    color: var(--color-text-secondary);
  }
  .wide > :global(.button) {
    width: 100%;
  }
  .strength {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .bars {
    display: flex;
    gap: var(--space-6);
  }
  .bar {
    flex: 1;
    height: var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--color-border-default);
  }
  .bar[data-tone='danger'] {
    background: var(--color-danger-default);
  }
  .bar[data-tone='warning'] {
    background: var(--color-warning-default);
  }
  .bar[data-tone='success'] {
    background: var(--color-success-default);
  }
  .caption {
    font-size: var(--size-xs);
    line-height: var(--line-height-xs);
    color: var(--color-text-secondary);
  }
  .caption[data-tone='danger'] {
    color: var(--color-danger-text);
  }
  .caption[data-tone='warning'] {
    color: var(--color-warning-text);
  }
  .caption[data-tone='success'] {
    color: var(--color-success-text);
  }
  .actions {
    display: flex;
    gap: var(--space-12);
  }
  .actions :global(.button) {
    flex: 1;
  }
</style>
