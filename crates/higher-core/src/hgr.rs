//! Cabeçalho do fluxo `.hgr` (`docs/01 §8`).
//!
//! ```text
//! offset  campo            tipo      nota
//! 0       magic            4 bytes   "HGR1"
//! 4       version          u8        formato = 1
//! 5       b                u8        1..32
//! 6       variant          u8        0=M0 1=M1 2=M2 4=M4
//! 7       flags            u8        bit0 = bloco adaptativo, bit1 = sync markers
//! 8       table_id         [u8; 8]   primeiros 8 bytes do sha256 da tabela
//! 16      n_symbols        u64
//! 24      n_input_bytes    u64
//! 32      crc32_payload    u32
//! ```
//!
//! São 36 bytes fixos, big-endian (`CLAUDE.md`, "Convenções"), e eles
//! **entram** na conta de bytes armazenados e transmitidos (`docs/02 §3`).
//!
//! O cabeçalho existe por causa do defeito D7 de `docs/05 §3`: o protótipo
//! anterior tinha 4 bytes só com `n_tokens`, sem id de tabela nem checksum,
//! e por isso aceitava em silêncio um fluxo decodificado com a tabela
//! errada. Aqui `table_id` e `crc32_payload` tornam esses dois casos
//! detectáveis.

use std::fmt;

use crate::model::{Variant, Width};

/// Magic do fluxo.
pub const HGR_MAGIC: &[u8; 4] = b"HGR1";

/// Versão de formato que este crate escreve e lê.
pub const HGR_VERSION: u8 = 1;

/// Tamanho fixo do cabeçalho, em bytes.
pub const HEADER_BYTES: usize = 36;

/// `flags.bit0` — há bloco adaptativo antes do payload (M4, Etapa E7).
pub const FLAG_ADAPTIVE: u8 = 0b0000_0001;

/// `flags.bit1` — há sync markers no payload (Etapa E8).
pub const FLAG_SYNC_MARKERS: u8 = 0b0000_0010;

/// Cabeçalho decodificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Largura do símbolo.
    pub width: Width,
    /// Variante da tabela.
    pub variant: Variant,
    /// Bits de flag, ainda sem semântica até E7/E8.
    pub flags: u8,
    /// Primeiros 8 bytes do sha256 da tabela usada.
    pub table_id: [u8; 8],
    /// Símbolos no payload. No LOWER, um ESCAPE conta como **um** símbolo,
    /// e os 8 bits brutos que o seguem são carga dele.
    pub n_symbols: u64,
    /// Bytes da entrada original, antes de codificar.
    pub n_input_bytes: u64,
    /// CRC-32 (IEEE, o mesmo do gzip/zlib) do payload empacotado.
    pub crc32_payload: u32,
}

/// Erro ao ler um cabeçalho `.hgr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    /// O fluxo é menor que os 36 bytes do cabeçalho.
    TooShort {
        /// Bytes recebidos.
        got: usize,
    },
    /// Os 4 primeiros bytes não são `HGR1`.
    BadMagic {
        /// O que veio no lugar.
        got: [u8; 4],
    },
    /// Versão de formato desconhecida.
    BadVersion {
        /// A versão lida.
        got: u8,
    },
    /// Largura fora de `1..=32`.
    BadWidth {
        /// A largura lida.
        got: u8,
    },
    /// Byte de variante fora de `{0, 1, 2, 4}`.
    BadVariant {
        /// O byte lido.
        got: u8,
    },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { got } => {
                write!(
                    f,
                    "fluxo com {got} bytes, menor que o cabeçalho de {HEADER_BYTES}"
                )
            }
            Self::BadMagic { got } => write!(f, "magic inválido: {got:?}, esperado HGR1"),
            Self::BadVersion { got } => write!(f, "versão de fluxo desconhecida: {got}"),
            Self::BadWidth { got } => write!(f, "largura inválida no cabeçalho: {got}"),
            Self::BadVariant { got } => write!(f, "variante inválida no cabeçalho: {got}"),
        }
    }
}

impl std::error::Error for HeaderError {}

impl Header {
    /// Escreve o cabeçalho em 36 bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HEADER_BYTES] {
        let mut out = [0u8; HEADER_BYTES];
        out[0..4].copy_from_slice(HGR_MAGIC);
        out[4] = HGR_VERSION;
        out[5] = self.width.bits();
        out[6] = self.variant.to_wire();
        out[7] = self.flags;
        out[8..16].copy_from_slice(&self.table_id);
        out[16..24].copy_from_slice(&self.n_symbols.to_be_bytes());
        out[24..32].copy_from_slice(&self.n_input_bytes.to_be_bytes());
        out[32..36].copy_from_slice(&self.crc32_payload.to_be_bytes());
        out
    }

    /// Lê o cabeçalho dos primeiros 36 bytes de `src`.
    ///
    /// # Errors
    ///
    /// [`HeaderError`] para fluxo curto, magic, versão, largura ou variante
    /// inválidos. Nunca entra em pânico.
    pub fn from_bytes(src: &[u8]) -> Result<Self, HeaderError> {
        if src.len() < HEADER_BYTES {
            return Err(HeaderError::TooShort { got: src.len() });
        }
        if &src[0..4] != HGR_MAGIC {
            let mut got = [0u8; 4];
            got.copy_from_slice(&src[0..4]);
            return Err(HeaderError::BadMagic { got });
        }
        if src[4] != HGR_VERSION {
            return Err(HeaderError::BadVersion { got: src[4] });
        }
        let width = Width::new(src[5]).map_err(|_| HeaderError::BadWidth { got: src[5] })?;
        let variant = Variant::from_wire(src[6]).ok_or(HeaderError::BadVariant { got: src[6] })?;

        let mut table_id = [0u8; 8];
        table_id.copy_from_slice(&src[8..16]);
        let mut oito = [0u8; 8];
        oito.copy_from_slice(&src[16..24]);
        let n_symbols = u64::from_be_bytes(oito);
        oito.copy_from_slice(&src[24..32]);
        let n_input_bytes = u64::from_be_bytes(oito);
        let crc32_payload = u32::from_be_bytes([src[32], src[33], src[34], src[35]]);

        Ok(Self {
            width,
            variant,
            flags: src[7],
            table_id,
            n_symbols,
            n_input_bytes,
            crc32_payload,
        })
    }

    /// `true` se o fluxo declara bloco adaptativo (M4).
    #[must_use]
    pub const fn has_adaptive_block(&self) -> bool {
        self.flags & FLAG_ADAPTIVE != 0
    }

    /// `true` se o fluxo declara sync markers.
    #[must_use]
    pub const fn has_sync_markers(&self) -> bool {
        self.flags & FLAG_SYNC_MARKERS != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cabecalho() -> Header {
        Header {
            width: Width::new(14).unwrap(),
            variant: Variant::M2,
            flags: 0,
            table_id: [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            n_symbols: 0x0102_0304_0506_0708,
            n_input_bytes: 0x1112_1314_1516_1718,
            crc32_payload: 0xDEAD_BEEF,
        }
    }

    #[test]
    fn os_campos_caem_nos_offsets_de_docs_01_secao_8() {
        let b = cabecalho().to_bytes();
        assert_eq!(b.len(), 36, "cabeçalho tem de ter 36 bytes fixos");
        assert_eq!(&b[0..4], b"HGR1");
        assert_eq!(b[4], 1, "version");
        assert_eq!(b[5], 14, "b");
        assert_eq!(b[6], 2, "variant M2");
        assert_eq!(b[7], 0, "flags");
        assert_eq!(&b[8..16], &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
        // Big-endian em todo cabeçalho (CLAUDE.md, "Convenções").
        assert_eq!(&b[16..24], &[1, 2, 3, 4, 5, 6, 7, 8], "n_symbols BE");
        assert_eq!(
            &b[24..32],
            &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18],
            "n_input_bytes BE"
        );
        assert_eq!(&b[32..36], &[0xDE, 0xAD, 0xBE, 0xEF], "crc32 BE");
    }

    #[test]
    fn roundtrip_do_cabecalho() {
        let original = cabecalho();
        assert_eq!(Header::from_bytes(&original.to_bytes()), Ok(original));
    }

    #[test]
    fn campos_extras_depois_do_cabecalho_sao_ignorados_na_leitura() {
        let mut bytes = cabecalho().to_bytes().to_vec();
        bytes.extend_from_slice(b"payload arbitrario");
        assert_eq!(Header::from_bytes(&bytes), Ok(cabecalho()));
    }

    #[test]
    fn erros_tipados_para_cabecalho_invalido() {
        let bom = cabecalho().to_bytes();

        assert_eq!(
            Header::from_bytes(&bom[..35]),
            Err(HeaderError::TooShort { got: 35 })
        );

        let mut m = bom;
        m[0] = b'X';
        assert!(matches!(
            Header::from_bytes(&m),
            Err(HeaderError::BadMagic { .. })
        ));

        let mut v = bom;
        v[4] = 2;
        assert_eq!(
            Header::from_bytes(&v),
            Err(HeaderError::BadVersion { got: 2 })
        );

        for largura_ruim in [0u8, 33, 255] {
            let mut w = bom;
            w[5] = largura_ruim;
            assert_eq!(
                Header::from_bytes(&w),
                Err(HeaderError::BadWidth { got: largura_ruim })
            );
        }

        for variante_ruim in [3u8, 5, 255] {
            let mut v = bom;
            v[6] = variante_ruim;
            assert_eq!(
                Header::from_bytes(&v),
                Err(HeaderError::BadVariant { got: variante_ruim })
            );
        }
    }

    #[test]
    fn as_flags_sao_lidas_bit_a_bit() {
        let mut h = cabecalho();
        h.flags = FLAG_ADAPTIVE;
        assert!(h.has_adaptive_block() && !h.has_sync_markers());
        h.flags = FLAG_SYNC_MARKERS;
        assert!(!h.has_adaptive_block() && h.has_sync_markers());
        h.flags = FLAG_ADAPTIVE | FLAG_SYNC_MARKERS;
        assert!(h.has_adaptive_block() && h.has_sync_markers());
    }
}
