// Textos do `Kit de emergência / Impressão` (FigCraft 16773, Fase 49; V7.10, D17).
//
// A folha diz onde ficam o cofre, as cópias locais e o pendrive, e deixa linhas em
// branco para a senha mestra escrita à mão. Nenhum segredo entra aqui: as pastas são as
// fixas da instalação (D17, D18), e do daemon vem só se o backup externo está ligado
// (e, quando houver, o UUID e o rótulo do pendrive, que não são segredo). Nada daqui
// guarda valor digitado, e nada vai para armazenamento, URL ou arquivo.

// Pastas da instalação (D17, D18): as mesmas que `Configurações › Backup local` mostra.
export const PASTA_COFRE = '~/.local/share/ward/';
export const PASTA_BACKUPS = '~/.local/share/ward/backups/';
// Pasta das cópias dentro do pendrive (D17: `<montagem>/ward-backup/`).
export const PASTA_NO_PENDRIVE = 'ward-backup/';

export const PASSOS_RESTAURAR = Object.freeze([
  'Instale o Ward e crie ou destrave um cofre.',
  `Se esta máquina parou, copie um arquivo ward-AAAAMMDD-HHMMSS.ward do pendrive para ${PASTA_BACKUPS}.`,
  'No app, abra Configurações › Backup local › Restaurar e escolha a versão.',
  'Digite a senha mestra que valia naquela versão.',
]);

// UUID de sistema de arquivos no formato de D45 (`[0-9A-Fa-f-]`, 1 a 36, sem `-` nas
// pontas). O Rust já confere; aqui é só para nunca imprimir outra coisa como UUID.
const UUID = /^[0-9A-Fa-f](?:[0-9A-Fa-f-]{0,34}[0-9A-Fa-f])?$/;
// Rótulo de pendrive: o Rust já neutralizou (D51) e cortou; aqui ele é só texto, e
// qualquer controle que sobrasse vira espaço.
const MAX_ROTULO = 64;
const CONTROLE = /[\u0000-\u001f\u007f-\u009f]/g;

/** "Impresso em 08/11/2026", na data local de `agora`. */
export function dataImpressa(agora) {
  const d = agora instanceof Date && !Number.isNaN(agora.getTime()) ? agora : new Date();
  const dois = (n) => String(n).padStart(2, '0');
  return `Impresso em ${dois(d.getDate())}/${dois(d.getMonth() + 1)}/${d.getFullYear()}`;
}

function rotuloLimpo(r) {
  if (typeof r !== 'string') return '';
  return r.replace(CONTROLE, ' ').trim().slice(0, MAX_ROTULO);
}

/**
 * Item "Pendrive" da seção "Onde estão os backups". `externo` é `{ ligado, uuid?,
 * pendrive?: { rotulo } }` (só `ligado` quando o app ainda não lê o UUID).
 */
export function linhaDoPendrive(externo) {
  if (externo?.ligado !== true) {
    return {
      rotulo: 'Pendrive · backup externo desligado',
      valor: 'Nenhuma cópia em pendrive. Ligue em Configurações › Backup externo em pendrive.',
      mono: false,
    };
  }
  const partes = [];
  const nome = rotuloLimpo(externo.pendrive?.rotulo);
  if (nome) partes.push(nome);
  const uuid = typeof externo.uuid === 'string' && UUID.test(externo.uuid) ? externo.uuid : '';
  if (uuid) partes.push(`UUID ${uuid}`);
  partes.push(`pasta ${PASTA_NO_PENDRIVE}`);
  return { rotulo: 'Pendrive · copiado sempre que é conectado', valor: partes.join(' · '), mono: true };
}
