//! Comandos próprios do app: a única lista do que a interface pode pedir (S11).
//!
//! O `build.rs` do Tauri registra exatamente estes nomes no manifesto do app (sem ele,
//! todo comando registrado ficaria liberado), a capability `principal` libera só as
//! permissões `allow-<comando>` de [`COMANDOS_PRINCIPAL`], a capability `aprovacao`
//! (janela de aprovação, Fase 36) só as de [`COMANDOS_APROVACAO`], e a ponte de teste
//! recusa qualquer outro nome. Em comum, só o `ouvir_avisos` (canal por janela, aviso
//! sem dado): a janela principal não responde aprovação, e a de aprovação não lê
//! credencial nem revela nada.
//! Fases seguintes acrescentam comandos aqui, um por tela que precisa.

use ward_core::limites;

use crate::ErroApp;

/// `estado_do_cofre`: travado, destravado ou sem cofre. Sem argumento.
pub const ESTADO_DO_COFRE: &str = "estado_do_cofre";
/// `copiar_campo { entrada, campo }`: o valor vai do daemon para a área de
/// transferência pelo lado Rust (D4); a resposta não traz o valor.
pub const COPIAR_CAMPO: &str = "copiar_campo";

/// `listar_credenciais`: as entradas para a tela Credenciais (Fase 32), sem segredo.
/// Sem argumento.
pub const LISTAR_CREDENCIAIS: &str = "listar_credenciais";
/// `detalhe_credencial { entrada }`: o painel de detalhe de uma entrada (Fase 32): campos
/// de texto, só os nomes dos segredos, extras sem o valor secreto e anexos sem o
/// conteúdo.
pub const DETALHE_CREDENCIAL: &str = "detalhe_credencial";
/// `gerar_senha { tamanho, minusculas, maiusculas, digitos, simbolos }`: gerador avulso
/// (Fase 37). A senha fica no lado Rust; a resposta traz só tamanho, força e símbolos.
pub const GERAR_SENHA: &str = "gerar_senha";
/// `copiar_senha_gerada`: a senha avulsa vai para a área de transferência (D4); a
/// resposta não traz o valor.
pub const COPIAR_SENHA_GERADA: &str = "copiar_senha_gerada";
/// `revelar_senha_gerada`: o valor, só no clique em "revelar" (S11).
pub const REVELAR_SENHA_GERADA: &str = "revelar_senha_gerada";
/// `descartar_senha_gerada`: zera a senha avulsa ao sair da tela.
pub const DESCARTAR_SENHA_GERADA: &str = "descartar_senha_gerada";
/// `revelar_campo { entrada, campo }`: o valor de um segredo da entrada, só no clique em
/// "revelar" (S11, D65 b). A tela o tira em 30 s, ao esconder ou ao sair.
pub const REVELAR_CAMPO: &str = "revelar_campo";
/// `codigo_totp { entrada }`: o código TOTP atual e os segundos até trocar (Fase 33).
/// A semente nunca vai à interface.
pub const CODIGO_TOTP: &str = "codigo_totp";
/// `copiar_codigo_totp { entrada }`: o código vai para a área de transferência (D4).
pub const COPIAR_CODIGO_TOTP: &str = "copiar_codigo_totp";
/// `usar_senha_gerada { campo }`: "Usar esta senha" do gerador do formulário. A senha
/// passa ao campo no lado Rust; a resposta traz só o tamanho.
pub const USAR_SENHA_GERADA: &str = "usar_senha_gerada";
/// `descartar_formulario`: ao cancelar ou sair do formulário, zera as senhas usadas.
pub const DESCARTAR_FORMULARIO: &str = "descartar_formulario";
/// `salvar_credencial { anterior, formulario }`: cria ou edita uma entrada (Fase 33).
/// Segredo mantido e senha gerada são postos no rascunho pelo lado Rust.
pub const SALVAR_CREDENCIAL: &str = "salvar_credencial";
/// `mudar_nivel { entrada, nivel, pin }`: muda o nível de acesso com o PIN (Fase 35,
/// S9). O PIN vai só ao daemon; a resposta traz o nível novo.
pub const MUDAR_NIVEL: &str = "mudar_nivel";
/// `escolher_anexo`: o lado Rust escolhe e lê o arquivo (Fase 34); a resposta traz id,
/// nome e tamanho, ou o aviso. Sem argumento: a interface não escolhe caminho.
pub const ESCOLHER_ANEXO: &str = "escolher_anexo";
/// `remover_anexo { id }`: tira um anexo escolhido e ainda não salvo.
pub const REMOVER_ANEXO: &str = "remover_anexo";
/// `carregar_chave { campo }`: "Carregar de arquivo" do campo "Chave privada" (Fase
/// 53a). O lado Rust escolhe, lê e confere o arquivo e guarda a chave até salvar; a
/// resposta traz só o tamanho, ou o aviso. A interface não manda caminho.
pub const CARREGAR_CHAVE: &str = "carregar_chave";
/// `conectar { entrada }`: "Conectar" do `servidor` (D3); o token não volta.
pub const CONECTAR: &str = "conectar";
/// `criar_cofre { dados: { senha, confirmacao, pin, confirmacao_pin } }`: primeiro uso
/// (Fase 30). Conferido no lado Rust; o daemon grava só o hash do PIN (D30). A resposta
/// traz `criado` ou os erros por campo, nunca o valor.
pub const CRIAR_COFRE: &str = "criar_cofre";
/// `destravar { senha }`: destrava com a senha mestra (Fase 31). A senha vai só ao
/// daemon, num tipo que zera; a resposta não a traz nem diz quantas tentativas houve.
pub const DESTRAVAR: &str = "destravar";
/// `travar`: "Travar agora" da `Navegação lateral` (Fase 39; a bandeja trava pelo lado
/// Rust). Sem argumento; limpa a área de transferência e o formulário antes.
pub const TRAVAR: &str = "travar";
/// `ouvir_avisos { canal }`: o canal do Tauri por onde o lado Rust avisa a tela de um
/// `pedido_de_destrave` (Fase 31, D65 a). O aviso leva só o nome dele, sem dado.
pub const OUVIR_AVISOS: &str = "ouvir_avisos";
/// `pedido_de_aprovacao`: o pedido da vez para a janela de aprovação (Fase 36), com o
/// texto do agente já neutralizado (D51), ou `null`. Só a janela `aprovacao`.
pub const PEDIDO_DE_APROVACAO: &str = "pedido_de_aprovacao";
/// `responder_aprovacao { id, decisao, pin }`: Negar (sem PIN) ou um dos três Permitir
/// (com PIN, S9, S10). Só a janela `aprovacao`; a resposta não traz o PIN.
pub const RESPONDER_APROVACAO: &str = "responder_aprovacao";

/// `configuracoes`: tempo de trava e backup externo ligado ou não (Fase 38). Sem
/// argumento; sem hash de PIN nem UUID.
pub const CONFIGURACOES: &str = "configuracoes";
/// `trocar_pin { dados: { atual, novo, confirmacao } }`: troca o PIN pedindo o atual
/// (Fase 38, V5.22, S9). A resposta traz `trocado` ou os erros por campo, nunca o valor.
pub const TROCAR_PIN: &str = "trocar_pin";
/// `trocar_senha_mestra { dados: { atual, nova, confirmacao } }`: troca a senha mestra
/// pedindo a atual (Fase 38, V5.22). A resposta traz `trocado` ou os erros por campo.
pub const TROCAR_SENHA_MESTRA: &str = "trocar_senha_mestra";
/// `definir_tempo_trava { minutos }`: tempo de trava por inatividade, gravado na
/// configuração do cofre (Fase 38, D30, D45).
pub const DEFINIR_TEMPO_TRAVA: &str = "definir_tempo_trava";
/// `backup_externo`: ligado ou não, o UUID escolhido e, se conectado, o pendrive (rótulo
/// neutralizado, D51). Nunca o caminho de montagem (Fase 48, V7.9).
pub const BACKUP_EXTERNO: &str = "backup_externo";
/// `listar_pendrives`: os pendrives que o daemon vê, para escolher (Fase 48).
pub const LISTAR_PENDRIVES: &str = "listar_pendrives";
/// `definir_backup_externo { ligado, uuid? }`: ligar com um pendrive que o daemon vê
/// agora (o lado Rust confere de novo) ou desligar mantendo o UUID (Fase 48, D17, D30).
pub const DEFINIR_BACKUP_EXTERNO: &str = "definir_backup_externo";
/// `listar_backups`: as versões do backup local (nome e instante, nunca conteúdo), da
/// mais nova para a mais antiga (Fase 47, V7.7).
pub const LISTAR_BACKUPS: &str = "listar_backups";
/// `restaurar_backup { dados: { versao, senha, pin } }`: restaura uma versão com a senha
/// mestra dela e o PIN atual (Fase 47, V7.7; D17, D39, S9). A resposta traz só
/// `restaurado` e a contagem de entradas.
pub const RESTAURAR_BACKUP: &str = "restaurar_backup";

/// Comandos da janela principal, na ordem da capability `principal`.
pub const COMANDOS_PRINCIPAL: &[&str] = &[
    ESTADO_DO_COFRE,
    COPIAR_CAMPO,
    LISTAR_CREDENCIAIS,
    DETALHE_CREDENCIAL,
    GERAR_SENHA,
    COPIAR_SENHA_GERADA,
    REVELAR_SENHA_GERADA,
    DESCARTAR_SENHA_GERADA,
    REVELAR_CAMPO,
    CODIGO_TOTP,
    COPIAR_CODIGO_TOTP,
    USAR_SENHA_GERADA,
    DESCARTAR_FORMULARIO,
    SALVAR_CREDENCIAL,
    MUDAR_NIVEL,
    ESCOLHER_ANEXO,
    REMOVER_ANEXO,
    CARREGAR_CHAVE,
    CONECTAR,
    CRIAR_COFRE,
    DESTRAVAR,
    OUVIR_AVISOS,
    CONFIGURACOES,
    TROCAR_PIN,
    TROCAR_SENHA_MESTRA,
    DEFINIR_TEMPO_TRAVA,
    BACKUP_EXTERNO,
    LISTAR_PENDRIVES,
    DEFINIR_BACKUP_EXTERNO,
    LISTAR_BACKUPS,
    RESTAURAR_BACKUP,
    TRAVAR,
];

/// Comandos da janela de aprovação, na ordem da capability `aprovacao` (Fase 36). O
/// `ouvir_avisos` é o único nas duas janelas: o canal é por janela e o aviso não leva
/// dado (a principal recebe `pedido-de-destrave`; a de aprovação, `pedido-de-aprovacao`).
pub const COMANDOS_APROVACAO: &[&str] = &[OUVIR_AVISOS, PEDIDO_DE_APROVACAO, RESPONDER_APROVACAO];

/// Os comandos que as duas janelas têm (só o canal de avisos, sem dado).
pub const COMANDOS_DAS_DUAS: &[&str] = &[OUVIR_AVISOS];

/// Todos os comandos (manifesto do app e ponte de teste): os da janela principal e os da
/// janela de aprovação.
pub const COMANDOS: &[&str] = &[
    ESTADO_DO_COFRE,
    COPIAR_CAMPO,
    LISTAR_CREDENCIAIS,
    DETALHE_CREDENCIAL,
    GERAR_SENHA,
    COPIAR_SENHA_GERADA,
    REVELAR_SENHA_GERADA,
    DESCARTAR_SENHA_GERADA,
    REVELAR_CAMPO,
    CODIGO_TOTP,
    COPIAR_CODIGO_TOTP,
    USAR_SENHA_GERADA,
    DESCARTAR_FORMULARIO,
    SALVAR_CREDENCIAL,
    MUDAR_NIVEL,
    ESCOLHER_ANEXO,
    REMOVER_ANEXO,
    CARREGAR_CHAVE,
    CONECTAR,
    CRIAR_COFRE,
    DESTRAVAR,
    OUVIR_AVISOS,
    CONFIGURACOES,
    TROCAR_PIN,
    TROCAR_SENHA_MESTRA,
    DEFINIR_TEMPO_TRAVA,
    BACKUP_EXTERNO,
    LISTAR_PENDRIVES,
    DEFINIR_BACKUP_EXTERNO,
    LISTAR_BACKUPS,
    RESTAURAR_BACKUP,
    PEDIDO_DE_APROVACAO,
    RESPONDER_APROVACAO,
    TRAVAR,
];

/// Nome da permissão gerada pelo Tauri para o comando (`allow-estado-do-cofre`).
pub fn permissao(comando: &str) -> String {
    format!("allow-{}", comando.replace('_', "-"))
}

/// Comando conhecido, a partir do nome que veio da interface (não confiável).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comando {
    EstadoDoCofre,
    CopiarCampo,
    ListarCredenciais,
    DetalheCredencial,
    GerarSenha,
    CopiarSenhaGerada,
    RevelarSenhaGerada,
    DescartarSenhaGerada,
    RevelarCampo,
    CodigoTotp,
    CopiarCodigoTotp,
    UsarSenhaGerada,
    DescartarFormulario,
    SalvarCredencial,
    MudarNivel,
    EscolherAnexo,
    RemoverAnexo,
    CarregarChave,
    Conectar,
    CriarCofre,
    Destravar,
    OuvirAvisos,
    Configuracoes,
    TrocarPin,
    TrocarSenhaMestra,
    DefinirTempoTrava,
    BackupExterno,
    ListarPendrives,
    DefinirBackupExterno,
    ListarBackups,
    RestaurarBackup,
    PedidoDeAprovacao,
    ResponderAprovacao,
    Travar,
}

impl Comando {
    pub fn de_nome(nome: &str) -> Option<Comando> {
        match nome {
            ESTADO_DO_COFRE => Some(Comando::EstadoDoCofre),
            COPIAR_CAMPO => Some(Comando::CopiarCampo),
            LISTAR_CREDENCIAIS => Some(Comando::ListarCredenciais),
            DETALHE_CREDENCIAL => Some(Comando::DetalheCredencial),
            GERAR_SENHA => Some(Comando::GerarSenha),
            COPIAR_SENHA_GERADA => Some(Comando::CopiarSenhaGerada),
            REVELAR_SENHA_GERADA => Some(Comando::RevelarSenhaGerada),
            DESCARTAR_SENHA_GERADA => Some(Comando::DescartarSenhaGerada),
            REVELAR_CAMPO => Some(Comando::RevelarCampo),
            CODIGO_TOTP => Some(Comando::CodigoTotp),
            COPIAR_CODIGO_TOTP => Some(Comando::CopiarCodigoTotp),
            USAR_SENHA_GERADA => Some(Comando::UsarSenhaGerada),
            DESCARTAR_FORMULARIO => Some(Comando::DescartarFormulario),
            SALVAR_CREDENCIAL => Some(Comando::SalvarCredencial),
            MUDAR_NIVEL => Some(Comando::MudarNivel),
            ESCOLHER_ANEXO => Some(Comando::EscolherAnexo),
            REMOVER_ANEXO => Some(Comando::RemoverAnexo),
            CARREGAR_CHAVE => Some(Comando::CarregarChave),
            CONECTAR => Some(Comando::Conectar),
            CRIAR_COFRE => Some(Comando::CriarCofre),
            DESTRAVAR => Some(Comando::Destravar),
            OUVIR_AVISOS => Some(Comando::OuvirAvisos),
            CONFIGURACOES => Some(Comando::Configuracoes),
            TROCAR_PIN => Some(Comando::TrocarPin),
            TROCAR_SENHA_MESTRA => Some(Comando::TrocarSenhaMestra),
            DEFINIR_TEMPO_TRAVA => Some(Comando::DefinirTempoTrava),
            BACKUP_EXTERNO => Some(Comando::BackupExterno),
            LISTAR_PENDRIVES => Some(Comando::ListarPendrives),
            DEFINIR_BACKUP_EXTERNO => Some(Comando::DefinirBackupExterno),
            LISTAR_BACKUPS => Some(Comando::ListarBackups),
            RESTAURAR_BACKUP => Some(Comando::RestaurarBackup),
            PEDIDO_DE_APROVACAO => Some(Comando::PedidoDeAprovacao),
            RESPONDER_APROVACAO => Some(Comando::ResponderAprovacao),
            TRAVAR => Some(Comando::Travar),
            _ => None,
        }
    }
}

/// Qual campo de qual entrada. Validado antes de chegar ao daemon: nome da entrada até
/// `MAX_NOME`, nome do campo até `MAX_NOME_EXTRA`, sem controle e sem espaço nas
/// pontas. O daemon valida de novo (a entrada também não é confiável lá).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Referencia {
    entrada: String,
    campo: String,
}

impl Referencia {
    pub fn nova(entrada: &str, campo: &str) -> Result<Referencia, ErroApp> {
        if !nome_valido(entrada, limites::MAX_NOME) || !nome_valido(campo, limites::MAX_NOME_EXTRA)
        {
            return Err(ErroApp::PedidoInvalido);
        }
        Ok(Referencia {
            entrada: entrada.to_owned(),
            campo: campo.to_owned(),
        })
    }

    pub fn entrada(&self) -> &str {
        &self.entrada
    }

    pub fn campo(&self) -> &str {
        &self.campo
    }
}

/// Nome de entrada vindo da interface (não confiável), com a mesma regra da
/// [`Referencia`].
pub fn validar_nome_entrada(nome: &str) -> Result<(), ErroApp> {
    if nome_valido(nome, limites::MAX_NOME) {
        Ok(())
    } else {
        Err(ErroApp::PedidoInvalido)
    }
}

fn nome_valido(nome: &str, max: usize) -> bool {
    !nome.is_empty()
        && nome.chars().count() <= max
        && nome.trim() == nome
        && !nome.chars().any(|c| c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_e_permissoes() {
        assert_eq!(
            COMANDOS,
            &[
                "estado_do_cofre",
                "copiar_campo",
                "listar_credenciais",
                "detalhe_credencial",
                "gerar_senha",
                "copiar_senha_gerada",
                "revelar_senha_gerada",
                "descartar_senha_gerada",
                "revelar_campo",
                "codigo_totp",
                "copiar_codigo_totp",
                "usar_senha_gerada",
                "descartar_formulario",
                "salvar_credencial",
                "mudar_nivel",
                "escolher_anexo",
                "remover_anexo",
                "carregar_chave",
                "conectar",
                "criar_cofre",
                "destravar",
                "ouvir_avisos",
                "configuracoes",
                "trocar_pin",
                "trocar_senha_mestra",
                "definir_tempo_trava",
                "backup_externo",
                "listar_pendrives",
                "definir_backup_externo",
                "listar_backups",
                "restaurar_backup",
                "pedido_de_aprovacao",
                "responder_aprovacao",
                "travar",
            ]
        );
        // As duas janelas juntas são todos; em comum, só o canal de avisos.
        for c in COMANDOS {
            assert!(
                COMANDOS_PRINCIPAL.contains(c) || COMANDOS_APROVACAO.contains(c),
                "{c}"
            );
            assert_eq!(
                COMANDOS_PRINCIPAL.contains(c) && COMANDOS_APROVACAO.contains(c),
                COMANDOS_DAS_DUAS.contains(c),
                "{c}"
            );
        }
        assert_eq!(
            COMANDOS_APROVACAO,
            &["ouvir_avisos", "pedido_de_aprovacao", "responder_aprovacao"]
        );
        assert_eq!(permissao(COPIAR_CAMPO), "allow-copiar-campo");
        for c in COMANDOS {
            assert!(Comando::de_nome(c).is_some());
        }
    }

    #[test]
    fn ataque_comando_inventado_ou_de_plugin_nao_existe() {
        for nome in [
            "",
            "plugin:fs|read_text_file",
            "plugin:shell|execute",
            "plugin:http|fetch",
            "revelar_tudo",
            "COPIAR_CAMPO",
            "copiar_campo ",
            "copiar-campo",
            "revelar",
            "detalhe",
            "lista",
            "detalhe_credencial ",
        ] {
            assert_eq!(Comando::de_nome(nome), None, "{nome}");
        }
    }

    #[test]
    fn ataque_referencia_fora_do_formato() {
        let longo = "a".repeat(limites::MAX_NOME + 1);
        let longo_campo = "a".repeat(limites::MAX_NOME_EXTRA + 1);
        for (e, c) in [
            ("", "senha"),
            ("acme-pg-dev", ""),
            (longo.as_str(), "senha"),
            ("acme-pg-dev", longo_campo.as_str()),
            (" acme-pg-dev", "senha"),
            ("acme\npg", "senha"),
            ("acme-pg-dev", "se\u{0}nha"),
            ("acme\u{202E}gp", "senha"),
            ("acme-pg-dev", "senha\u{200B}"),
        ] {
            assert_eq!(
                Referencia::nova(e, c),
                Err(ErroApp::PedidoInvalido),
                "{e:?} {c:?}"
            );
        }
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert_eq!((r.entrada(), r.campo()), ("acme-pg-dev", "senha"));
        assert!(Referencia::nova(&"á".repeat(limites::MAX_NOME), "senha").is_ok());
    }
}
