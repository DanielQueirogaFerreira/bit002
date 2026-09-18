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
//! - [`packing`] — `BitWriter`/`BitReader` MSB-first para `b` de 1 a 32 (E1);
//! - [`table`] — layout de namespace, construção M2 e serialização (E2);
//! - [`hgr`] — o cabeçalho de 36 bytes do fluxo (E2);
//! - [`codec`] — o [`Model`](codec::Model) paramétrico: tokenizar, codificar
//!   e decodificar (E2).
//!
//! Os corpora (E3), a sweep com baselines (E4) e as ops no domínio de
//! símbolos (E5) vêm nas etapas seguintes.

#![forbid(unsafe_code)]

pub mod codec;
pub mod hgr;
pub mod model;
pub mod packing;
pub mod table;

/// Versão do crate, propagada para o cabeçalho dos runs (`docs/02 §6.1`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
