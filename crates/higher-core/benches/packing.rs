//! Microbench de pack/unpack por largura (aceite da Etapa E1).
//!
//! Mede três coisas, para cada `b` de 1 a 32:
//!
//! - `pack/b=NN` e `unpack/b=NN` — o caminho de trabalho, já despachado pela
//!   estratégia da largura;
//! - `pack_generic/b=NN` e `unpack_generic/b=NN` — o caminho genérico na
//!   mesma largura. A razão entre os dois é **o ganho do caminho
//!   especializado**, que `docs/01 §7` manda reportar.
//!
//! O que está sendo medido é packing puro: não há tabela, tokenização nem
//! entrada real. Nada aqui vira `enc_ns_per_byte` de `docs/02 §5` — aquilo
//! é a E5, com o protocolo de `docs/02 §6` (máquina dedicada, `taskset`,
//! governor). Este bench serve para comparar larguras entre si e para
//! detectar regressão via baseline do criterion.

// `criterion_group!` gera itens públicos sem doc; o lint `missing_docs` do
// crate não se aplica a um alvo de bench, que não é API.
#![allow(missing_docs)]

use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use higher_core::model::{all_widths, Width};
use higher_core::packing::{
    max_value, pack, pack_via_generic, unpack, unpack_via_generic, Strategy,
};

/// Símbolos por medição. Um fluxo grande o bastante para amortizar o setup
/// e pequeno o bastante para caber em cache (16 384 × 4 B = 64 KB no pior
/// caso), de modo que o bench meça packing, não a hierarquia de memória.
const N_SIMBOLOS: usize = 16_384;

/// Seed fixa: o mesmo input para todas as larguras e execuções (`docs/02 §6.5`).
const SEED: u64 = 42;

struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn valores(width: Width) -> Vec<u32> {
    let max = max_value(width.bits());
    let mut rng = SplitMix64(SEED ^ u64::from(width.bits()));
    // Truncar é o objetivo: o resto já está em `0..=max`, que cabe em u32.
    #[allow(clippy::cast_possible_truncation)]
    (0..N_SIMBOLOS)
        .map(|_| {
            if max == u32::MAX {
                rng.next_u64() as u32
            } else {
                (rng.next_u64() % (u64::from(max) + 1)) as u32
            }
        })
        .collect()
}

/// Sufixo do nome do bench, para a estratégia aparecer no relatório do criterion.
fn rotulo(width: Width) -> String {
    format!("b={:02}/{}", width.bits(), Strategy::of(width).label())
}

fn bench_pack(c: &mut Criterion) {
    let mut grupo = c.benchmark_group("pack");
    grupo.throughput(Throughput::Elements(N_SIMBOLOS as u64));
    for width in all_widths() {
        let vals = valores(width);
        grupo.bench_with_input(BenchmarkId::from_parameter(rotulo(width)), &vals, |b, v| {
            b.iter(|| pack(std::hint::black_box(v), width).unwrap());
        });
    }
    grupo.finish();
}

fn bench_unpack(c: &mut Criterion) {
    let mut grupo = c.benchmark_group("unpack");
    grupo.throughput(Throughput::Elements(N_SIMBOLOS as u64));
    for width in all_widths() {
        let bytes = pack(&valores(width), width).unwrap();
        grupo.bench_with_input(
            BenchmarkId::from_parameter(rotulo(width)),
            &bytes,
            |b, s| {
                b.iter(|| unpack(std::hint::black_box(s), N_SIMBOLOS, width).unwrap());
            },
        );
    }
    grupo.finish();
}

/// O mesmo trabalho, forçado pelo caminho genérico.
///
/// Só faz sentido nas larguras que **têm** caminho especializado: onde a
/// estratégia já é `Generic`, os dois números seriam o mesmo bench rodado
/// duas vezes.
fn bench_pack_generic(c: &mut Criterion) {
    let mut grupo = c.benchmark_group("pack_generic");
    grupo.throughput(Throughput::Elements(N_SIMBOLOS as u64));
    for width in all_widths() {
        if Strategy::of(width) == Strategy::Generic {
            continue;
        }
        let vals = valores(width);
        grupo.bench_with_input(BenchmarkId::from_parameter(rotulo(width)), &vals, |b, v| {
            b.iter(|| pack_via_generic(std::hint::black_box(v), width).unwrap());
        });
    }
    grupo.finish();
}

fn bench_unpack_generic(c: &mut Criterion) {
    let mut grupo = c.benchmark_group("unpack_generic");
    grupo.throughput(Throughput::Elements(N_SIMBOLOS as u64));
    for width in all_widths() {
        if Strategy::of(width) == Strategy::Generic {
            continue;
        }
        let bytes = pack(&valores(width), width).unwrap();
        grupo.bench_with_input(
            BenchmarkId::from_parameter(rotulo(width)),
            &bytes,
            |b, s| {
                b.iter(|| unpack_via_generic(std::hint::black_box(s), N_SIMBOLOS, width).unwrap());
            },
        );
    }
    grupo.finish();
}

fn config() -> Criterion {
    // 32 larguras × 4 benches. Com o padrão do criterion (3 s de warmup,
    // 5 s de medição) isso passaria de 10 minutos. Estes valores mantêm a
    // suíte em poucos minutos sem abrir mão da estatística; a E5, que é a
    // medição oficial, usa o protocolo completo de `docs/02 §6`.
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50)
}

criterion_group! {
    name = benches;
    config = config();
    targets = bench_pack, bench_unpack, bench_pack_generic, bench_unpack_generic
}
criterion_main!(benches);
