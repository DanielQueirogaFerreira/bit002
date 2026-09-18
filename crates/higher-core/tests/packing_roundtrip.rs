//! Roundtrip do packing nas 32 larguras (aceite da Etapa E1).
//!
//! Dois regimes complementares:
//!
//! - **proptest** — sequências e larguras sorteadas pelo motor, com redução
//!   automática do contraexemplo quando algo falha;
//! - **volume** — o critério de aceite da E1 (≥ 10⁵ casos aleatórios por
//!   largura), com PRNG determinístico e seed registrada, para que uma
//!   falha seja reproduzível byte a byte.
//!
//! "Caso aleatório" aqui é **um símbolo sorteado e roundtripado**. A leitura
//! alternativa — 10⁵ *sequências* por largura — daria 3,2 milhões de
//! sequências em `cargo test`, que roda em debug: minutos por execução, numa
//! suíte que precisa rodar em toda CI. O desvio está registrado em
//! `results/ETAPA-01.md`.

use higher_core::model::{all_widths, Width};
use higher_core::packing::{max_value, pack, packed_len, unpack, BitReader, BitWriter};
use proptest::prelude::*;

/// Seed do PRNG dos testes de volume. Fixa e registrada (`docs/02 §6.5`).
const SEED: u64 = 42;

/// Casos aleatórios por largura no teste de volume (aceite da E1).
const CASOS_POR_LARGURA: usize = 100_000;

/// `SplitMix64` — determinístico, sem dependência, idêntico entre máquinas.
///
/// O `rand` do proptest serve ao proptest; aqui o que importa é que a mesma
/// seed produza exatamente a mesma sequência em qualquer lugar, hoje e na
/// E10, para que um contraexemplo continue reproduzível.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Valor uniforme em `0..=max`.
    fn valor_ate(&mut self, max: u32) -> u32 {
        if max == u32::MAX {
            #[allow(clippy::cast_possible_truncation)]
            return self.next_u64() as u32;
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            (self.next_u64() % (u64::from(max) + 1)) as u32
        }
    }
}

#[test]
fn volume_100k_casos_por_largura() {
    for width in all_widths() {
        let max = max_value(width.bits());
        let mut rng = SplitMix64::new(SEED ^ u64::from(width.bits()));

        // Um fluxo único com 100 000 símbolos sorteados. Mantê-los num só
        // fluxo exercita o encadeamento entre grupos, que é onde um bug de
        // acumulador se esconderia.
        let valores: Vec<u32> = (0..CASOS_POR_LARGURA).map(|_| rng.valor_ate(max)).collect();

        let bytes = pack(&valores, width).expect("valores cabem na largura");
        assert_eq!(
            bytes.len(),
            packed_len(valores.len(), width),
            "b = {width}: tamanho empacotado fora do previsto"
        );

        let voltou = unpack(&bytes, valores.len(), width).expect("roundtrip");
        assert_eq!(
            voltou, valores,
            "b = {width}: roundtrip divergiu (seed {SEED})"
        );
    }
}

#[test]
fn volume_em_sequencias_curtas_de_comprimento_sorteado() {
    // O mesmo volume, agora fatiado em milhares de sequências curtas de
    // comprimento aleatório: é o que expõe resto de grupo e completamento.
    for width in all_widths() {
        let max = max_value(width.bits());
        let mut rng = SplitMix64::new(SEED.wrapping_mul(31) ^ u64::from(width.bits()));
        let mut total = 0usize;

        while total < CASOS_POR_LARGURA {
            let n = (rng.next_u64() % 65) as usize;
            let valores: Vec<u32> = (0..n).map(|_| rng.valor_ate(max)).collect();
            let bytes = pack(&valores, width).expect("valores cabem na largura");
            assert_eq!(bytes.len(), packed_len(n, width), "b = {width}, n = {n}");
            assert_eq!(
                unpack(&bytes, n, width).expect("roundtrip"),
                valores,
                "b = {width}, n = {n} (seed {SEED})"
            );
            total += n.max(1);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Roundtrip de sequência aleatória, com a largura também sorteada.
    #[test]
    fn roundtrip_sequencia_aleatoria(
        bits in 1u8..=32,
        semente in any::<u64>(),
        n in 0usize..300,
    ) {
        let width = Width::new(bits).unwrap();
        let max = max_value(bits);
        let mut rng = SplitMix64::new(semente);
        let valores: Vec<u32> = (0..n).map(|_| rng.valor_ate(max)).collect();

        let bytes = pack(&valores, width).unwrap();
        prop_assert_eq!(bytes.len(), packed_len(n, width));
        prop_assert_eq!(unpack(&bytes, n, width).unwrap(), valores);
    }

    /// Valores dados pelo proptest, mascarados para a largura sorteada.
    ///
    /// Difere do teste acima por deixar o motor escolher os valores — e com
    /// isso favorecer extremos (0, máximo, potências de dois).
    #[test]
    fn roundtrip_valores_do_proptest(
        bits in 1u8..=32,
        brutos in prop::collection::vec(any::<u32>(), 0..200),
    ) {
        let width = Width::new(bits).unwrap();
        let max = max_value(bits);
        let valores: Vec<u32> = brutos.iter().map(|&v| v & max).collect();

        let bytes = pack(&valores, width).unwrap();
        prop_assert_eq!(unpack(&bytes, valores.len(), width).unwrap(), valores);
    }

    /// Escrita desalinhada: um prefixo de largura arbitrária antes da sequência.
    #[test]
    fn roundtrip_desalinhado(
        prefixo_bits in 1u8..=32,
        bits in 1u8..=32,
        semente in any::<u64>(),
        n in 0usize..120,
    ) {
        let w_prefixo = Width::new(prefixo_bits).unwrap();
        let width = Width::new(bits).unwrap();
        let mut rng = SplitMix64::new(semente);

        let prefixo = rng.valor_ate(max_value(prefixo_bits));
        let valores: Vec<u32> = (0..n).map(|_| rng.valor_ate(max_value(bits))).collect();

        let mut bw = BitWriter::new();
        bw.push(prefixo, w_prefixo).unwrap();
        bw.push_all(&valores, width).unwrap();
        let bytes = bw.finish();

        let mut br = BitReader::new(&bytes);
        prop_assert_eq!(br.read(w_prefixo), Some(prefixo));
        let mut voltou = Vec::new();
        br.read_all_into(&mut voltou, n, width).unwrap();
        prop_assert_eq!(voltou, valores);
    }

    /// Valor que não cabe na largura é sempre recusado, nunca mascarado.
    #[test]
    fn valor_largo_demais_nunca_passa(bits in 1u8..=31, extra in 1u32..1000) {
        let width = Width::new(bits).unwrap();
        let valor = max_value(bits).saturating_add(extra);
        prop_assume!(valor > max_value(bits));
        prop_assert!(pack(&[valor], width).is_err());
    }
}
