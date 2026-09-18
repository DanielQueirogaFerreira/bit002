//! Núcleo da Arquitetura Higher.
//!
//! Este crate é o único lugar onde a codificação acontece: a CLI
//! ([`bitbench`]) e os bindings WASM ([`higher-wasm`]) só o chamam. É assim
//! que o tempo medido no nativo e o comportamento visto no browser vêm do
//! mesmo código (ver `CLAUDE.md`, seção "Stack").
//!
//! Na Etapa E0 só existe o vocabulário do projeto: larguras, faixas,
//! variantes e os limiares derivados de `docs/00-TESE.md §3`. O packing
//! (E1), o modelo paramétrico e o formato `.hgr` (E2) entram nas etapas
//! seguintes.

#![forbid(unsafe_code)]

pub mod model;

/// Versão do crate, propagada para o cabeçalho dos runs (`docs/02 §6.1`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
