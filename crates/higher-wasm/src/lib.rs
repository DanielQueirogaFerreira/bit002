//! Bindings WASM do `higher-core` para a UI (`web/`) e o Worker.
//!
//! O browser roda exatamente o mesmo núcleo do nativo. O que ele **não**
//! faz é medir tempo oficial: todo resultado produzido aqui sai rotulado
//! `env = "wasm"` e não se mistura com o nativo (`docs/02 §6.7`).
//!
//! Na Etapa E0 só há a superfície mínima. `encode`/`decode`/`sweep` entram
//! na Etapa E9.

#![forbid(unsafe_code)]

use higher_core::model::{all_widths, Variant, Width};
use wasm_bindgen::prelude::wasm_bindgen;

/// Ambiente reportado por todo run originado deste crate.
pub const ENV: &str = "wasm";

/// Versão do `higher-core` embutido neste build.
#[wasm_bindgen]
#[must_use]
pub fn core_version() -> String {
    higher_core::VERSION.to_string()
}

/// Ambiente a gravar no campo `env` do run (`docs/02 §6.1`).
#[wasm_bindgen]
#[must_use]
pub fn env_label() -> String {
    ENV.to_string()
}

/// Nome canônico do modelo de largura `bits`, ou string vazia se `bits` sair de `1..=32`.
#[wasm_bindgen]
#[must_use]
pub fn model_name(bits: u8) -> String {
    Width::new(bits).map_or_else(|_| String::new(), Width::model_name)
}

/// Nomes de run das 32 larguras da sweep, separados por vírgula.
///
/// A UI usa isso para montar a lista de modelos sem duplicar a convenção
/// de nomes em JavaScript.
#[wasm_bindgen]
#[must_use]
pub fn sweep_run_names() -> String {
    all_widths()
        .into_iter()
        .map(|w| w.run_name(Variant::M2))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_superficie_wasm_reexporta_a_convencao_de_nomes() {
        assert_eq!(model_name(14), "higher-14");
        assert_eq!(model_name(8), "base-8");
        assert_eq!(model_name(0), "");
        assert_eq!(model_name(33), "");
        assert_eq!(env_label(), "wasm");
        assert_eq!(core_version(), higher_core::VERSION);

        let lista = sweep_run_names();
        let nomes: Vec<&str> = lista.split(',').collect();
        assert_eq!(nomes.len(), 32);
        assert_eq!(nomes[0], "lower-1:M2");
        assert_eq!(nomes[7], "base-8:M2");
        assert_eq!(nomes[31], "higher-32:M2");
    }
}
