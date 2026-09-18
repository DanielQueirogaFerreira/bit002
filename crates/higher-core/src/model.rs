//! Vocabulário dos modelos: largura, faixa, variante e nome de run.
//!
//! Tudo aqui vem de `CLAUDE.md` ("Convenções") e de `docs/00-TESE.md §3`.
//! É a fundação que as etapas seguintes reusam, não o codec.

use core::fmt;

/// Menor largura de símbolo admitida pela sweep.
pub const MIN_BITS: u8 = 1;
/// Maior largura de símbolo admitida pela sweep.
pub const MAX_BITS: u8 = 32;
/// Largura do símbolo da régua (`base-8`).
pub const BASE_BITS: u8 = 8;

/// Faixa a que uma largura pertence.
///
/// A faixa é só uma leitura de `b`: ela não carrega política, mas decide
/// qual regra de tabela se aplica (`docs/01 §1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Band {
    /// `b < 8`: menos bits por símbolo, com escape (`docs/01 §6`).
    Lower,
    /// `b = 8`: a régua, identidade byte a byte.
    Base,
    /// `b > 8`: a tese, com os 256 códigos legados preservados.
    Higher,
}

/// Variantes de tabela, generalizadas de M0–M4 para qualquer `b` (`docs/01 §2`).
///
/// O valor numérico é o que vai no campo `variant` do cabeçalho `.hgr`
/// (`docs/01 §8`), e por isso `M3` não existe: na sweep 1..32 ele é apenas
/// `higher-16:M2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Régua: `base-8` puro.
    M0 = 0,
    /// Legado + camada Tipo 1 (humana), ordem de preenchimento fixada à mão.
    M1 = 1,
    /// Legado + blocos escolhidos por função de ganho no corpus de treino.
    M2 = 2,
    /// `M2` mais o namespace adaptativo sincronizado.
    M4 = 4,
}

impl Variant {
    /// Byte que representa a variante no cabeçalho `.hgr`.
    #[must_use]
    pub const fn to_wire(self) -> u8 {
        self as u8
    }

    /// Lê a variante a partir do byte do cabeçalho.
    ///
    /// Devolve `None` para qualquer valor fora de `{0, 1, 2, 4}` — inclusive
    /// `3`, que nunca foi emitido (ver a nota sobre `M3` acima).
    #[must_use]
    pub const fn from_wire(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::M0),
            1 => Some(Self::M1),
            2 => Some(Self::M2),
            4 => Some(Self::M4),
            _ => None,
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::M0 => "M0",
            Self::M1 => "M1",
            Self::M2 => "M2",
            Self::M4 => "M4",
        };
        f.write_str(s)
    }
}

/// Erro de construção de [`Width`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidthOutOfRange {
    /// O valor recusado.
    pub bits: u32,
}

impl fmt::Display for WidthOutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "largura de símbolo {} fora da faixa {}..={}",
            self.bits, MIN_BITS, MAX_BITS
        )
    }
}

impl std::error::Error for WidthOutOfRange {}

/// Largura de símbolo validada, sempre em `1..=32`.
///
/// Existir como tipo evita que uma largura inválida chegue ao packing e
/// vire `shift overflow` lá dentro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Width(u8);

impl Width {
    /// A largura da régua, `base-8`.
    ///
    /// Existe como constante porque o codec precisa dela em caminhos que
    /// não podem entrar em pânico (ler os 8 bits brutos de um ESCAPE, por
    /// exemplo), e `Width::new(8).unwrap()` seria um `panic!` escondido
    /// justamente onde o aceite da E2 proíbe um.
    pub const BASE: Self = Self(BASE_BITS);

    /// Constrói a largura, recusando qualquer valor fora de `1..=32`.
    ///
    /// # Errors
    ///
    /// Devolve [`WidthOutOfRange`] se `bits` não estiver em `1..=32`.
    pub const fn new(bits: u8) -> Result<Self, WidthOutOfRange> {
        if bits >= MIN_BITS && bits <= MAX_BITS {
            Ok(Self(bits))
        } else {
            Err(WidthOutOfRange { bits: bits as u32 })
        }
    }

    /// Bits por símbolo.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Faixa da largura.
    #[must_use]
    pub const fn band(self) -> Band {
        if self.0 < BASE_BITS {
            Band::Lower
        } else if self.0 == BASE_BITS {
            Band::Base
        } else {
            Band::Higher
        }
    }

    /// Capacidade da tabela, `2^b`.
    ///
    /// Devolve `u64` porque `higher-32` já são 4.294.967.296 slots — o que
    /// não significa que caibam em memória (`docs/01 §4`).
    #[must_use]
    pub const fn capacity(self) -> u64 {
        1u64 << self.0
    }

    /// Contêiner de RAM alinhado que comporta um símbolo (`docs/00 §3.3`).
    ///
    /// Devolve o tamanho em bytes: 1, 2 ou 4.
    #[must_use]
    pub const fn container_bytes(self) -> u8 {
        if self.0 <= 8 {
            1
        } else if self.0 <= 16 {
            2
        } else {
            4
        }
    }

    /// `true` quando a largura é nativa (8, 16 ou 32) e o packing é cópia direta.
    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(self.0, 8 | 16 | 32)
    }

    /// Quantos símbolos fecham um número inteiro de bytes: `lcm(b, 8) / b`.
    ///
    /// É o tamanho do grupo do caminho especializado de packing (`docs/01 §7`).
    #[must_use]
    pub const fn pack_group_symbols(self) -> u8 {
        // gcd(b, 8) é sempre uma potência de dois <= 8, então cabe em u8.
        let b = self.0;
        let mut a = b;
        let mut c = 8u8;
        while c != 0 {
            let t = a % c;
            a = c;
            c = t;
        }
        8 / a
    }

    /// Quantos bytes o grupo de [`Self::pack_group_symbols`] ocupa: `lcm(b, 8) / 8`.
    #[must_use]
    pub const fn pack_group_bytes(self) -> u8 {
        let b = self.0;
        let mut a = b;
        let mut c = 8u8;
        while c != 0 {
            let t = a % c;
            a = c;
            c = t;
        }
        b / a
    }

    /// Limiar de ganho da largura, como fração em `0.0..1.0`.
    ///
    /// Para HIGHER é a redução horizontal mínima `1 − 8/b` (`docs/00 §3.1`):
    /// abaixo dela o fluxo fica maior que `base-8`. Para LOWER é a cobertura
    /// mínima sem escape `b/8` (`docs/00 §3.2`). Para `base-8`, que é a
    /// régua, é `0.0`.
    ///
    /// É um limiar derivado da aritmética, não uma medição — ele diz onde o
    /// ganho começaria a existir, nunca que ele existe.
    #[must_use]
    pub fn threshold(self) -> f64 {
        let b = f64::from(self.0);
        match self.band() {
            Band::Lower => b / 8.0,
            Band::Base => 0.0,
            Band::Higher => 1.0 - 8.0 / b,
        }
    }

    /// Nome canônico do modelo: `lower-3`, `base-8`, `higher-14`.
    #[must_use]
    pub fn model_name(self) -> String {
        match self.band() {
            Band::Lower => format!("lower-{}", self.0),
            Band::Base => "base-8".to_string(),
            Band::Higher => format!("higher-{}", self.0),
        }
    }

    /// Nome de run: `higher-14:M2`, `lower-6:M2`, `base-8`.
    ///
    /// `base-8` não leva sufixo porque a régua é sempre `M0`.
    #[must_use]
    pub fn run_name(self, variant: Variant) -> String {
        if self.band() == Band::Base && variant == Variant::M0 {
            return self.model_name();
        }
        format!("{}:{variant}", self.model_name())
    }
}

impl fmt::Display for Width {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u8> for Width {
    type Error = WidthOutOfRange;

    fn try_from(bits: u8) -> Result<Self, Self::Error> {
        Self::new(bits)
    }
}

/// Todas as larguras da sweep, de 1 a 32, na ordem.
///
/// A sweep não pula largura nenhuma: pular era o defeito D8 do protótipo
/// anterior (`docs/05 §3`).
#[must_use]
pub fn all_widths() -> Vec<Width> {
    (MIN_BITS..=MAX_BITS).map(Width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sweep_cobre_1_ate_32_sem_pular() {
        let widths = all_widths();
        assert_eq!(widths.len(), 32);
        for (i, w) in widths.iter().enumerate() {
            assert_eq!(w.bits() as usize, i + 1);
        }
    }

    #[test]
    fn largura_fora_da_faixa_e_recusada() {
        assert!(Width::new(0).is_err());
        assert!(Width::new(33).is_err());
        assert!(Width::new(255).is_err());
        assert!(Width::new(1).is_ok());
        assert!(Width::new(32).is_ok());
    }

    #[test]
    fn faixas_seguem_o_corte_em_8() {
        for w in all_widths() {
            let esperado = match w.bits() {
                1..=7 => Band::Lower,
                8 => Band::Base,
                _ => Band::Higher,
            };
            assert_eq!(w.band(), esperado, "b = {}", w.bits());
        }
    }

    #[test]
    fn nomes_de_modelo_batem_com_claude_md() {
        let nome = |b: u8| Width::new(b).unwrap().model_name();
        assert_eq!(nome(1), "lower-1");
        assert_eq!(nome(7), "lower-7");
        assert_eq!(nome(8), "base-8");
        assert_eq!(nome(9), "higher-9");
        assert_eq!(nome(14), "higher-14");
        assert_eq!(nome(32), "higher-32");
    }

    #[test]
    fn nomes_de_run_levam_a_variante_menos_a_regua() {
        let w14 = Width::new(14).unwrap();
        assert_eq!(w14.run_name(Variant::M2), "higher-14:M2");
        assert_eq!(w14.run_name(Variant::M4), "higher-14:M4");
        assert_eq!(Width::new(6).unwrap().run_name(Variant::M2), "lower-6:M2");
        assert_eq!(Width::new(8).unwrap().run_name(Variant::M0), "base-8");
    }

    #[test]
    fn capacidade_e_contêiner_seguem_a_tabela_da_tese() {
        let cap = |b: u8| Width::new(b).unwrap().capacity();
        assert_eq!(cap(1), 2);
        assert_eq!(cap(8), 256);
        assert_eq!(cap(14), 16_384);
        assert_eq!(cap(16), 65_536);
        assert_eq!(cap(32), 4_294_967_296);

        let cont = |b: u8| Width::new(b).unwrap().container_bytes();
        assert_eq!(cont(1), 1);
        assert_eq!(cont(8), 1);
        assert_eq!(cont(9), 2);
        assert_eq!(cont(16), 2);
        assert_eq!(cont(17), 4);
        assert_eq!(cont(32), 4);
    }

    #[test]
    fn larguras_nativas_sao_8_16_32() {
        for w in all_widths() {
            assert_eq!(w.is_native(), matches!(w.bits(), 8 | 16 | 32), "b = {w}");
        }
    }

    #[test]
    fn grupos_de_packing_batem_com_a_coluna_da_tese() {
        // (b, símbolos -> bytes) copiado de docs/00 §3.3.
        let esperado: [(u8, u8, u8); 12] = [
            (1, 8, 1),
            (2, 4, 1),
            (3, 8, 3),
            (4, 2, 1),
            (8, 1, 1),
            (12, 2, 3),
            (14, 4, 7),
            (16, 1, 2),
            (20, 2, 5),
            (24, 1, 3),
            (28, 2, 7),
            (32, 1, 4),
        ];
        for (b, simbolos, bytes) in esperado {
            let w = Width::new(b).unwrap();
            assert_eq!(w.pack_group_symbols(), simbolos, "b = {b}");
            assert_eq!(w.pack_group_bytes(), bytes, "b = {b}");
        }
    }

    #[test]
    fn grupo_de_packing_fecha_byte_em_toda_largura() {
        for w in all_widths() {
            let bits = u32::from(w.pack_group_symbols()) * u32::from(w.bits());
            assert_eq!(bits % 8, 0, "b = {w}");
            assert_eq!(bits / 8, u32::from(w.pack_group_bytes()), "b = {w}");
        }
    }

    #[test]
    fn limiares_batem_com_a_matematica_da_tese() {
        let t = |b: u8| Width::new(b).unwrap().threshold();
        // HIGHER: 1 - 8/b (docs/00 §3.1).
        assert!((t(9) - 0.111_111).abs() < 1e-5);
        assert!((t(12) - 0.333_333).abs() < 1e-5);
        assert!((t(14) - 0.428_571).abs() < 1e-5);
        assert!((t(16) - 0.5).abs() < 1e-12);
        assert!((t(32) - 0.75).abs() < 1e-12);
        // LOWER: cobertura sem escape > b/8 (docs/00 §3.2).
        assert!((t(1) - 0.125).abs() < 1e-12);
        assert!((t(4) - 0.5).abs() < 1e-12);
        assert!((t(7) - 0.875).abs() < 1e-12);
        // BASE é a régua.
        assert!((t(8) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn variante_3_nao_existe_no_fio() {
        assert_eq!(Variant::from_wire(3), None);
        assert_eq!(Variant::from_wire(5), None);
        for v in [Variant::M0, Variant::M1, Variant::M2, Variant::M4] {
            assert_eq!(Variant::from_wire(v.to_wire()), Some(v));
        }
    }
}
