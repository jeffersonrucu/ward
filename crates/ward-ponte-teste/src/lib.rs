//! Ponte HTTP de teste (D7): liga a interface Svelte no Chrome ao daemon de teste.
//! Só existe com a feature `modo-teste`; o build de release não a contém e
//! `make install` recusa artefatos com `modo-teste`.
//!
//! Fase 29. A ponte:
//! - escuta só em `127.0.0.1`, porta aleatória, com um token aleatório de 32 bytes que
//!   vai na URL de abertura (`#token=…`, fragmento: nunca chega ao servidor nem a log) e
//!   volta no cabeçalho `Authorization` de cada chamada (comparado em tempo constante);
//! - confere `Host` (contra DNS rebinding) e `Origin` (contra outra página do Chrome);
//! - serve `dist-teste/` (só `index.html` e `assets/<nome>`) com a mesma CSP do app;
//! - expõe em `POST /ponte/<comando>` só os comandos de
//!   `ward_app_nucleo::comandos::COMANDOS`, pelo mesmo `Servico` do Tauri: nenhuma
//!   resposta leva segredo (o "copiar" é resolvido no Rust, D4).

#![forbid(unsafe_code)]

#[cfg(feature = "modo-teste")]
pub mod http;
#[cfg(feature = "modo-teste")]
pub mod simulados;
