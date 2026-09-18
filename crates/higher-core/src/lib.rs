//! Núcleo da Arquitetura Higher.
//!
//! Este crate é o único lugar onde a codificação acontece: a CLI
//! ([`bitbench`]) e os bindings WASM ([`higher-wasm`]) só o chamam. É assim
//! que o tempo medido no nativo e o comportamento visto no browser vêm do
//! mesmo código (ver `CLAUDE.md`, seção "Stack").
//!
//! O que existe até aqui:
//!
//! - [`model`] — o vocabulário do projeto: larguras, faixas, variantes e os
//!   limiares derivados de `docs/00-TESE.md §3` (E0);
//! - [`packing`] — `BitWriter`/`BitReader` MSB-first para `b` de 1 a 32 (E1).
//!
//! O modelo paramétrico com tabela e o formato `.hgr` são a E2.

#![forbid(unsafe_code)]

pub mod model;
pub mod packing;

/// Versão do crate, propagada para o cabeçalho dos runs (`docs/02 §6.1`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
