//! Modelo paramétrico: tokenização, encode e decode do fluxo `.hgr`.
//!
//! Um só [`Model`] atende as 32 larguras (`docs/01 §1`). O que muda é a
//! faixa, e ela decide três coisas:
//!
//! | Faixa | Tokenização | Símbolo não coberto |
//! |---|---|---|
//! | LOWER `b < 8` | maior casamento na tabela | **ESCAPE** (código 0) + 8 bits brutos |
//! | BASE `b = 8` | byte a byte | não existe: os 256 cabem |
//! | HIGHER `b > 8` | maior casamento, mínimo 2 bytes | o byte vai no seu código legado |
//!
//! O parser é o P-greedy de `docs/01 §5`: maior casamento via trie. O
//! P-optimal (programação dinâmica) é a E6, e existe para medir **quanto o
//! guloso deixa na mesa** — não para substituí-lo.

use crc32fast::Hasher as Crc32;

use crate::hgr::{Header, HeaderError, HEADER_BYTES};
use crate::model::{Band, Variant, Width};
use crate::packing::{BitReader, BitWriter, PackError};
use crate::table::{Table, ESCAPE_CODE};

/// Erro ao codificar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// A entrada é maior do que o cabeçalho consegue declarar.
    InputTooLarge {
        /// Bytes da entrada.
        got: usize,
    },
    /// Um código não coube na largura — indica tabela inconsistente.
    Packing(PackError),
}

impl From<PackError> for EncodeError {
    fn from(e: PackError) -> Self {
        Self::Packing(e)
    }
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InputTooLarge { got } => {
                write!(f, "entrada de {got} bytes não cabe no cabeçalho")
            }
            Self::Packing(e) => write!(f, "erro de packing ao codificar: {e}"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Erro ao decodificar.
///
/// Todos os casos são detectados e devolvidos: o decoder **nunca** entra em
/// pânico, nem devolve bytes plausíveis a partir de um fluxo inconsistente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Cabeçalho inválido.
    Header(HeaderError),
    /// O fluxo foi escrito com outra tabela.
    ///
    /// É o defeito D7 de `docs/05 §3`: sem esta checagem o decoder aceitaria
    /// a tabela errada em silêncio e devolveria bytes verossímeis e errados.
    TableMismatch {
        /// `table_id` declarado no cabeçalho.
        expected: [u8; 8],
        /// `table_id` da tabela oferecida ao decoder.
        got: [u8; 8],
    },
    /// A largura do cabeçalho não é a da tabela oferecida.
    WidthMismatch {
        /// Largura no cabeçalho.
        expected: u8,
        /// Largura da tabela.
        got: u8,
    },
    /// A variante do cabeçalho não é a da tabela oferecida.
    VariantMismatch {
        /// Variante no cabeçalho.
        expected: u8,
        /// Variante da tabela.
        got: u8,
    },
    /// O CRC do payload não bate.
    CrcMismatch {
        /// CRC declarado no cabeçalho.
        expected: u32,
        /// CRC calculado sobre o payload recebido.
        got: u32,
    },
    /// Faltam bits para os `n_symbols` declarados.
    Truncated {
        /// Símbolos declarados no cabeçalho.
        declared: u64,
        /// Símbolos que o payload comporta.
        available: u64,
    },
    /// Um código do fluxo não existe na tabela.
    UnknownCode {
        /// O código lido.
        code: u32,
        /// Posição do símbolo no fluxo.
        index: u64,
    },
    /// O decode produziu um número de bytes diferente do declarado.
    LengthMismatch {
        /// Bytes declarados no cabeçalho.
        declared: u64,
        /// Bytes produzidos.
        got: u64,
    },
    /// Os bits de completamento do último byte não são zero (`docs/01 §7`).
    PaddingNotZero,
    /// O fluxo declara um recurso que esta etapa ainda não implementa.
    UnsupportedFlags {
        /// Os bits de flag do cabeçalho.
        flags: u8,
    },
}

impl From<HeaderError> for DecodeError {
    fn from(e: HeaderError) -> Self {
        Self::Header(e)
    }
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Header(e) => write!(f, "cabeçalho inválido: {e}"),
            Self::TableMismatch { expected, got } => write!(
                f,
                "fluxo escrito com outra tabela: cabeçalho pede {expected:02x?}, tabela é {got:02x?}"
            ),
            Self::WidthMismatch { expected, got } => {
                write!(f, "cabeçalho declara b={expected}, tabela é b={got}")
            }
            Self::VariantMismatch { expected, got } => {
                write!(f, "cabeçalho declara variante {expected}, tabela é {got}")
            }
            Self::CrcMismatch { expected, got } => {
                write!(f, "crc32 do payload não bate: esperado {expected:#010x}, calculado {got:#010x}")
            }
            Self::Truncated {
                declared,
                available,
            } => write!(f, "fluxo truncado: {declared} símbolos declarados, {available} disponíveis"),
            Self::UnknownCode { code, index } => {
                write!(f, "código {code} do símbolo {index} não existe na tabela")
            }
            Self::LengthMismatch { declared, got } => {
                write!(f, "cabeçalho declara {declared} bytes de entrada, decode produziu {got}")
            }
            Self::PaddingNotZero => {
                write!(f, "bits de completamento do último byte não são zero")
            }
            Self::UnsupportedFlags { flags } => {
                write!(f, "flags {flags:#010b} pedem recurso não implementado nesta etapa")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Estatísticas de um encode, para alimentar `docs/02 §3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncodeStats {
    /// Bytes da entrada.
    pub input_bytes: u64,
    /// Símbolos emitidos (um ESCAPE conta como um).
    pub n_symbols: u64,
    /// Quantos desses símbolos foram ESCAPE (só LOWER).
    pub n_escapes: u64,
    /// Bytes do fluxo inteiro, cabeçalho incluído.
    pub encoded_bytes: u64,
    /// Bytes do cabeçalho — sempre [`HEADER_BYTES`], reportado à parte
    /// porque `docs/02 §4` pede `overhead_header` separado.
    pub header_bytes: u64,
}

// Métricas de razão: os contadores viram `f64`. `docs/02 §1` limita os
// corpora a 10 MB (2^24), muito abaixo dos 2^53 em que um u64 começaria a
// perder precisão num f64.
#[allow(clippy::cast_precision_loss)]
impl EncodeStats {
    /// `bits_per_byte` de `docs/02 §3`, a métrica principal de tamanho.
    ///
    /// Inclui o cabeçalho. **Não** inclui a tabela: ela é pré-acordada e
    /// amortizada, e entra na conta por `table_bytes`/`breakeven_files`,
    /// como `docs/02 §3` separa.
    #[must_use]
    pub fn bits_per_byte(&self) -> f64 {
        if self.input_bytes == 0 {
            return f64::NAN;
        }
        self.encoded_bytes as f64 * 8.0 / self.input_bytes as f64
    }

    /// `horiz_reduction` de `docs/02 §3`: `1 − n_symbols / input_bytes`.
    ///
    /// É a redução horizontal que `docs/00 §3.1` exige ficar acima de
    /// `1 − 8/b` para haver ganho no HIGHER.
    #[must_use]
    pub fn horiz_reduction(&self) -> f64 {
        if self.input_bytes == 0 {
            return f64::NAN;
        }
        1.0 - self.n_symbols as f64 / self.input_bytes as f64
    }

    /// `escape_rate` de `docs/02 §3`: escapes por símbolo.
    #[must_use]
    pub fn escape_rate(&self) -> f64 {
        if self.n_symbols == 0 {
            return 0.0;
        }
        self.n_escapes as f64 / self.n_symbols as f64
    }
}

/// Modelo paramétrico: uma largura, uma variante e uma tabela (`docs/01 §1`).
#[derive(Debug, Clone)]
pub struct Model {
    table: Table,
}

impl Model {
    /// Modelo sobre uma tabela.
    #[must_use]
    pub const fn new(table: Table) -> Self {
        Self { table }
    }

    /// A régua: `base-8`, identidade byte a byte.
    #[must_use]
    pub fn base8() -> Self {
        Self::new(Table::legacy_only(Width::BASE, Variant::M0))
    }

    /// Largura do modelo.
    #[must_use]
    pub const fn width(&self) -> Width {
        self.table.width()
    }

    /// Variante do modelo.
    #[must_use]
    pub const fn variant(&self) -> Variant {
        self.table.variant()
    }

    /// A tabela do modelo.
    #[must_use]
    pub const fn table(&self) -> &Table {
        &self.table
    }

    /// Nome de run, no formato de `CLAUDE.md`: `higher-14:M2`, `base-8`.
    #[must_use]
    pub fn run_name(&self) -> String {
        self.width().run_name(self.variant())
    }

    /// Tokeniza `input` em símbolos, sem empacotar (P-greedy, `docs/01 §5`).
    ///
    /// Devolve os códigos e, junto de cada ESCAPE, o byte bruto que o segue.
    /// Serve para as ops no domínio de símbolos da E5, que operam sobre os
    /// códigos sem voltar a bytes.
    #[must_use]
    pub fn tokenize(&self, input: &[u8]) -> Vec<Symbol> {
        let mut out = Vec::new();
        let band = self.width().band();
        let mut i = 0usize;
        while i < input.len() {
            match band {
                Band::Base => {
                    out.push(Symbol::Code(u32::from(input[i])));
                    i += 1;
                }
                Band::Higher => {
                    // Casamento mínimo de 2 bytes: um bloco de 1 byte custaria
                    // `b > 8` bits para o que o código legado já resolve, e a
                    // função de ganho de `docs/01 §4` nem o geraria.
                    match self.table.longest_match(&input[i..]) {
                        Some((code, len)) if len >= 2 => {
                            out.push(Symbol::Code(code));
                            i += len;
                        }
                        _ => {
                            out.push(Symbol::Code(u32::from(input[i])));
                            i += 1;
                        }
                    }
                }
                Band::Lower => {
                    if let Some((code, len)) = self.table.longest_match(&input[i..]) {
                        out.push(Symbol::Code(code));
                        i += len;
                    } else {
                        out.push(Symbol::Escape(input[i]));
                        i += 1;
                    }
                }
            }
        }
        out
    }

    /// Codifica `input` num fluxo `.hgr` completo.
    ///
    /// # Errors
    ///
    /// [`EncodeError`] se a entrada não couber no cabeçalho ou se a tabela
    /// produzir código fora da largura.
    pub fn encode(&self, input: &[u8]) -> Result<(Vec<u8>, EncodeStats), EncodeError> {
        let width = self.width();
        let simbolos = self.tokenize(input);

        let n_symbols = simbolos.len() as u64;
        let n_escapes = simbolos
            .iter()
            .filter(|s| matches!(s, Symbol::Escape(_)))
            .count() as u64;

        let mut bw = BitWriter::with_capacity_for(simbolos.len(), width);
        if n_escapes == 0 {
            // Fluxo de largura uniforme: passa pelos caminhos especializados
            // de packing da E1.
            let codigos: Vec<u32> = simbolos
                .iter()
                .map(|s| match s {
                    Symbol::Code(c) => *c,
                    Symbol::Escape(_) => ESCAPE_CODE,
                })
                .collect();
            bw.push_all(&codigos, width)?;
        } else {
            for s in &simbolos {
                match s {
                    Symbol::Code(c) => bw.push(*c, width)?,
                    Symbol::Escape(raw) => {
                        bw.push(ESCAPE_CODE, width)?;
                        bw.push(u32::from(*raw), Width::BASE)?;
                    }
                }
            }
        }
        let payload = bw.finish();

        let mut crc = Crc32::new();
        crc.update(&payload);
        let header = Header {
            width,
            variant: self.variant(),
            flags: 0,
            table_id: self.table.id8(),
            n_symbols,
            n_input_bytes: input.len() as u64,
            crc32_payload: crc.finalize(),
        };

        let mut out = Vec::with_capacity(HEADER_BYTES + payload.len());
        out.extend_from_slice(&header.to_bytes());
        out.extend_from_slice(&payload);

        let stats = EncodeStats {
            input_bytes: input.len() as u64,
            n_symbols,
            n_escapes,
            encoded_bytes: out.len() as u64,
            header_bytes: HEADER_BYTES as u64,
        };
        Ok((out, stats))
    }

    /// Decodifica um fluxo `.hgr` com **esta** tabela.
    ///
    /// Verifica, nesta ordem: cabeçalho, flags, largura, variante,
    /// `table_id`, crc do payload, bits suficientes, cada código, o
    /// completamento em zeros e o número de bytes produzidos. Qualquer
    /// divergência vira [`DecodeError`]; nada entra em pânico.
    ///
    /// # Errors
    ///
    /// [`DecodeError`] em qualquer das verificações acima.
    pub fn decode(&self, stream: &[u8]) -> Result<Vec<u8>, DecodeError> {
        let header = Header::from_bytes(stream)?;

        if header.flags != 0 {
            // Bloco adaptativo (E7) e sync markers (E8) mudam o layout do
            // payload. Aceitar a flag sem implementar o layout devolveria
            // lixo verossímil.
            return Err(DecodeError::UnsupportedFlags {
                flags: header.flags,
            });
        }
        let width = self.width();
        if header.width != width {
            return Err(DecodeError::WidthMismatch {
                expected: header.width.bits(),
                got: width.bits(),
            });
        }
        if header.variant != self.variant() {
            return Err(DecodeError::VariantMismatch {
                expected: header.variant.to_wire(),
                got: self.variant().to_wire(),
            });
        }
        let id8 = self.table.id8();
        if header.table_id != id8 {
            return Err(DecodeError::TableMismatch {
                expected: header.table_id,
                got: id8,
            });
        }

        let payload = &stream[HEADER_BYTES..];
        let mut crc = Crc32::new();
        crc.update(payload);
        let calculado = crc.finalize();
        if calculado != header.crc32_payload {
            return Err(DecodeError::CrcMismatch {
                expected: header.crc32_payload,
                got: calculado,
            });
        }

        let bits_total = payload.len() as u64 * 8;
        let bits_por_simbolo = u64::from(width.bits());
        if header.n_symbols.saturating_mul(bits_por_simbolo) > bits_total {
            return Err(DecodeError::Truncated {
                declared: header.n_symbols,
                available: bits_total / bits_por_simbolo,
            });
        }

        // A capacidade sai de `n_symbols`, não de `n_input_bytes`: o
        // primeiro já foi validado contra o tamanho do payload, o segundo é
        // um número que o cabeçalho declara. Um fluxo com crc válido e
        // `n_input_bytes = 2^63` pediria uma alocação absurda antes de
        // qualquer verificação — e o `LengthMismatch` lá embaixo é quem
        // pega a mentira, sem precisar reservar memória para ela.
        let mut out = Vec::with_capacity(usize::try_from(header.n_symbols).unwrap_or(0));
        let mut br = BitReader::new(payload);
        let band = width.band();

        for index in 0..header.n_symbols {
            let code = br.read(width).ok_or(DecodeError::Truncated {
                declared: header.n_symbols,
                available: index,
            })?;
            match band {
                Band::Base => out.push(
                    u8::try_from(code).map_err(|_| DecodeError::UnknownCode { code, index })?,
                ),
                Band::Higher => {
                    if code < 256 {
                        #[allow(clippy::cast_possible_truncation)]
                        out.push(code as u8);
                    } else {
                        let bloco = self
                            .table
                            .block_of(code)
                            .ok_or(DecodeError::UnknownCode { code, index })?;
                        out.extend_from_slice(bloco);
                    }
                }
                Band::Lower => {
                    if code == ESCAPE_CODE {
                        let raw = br.read(Width::BASE).ok_or(DecodeError::Truncated {
                            declared: header.n_symbols,
                            available: index,
                        })?;
                        #[allow(clippy::cast_possible_truncation)]
                        out.push(raw as u8);
                    } else {
                        let bloco = self
                            .table
                            .block_of(code)
                            .ok_or(DecodeError::UnknownCode { code, index })?;
                        out.extend_from_slice(bloco);
                    }
                }
            }
        }

        // O que sobra tem de ser só o completamento em zeros de `docs/01 §7`.
        // Bit sujo aqui significa payload maior do que `n_symbols` explica —
        // cabeçalho e corpo inconsistentes, ainda que o crc feche.
        if br.bits_remaining() >= 8 || !br.remaining_is_zero() {
            return Err(DecodeError::PaddingNotZero);
        }

        if out.len() as u64 != header.n_input_bytes {
            return Err(DecodeError::LengthMismatch {
                declared: header.n_input_bytes,
                got: out.len() as u64,
            });
        }
        Ok(out)
    }
}

/// Um símbolo tokenizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    /// Um código da tabela (ou legado, no BASE e no HIGHER).
    Code(u32),
    /// ESCAPE do LOWER, carregando o byte bruto que o segue (`docs/01 §6`).
    Escape(u8),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hgr::Header;
    use crate::table::Table;

    fn w(bits: u8) -> Width {
        Width::new(bits).expect("largura de teste válida")
    }

    const TREINO: &[u8] = b"a rosa e a rosa e a rosa branca, a rosa vermelha, a rosa azul. \
                            a rosa e a rosa e a rosa branca, a rosa amarela, a rosa rosa. ";

    #[test]
    fn base_8_e_identidade_byte_a_byte() {
        // docs/01 §1: BASE é identidade, sem escape e sem packing.
        let modelo = Model::base8();
        let entrada: Vec<u8> = (0..=255u8).collect();
        let (fluxo, stats) = modelo.encode(&entrada).unwrap();

        assert_eq!(
            &fluxo[HEADER_BYTES..],
            &entrada[..],
            "payload é a entrada crua"
        );
        assert_eq!(stats.n_symbols, 256);
        assert_eq!(stats.n_escapes, 0);
        assert!(
            stats.horiz_reduction().abs() < 1e-12,
            "base-8 não reduz nada"
        );
        assert_eq!(modelo.decode(&fluxo).unwrap(), entrada);
    }

    #[test]
    fn base_8_m2_e_identico_a_base_8() {
        // Resolve a ambiguidade §4.2 de results/ETAPA-00.md: `base-8:M2` é
        // um nome legítimo, mas a largura não tem slot dinâmico nenhum
        // (256 códigos, 256 bytes legados), então a tabela sai vazia e o
        // fluxo é byte a byte igual ao de `base-8`. Treinar num corpus não
        // muda isso.
        let m0 = Model::base8();
        let m2 = Model::new(Table::train_m2(w(8), TREINO));
        assert_eq!(m2.table().slots_used(), 0);
        assert_eq!(m2.run_name(), "base-8:M2");
        assert_eq!(m0.run_name(), "base-8");

        let entrada = b"qualquer coisa aqui".to_vec();
        let (f0, _) = m0.encode(&entrada).unwrap();
        let (f2, _) = m2.encode(&entrada).unwrap();
        assert_eq!(
            &f0[HEADER_BYTES..],
            &f2[HEADER_BYTES..],
            "o payload tem de ser o mesmo; só variante e table_id diferem"
        );
    }

    #[test]
    fn o_cabecalho_entra_na_conta_de_bytes() {
        // docs/02 §3: `encoded_bytes` = payload + cabeçalho. Em `base-8`
        // isso põe b/B acima de 8, e é o que a ambiguidade §4.6 da E0
        // manda decidir antes de qualquer número de H1.
        let modelo = Model::base8();
        let entrada = vec![b'x'; 10_000];
        let (fluxo, stats) = modelo.encode(&entrada).unwrap();

        assert_eq!(stats.encoded_bytes, 10_000 + HEADER_BYTES as u64);
        assert_eq!(fluxo.len(), 10_036);
        assert!(
            stats.bits_per_byte() > 8.0,
            "a régua paga o cabeçalho: {}",
            stats.bits_per_byte()
        );
        assert!((stats.bits_per_byte() - 8.0288).abs() < 1e-3);
    }

    #[test]
    fn higher_usa_o_codigo_legado_quando_nao_ha_bloco() {
        let modelo = Model::new(Table::legacy_only(w(14), Variant::M1));
        let simbolos = modelo.tokenize(b"AB");
        assert_eq!(
            simbolos,
            vec![Symbol::Code(u32::from(b'A')), Symbol::Code(u32::from(b'B'))]
        );
    }

    #[test]
    fn higher_prefere_o_maior_bloco() {
        let modelo = Model::new(Table::train_m2(w(14), TREINO));
        let simbolos = modelo.tokenize(b"a rosa e a rosa");
        // O primeiro símbolo tem de cobrir mais de um byte, senão a trie não
        // está sendo consultada.
        assert!(
            simbolos.len() < 15,
            "P-greedy devia reduzir 15 bytes a menos símbolos, deu {}",
            simbolos.len()
        );
        assert!(matches!(simbolos[0], Symbol::Code(c) if c >= 256));
    }

    #[test]
    fn lower_escapa_o_que_a_tabela_nao_cobre() {
        let modelo = Model::new(Table::legacy_only(w(4), Variant::M1));
        // Tabela vazia: tudo escapa.
        let simbolos = modelo.tokenize(b"oi");
        assert_eq!(simbolos, vec![Symbol::Escape(b'o'), Symbol::Escape(b'i')]);

        let (fluxo, stats) = modelo.encode(b"oi").unwrap();
        assert_eq!(stats.n_symbols, 2);
        assert_eq!(stats.n_escapes, 2);
        assert!((stats.escape_rate() - 1.0).abs() < 1e-12);
        // Cada símbolo custa 4 bits de código + 8 de byte bruto = 24 bits = 3 bytes.
        assert_eq!(fluxo.len(), HEADER_BYTES + 3);
        assert_eq!(modelo.decode(&fluxo).unwrap(), b"oi");
    }

    #[test]
    fn a_redução_horizontal_e_o_limiar_da_tese() {
        // docs/00 §3.1: no HIGHER só há ganho se a redução passar de 1 − 8/b.
        // Aqui isso é só a aritmética de EncodeStats; se ela vale de fato em
        // corpus real é a E4 que mede.
        let modelo = Model::new(Table::train_m2(w(14), TREINO));
        let (_, stats) = modelo.encode(TREINO).unwrap();
        let limiar = w(14).threshold();
        assert!((limiar - 0.428_571).abs() < 1e-5);
        assert!(
            stats.horiz_reduction() > limiar,
            "no próprio corpus de treino a redução ({}) devia passar do limiar ({limiar})",
            stats.horiz_reduction()
        );
    }

    #[test]
    fn o_cabecalho_declara_o_que_o_modelo_usou() {
        let modelo = Model::new(Table::train_m2(w(12), TREINO));
        let (fluxo, stats) = modelo.encode(TREINO).unwrap();
        let h = Header::from_bytes(&fluxo).unwrap();

        assert_eq!(h.width, w(12));
        assert_eq!(h.variant, Variant::M2);
        assert_eq!(h.flags, 0);
        assert_eq!(h.table_id, modelo.table().id8());
        assert_eq!(h.n_symbols, stats.n_symbols);
        assert_eq!(h.n_input_bytes, TREINO.len() as u64);
    }

    #[test]
    fn decoder_rejeita_largura_e_variante_trocadas() {
        let modelo14 = Model::new(Table::train_m2(w(14), TREINO));
        let (fluxo, _) = modelo14.encode(TREINO).unwrap();

        let modelo16 = Model::new(Table::train_m2(w(16), TREINO));
        assert!(matches!(
            modelo16.decode(&fluxo),
            Err(DecodeError::WidthMismatch {
                expected: 14,
                got: 16
            })
        ));

        let legado14 = Model::new(Table::legacy_only(w(14), Variant::M1));
        assert!(matches!(
            legado14.decode(&fluxo),
            Err(DecodeError::VariantMismatch {
                expected: 2,
                got: 1
            })
        ));
    }

    #[test]
    fn decoder_rejeita_n_input_bytes_mentiroso() {
        // Cabeçalho coerente e crc válido, mas `n_input_bytes` adulterado:
        // o decode roda até o fim e o total tem de bater.
        let modelo = Model::base8();
        let entrada = b"doze bytes!!".to_vec();
        let (mut fluxo, _) = modelo.encode(&entrada).unwrap();
        fluxo[31] = 99; // byte baixo de n_input_bytes

        assert!(matches!(
            modelo.decode(&fluxo),
            Err(DecodeError::LengthMismatch {
                declared: 99,
                got: 12
            })
        ));
    }

    #[test]
    fn decoder_rejeita_payload_maior_do_que_n_symbols_explica() {
        let modelo = Model::base8();
        let (fluxo, _) = modelo.encode(b"abc").unwrap();
        // Acrescenta um byte ao payload e recalcula o crc, para o fluxo
        // passar em tudo menos na checagem de completamento.
        let mut inflado = fluxo.clone();
        inflado.push(0xFF);
        let mut crc = Crc32::new();
        crc.update(&inflado[HEADER_BYTES..]);
        inflado[32..36].copy_from_slice(&crc.finalize().to_be_bytes());

        assert_eq!(modelo.decode(&inflado), Err(DecodeError::PaddingNotZero));
    }

    #[test]
    fn entrada_vazia_produz_so_o_cabecalho() {
        for bits in [1u8, 4, 8, 14, 32] {
            let modelo = Model::new(Table::train_m2(w(bits), TREINO));
            let (fluxo, stats) = modelo.encode(b"").unwrap();
            assert_eq!(fluxo.len(), HEADER_BYTES, "b = {bits}");
            assert_eq!(stats.n_symbols, 0, "b = {bits}");
            assert!(
                stats.bits_per_byte().is_nan(),
                "b = {bits}: 0 bytes não tem b/B"
            );
            assert_eq!(
                modelo.decode(&fluxo).unwrap(),
                Vec::<u8>::new(),
                "b = {bits}"
            );
        }
    }
}
