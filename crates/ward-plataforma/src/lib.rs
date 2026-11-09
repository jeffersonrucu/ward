//! Tudo o que depende do sistema (Linux): iniciar com a sessão, bloqueio de tela e
//! suspensão, socket local, `SO_PEERCRED`, agente SSH temporário, proteção de memória
//! e identificação do processo que chama (R3).
//!
//! Único crate onde `unsafe` pode aparecer, sempre com `#[allow(unsafe_code)]` no
//! item, comentário `// SAFETY:` e teste. Vazio na Fase 11; o conteúdo chega na
//! Fase 21.
