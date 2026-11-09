//! Limites do cofre numa fonte só (D25, D29, D32).
//!
//! A validação da entrada ([`crate::entrada`]), a leitura e a escrita do corpo
//! ([`crate::corpo`]) e a máscara ([`crate::mascara`]) usam estas constantes; o protocolo
//! (Fase 20) e o app (Fase 34) também. Nenhum desses módulos repete um número daqui.
//! Limite não é segredo: tudo é `pub`.
//!
//! Tudo o que chega do socket, da extensão e do disco é não confiável; estes limites
//! são conferidos antes de alocar (corpo) e ao salvar (validação).

/// Bytes em 1 KiB.
pub const KIB: usize = 1024;
/// Bytes em 1 MiB.
pub const MIB: usize = 1024 * KIB;

// ---- Texto (bytes de UTF-8) ----

/// Nome da entrada (slug), da variável do `api` e da referência de túnel/salto.
pub const MAX_NOME: usize = 64;
/// Texto comum de uma linha (grupo, projeto, usuário…).
pub const MAX_LINHA: usize = 256;
/// Usuário de qualquer tipo (o do `servidor` e do `ftp/sftp` ainda segue o formato de
/// D43, `[A-Za-z0-9._-]`).
pub const MAX_USUARIO: usize = MAX_LINHA;
/// URL e role ARN: o maior texto de D25.
pub const MAX_URL: usize = 2048;
/// Host (o limite do DNS).
pub const MAX_HOST: usize = 253;
/// Uma tag.
pub const MAX_TAG: usize = 32;
/// Nome de um campo extra.
pub const MAX_NOME_EXTRA: usize = 64;
/// Nome de um anexo.
pub const MAX_NOME_ANEXO: usize = 128;
/// Access key ID da AWS: de 16 a 128 caracteres.
pub const MIN_ACCESS_KEY: usize = 16;
/// Access key ID da AWS: de 16 a 128 caracteres.
pub const MAX_ACCESS_KEY: usize = 128;
/// Região da AWS (ex.: `sa-east-1`).
pub const MAX_REGIAO: usize = 32;
/// Base do `banco` (todo motor menos `redis`): `[A-Za-z0-9_-]`, de 1 a 63 (o limite do
/// Postgres, D33).
pub const MAX_BASE: usize = 63;
/// Base do `redis`: número de 0 a 99999 (D33).
pub const MAX_BASE_REDIS: u32 = 99_999;

/// Maior texto de qualquer campo comum: a leitura do corpo recusa acima disto antes
/// de alocar, e a validação confere de novo com o limite exato de cada campo.
pub const MAX_TEXTO: usize = MAX_URL;

// ---- Segredos ----

/// Um segredo de texto (senha, token, chave privada, passphrase…).
pub const MAX_SEGREDO: usize = 64 * KIB;
/// Valor de um campo extra (secreto ou não).
pub const MAX_VALOR_EXTRA: usize = MAX_SEGREDO;
/// Texto da nota segura.
pub const MAX_NOTA: usize = MIB;
/// Semente TOTP em base32: no mínimo 80 bits (16 caracteres, D35).
pub const MIN_TOTP: usize = 16;
/// Semente TOTP em base32: no máximo 128 caracteres.
pub const MAX_TOTP: usize = 128;

// ---- Listas ----

/// Tags por entrada.
pub const MAX_TAGS: usize = 16;
/// Campos extras por entrada.
pub const MAX_EXTRAS: usize = 64;
/// Anexos por entrada.
pub const MAX_ANEXOS: usize = 32;

// ---- Anexos e máscara (D29) ----

/// Soma dos anexos de uma entrada (e, logo, cada anexo): 256 KiB. Chave SSH, `.pem`,
/// `.env`, `.ovpn` e kubeconfig ficam muito abaixo.
pub const MAX_ANEXOS_SOMADOS: usize = 256 * KIB;

/// Limite da soma dos padrões da máscara montada (D11). Acima disto a máscara não é
/// montada e o comando não roda (falha fechada).
pub const LIMITE_PADROES: usize = 8 * MIB;

/// Orçamento de padrões da máscara por entrada com entrega (D29): metade do limite,
/// para que as duas entradas de um uso combinado (salto ou túnel, D19) caibam juntas.
pub const ORCAMENTO_MASCARA: usize = LIMITE_PADROES / 2;

/// Tamanho, em caracteres, que um valor precisa ter para a máscara ser confiável (D11).
/// Também é o mínimo de uma linha de anexo e do valor de `CHAVE=valor` (D34) para virar
/// padrão próprio.
pub const MINIMO_CONFIAVEL: usize = 6;

// ---- Corpo (D26) ----

/// Maior corpo decifrado aceito, em bytes (provisório).
pub const MAX_CORPO: usize = 64 * MIB;
/// Maior número de entradas num cofre.
pub const MAX_ENTRADAS: usize = 10_000;

// ---- Configuração do cofre (D30) ----

/// PIN: de 4 a 6 dígitos ("Aprovação").
pub const MIN_DIGITOS_PIN: usize = 4;
/// PIN: de 4 a 6 dígitos ("Aprovação").
pub const MAX_DIGITOS_PIN: usize = 6;
/// Hash do PIN em texto PHC (`$argon2id$v=19$m=…,t=…,p=…$<sal>$<hash>`), em bytes. O
/// hash de hoje tem cerca de 100; a leitura recusa acima disto antes de qualquer coisa.
pub const MAX_HASH_PIN: usize = 128;
/// Tempo de trava por inatividade padrão: 4 h (R23, D16).
pub const MINUTOS_TRAVA_PADRAO: u32 = 240;
/// Menor tempo de trava aceito, em minutos. Não existe "nunca travar".
pub const MIN_MINUTOS_TRAVA: u32 = 1;
/// Maior tempo de trava aceito, em minutos: 24 h.
pub const MAX_MINUTOS_TRAVA: u32 = 24 * 60;
/// UUID do sistema de arquivos do pendrive do backup externo (D17): até 36 caracteres
/// (o UUID de 128 bits com hífens; FAT e exFAT usam `XXXX-XXXX`).
pub const MAX_UUID_BACKUP: usize = 36;

// ---- Protocolo do socket (Fase 20) ----

/// Soma dos bytes secretos que uma entrada pode ter (nota, dois segredos de campo,
/// os valores dos extras e os anexos somados). Dimensiona o quadro do app.
pub const MAX_ENTRADA: usize =
    MAX_NOTA + 2 * MAX_SEGREDO + MAX_EXTRAS * MAX_VALOR_EXTRA + MAX_ANEXOS_SOMADOS;
/// Soma dos textos comuns de uma entrada (com folga: 32 campos no limite de texto).
pub const MAX_TEXTOS_ENTRADA: usize =
    32 * MAX_TEXTO + MAX_TAGS * MAX_TAG + MAX_EXTRAS * MAX_NOME_EXTRA + MAX_ANEXOS * MAX_NOME_ANEXO;
/// Maior quadro do cliente comum (agente) e do host da extensão (V3.4). Cabe o
/// `ward run` com o argv e o ambiente da CLI; um quadro de 2 MiB é recusado.
pub const MAX_QUADRO_COMUM: usize = MIB;
/// Maior quadro do app já identificado (D5, D29): a maior entrada que a validação
/// aceita, com os segredos e os 256 KiB de anexos em base64. O módulo do protocolo
/// confere na compilação que ela cabe.
pub const MAX_QUADRO_APP: usize = 8 * MIB;
/// Maior quadro do daemon para o cliente: a lista de um cofre cheio (o texto das
/// entradas soma no máximo o corpo, e o escape do JSON no máximo dobra) com folga por
/// entrada.
pub const MAX_QUADRO_RESPOSTA: usize = 2 * MAX_CORPO + MAX_ENTRADAS * KIB;
/// Um argumento ou valor de variável de ambiente do `ward run` (o `MAX_ARG_STRLEN` do
/// Linux).
pub const MAX_ARGUMENTO: usize = 128 * KIB;
/// Argumentos do comando do `ward run`/`ward ssh`.
pub const MAX_ARGUMENTOS: usize = 2048;
/// Variáveis do ambiente que a CLI repassa.
pub const MAX_VARIAVEIS_AMBIENTE: usize = 2048;
/// Texto mostrado na janela de aprovação (o comando exato) e um pedaço de saída.
pub const MAX_TEXTO_EXIBIDO: usize = 64 * KIB;

// Coerência entre os limites, conferida na compilação.
const _: () = {
    assert!(MAX_NOME <= MAX_TEXTO);
    assert!(MAX_LINHA <= MAX_TEXTO);
    assert!(MAX_USUARIO <= MAX_TEXTO);
    assert!(MAX_HOST <= MAX_TEXTO);
    assert!(MAX_TAG <= MAX_TEXTO);
    assert!(MAX_NOME_EXTRA <= MAX_TEXTO);
    assert!(MAX_NOME_ANEXO <= MAX_TEXTO);
    assert!(MAX_ACCESS_KEY <= MAX_TEXTO);
    assert!(MAX_REGIAO <= MAX_TEXTO);
    assert!(MAX_BASE <= MAX_TEXTO);
    assert!(MIN_ACCESS_KEY <= MAX_ACCESS_KEY);
    assert!(MIN_TOTP <= MAX_TOTP);
    assert!(MAX_TOTP <= MAX_SEGREDO);
    assert!(ORCAMENTO_MASCARA * 2 <= LIMITE_PADROES);
    assert!(MIN_DIGITOS_PIN <= MAX_DIGITOS_PIN);
    assert!(MIN_MINUTOS_TRAVA <= MINUTOS_TRAVA_PADRAO);
    assert!(MINUTOS_TRAVA_PADRAO <= MAX_MINUTOS_TRAVA);
    assert!(MAX_HASH_PIN <= MAX_TEXTO);
    assert!(MAX_UUID_BACKUP <= MAX_TEXTO);
    assert!(MAX_QUADRO_COMUM < MAX_QUADRO_APP);
    assert!(MAX_QUADRO_RESPOSTA <= u32::MAX as usize);
    assert!(MAX_ARGUMENTO < MAX_QUADRO_COMUM);
    // O comprimento de todo campo cabe no u32 do corpo.
    assert!(MAX_CORPO <= u32::MAX as usize);
};

#[cfg(test)]
mod testes;
