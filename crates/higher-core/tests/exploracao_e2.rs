//! Exploração da E2 — **não** é aceite e **não** é medição da tese.
//!
//! Estes testes são `#[ignore]` de propósito: eles imprimem tabelas, não
//! verificam nada. Existem para que os números do relatório
//! `results/ETAPA-02.md` sejam reproduzíveis com um comando, e para dar um
//! smoke check de que o codec de fato reduz bytes onde deveria.
//!
//! O que eles **não** são:
//!
//! - não há corpus real (`docs/02 §2.1`), só sintético;
//! - não há baseline nenhuma (gzip, zstd, brotli, lz4, BPE — `docs/02 §1`);
//! - não há repetição, seed múltipla nem intervalo de confiança.
//!
//! Ou seja: nada aqui sustenta ou refuta H1–H7. A sweep de verdade é a E4.
//!
//! ```bash
//! cargo test -p higher-core --release --test exploracao_e2 -- --ignored --nocapture
//! ```

// Corpora de exploração ficam abaixo de 10 MB (2^24), muito longe dos 2^53
// em que um inteiro começaria a perder precisão num f64.
#![allow(clippy::cast_precision_loss)]

use higher_core::codec::Model;
use higher_core::model::{all_widths, Width};
use higher_core::table::{dynamic_slots, Table};

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

fn de_alfabeto(alfabeto: &[u8], n: usize, seed: u64) -> Vec<u8> {
    let mut r = SplitMix64(seed);
    (0..n)
        .map(|_| alfabeto[usize::try_from(r.next_u64() % alfabeto.len() as u64).unwrap()])
        .collect()
}

fn logs(n_linhas: usize, seed: u64) -> Vec<u8> {
    let mut r = SplitMix64(seed);
    let mut v = Vec::new();
    for _ in 0..n_linhas {
        let m = r.next_u64();
        v.extend_from_slice(
            format!(
                "2026-09-{:02}T{:02}:{:02}:{:02}Z {} /api/{} {} {}ms\n",
                m % 28 + 1,
                m >> 8 & 0x17,
                m >> 16 & 0x3b,
                m >> 24 & 0x3b,
                ["GET", "POST", "PUT"][usize::try_from(m >> 32 & 0x2).unwrap()],
                ["runs", "aggregate", "import", "version"][usize::try_from(m >> 40 & 0x3).unwrap()],
                [200, 201, 404, 500][usize::try_from(m >> 44 & 0x3).unwrap()],
                m % 900 + 10,
            )
            .as_bytes(),
        );
    }
    v
}

fn tabela(nome: &str, treino: &[u8], teste: &[u8]) {
    println!(
        "\n=== {nome} — treino {} B, teste {} B ===",
        treino.len(),
        teste.len()
    );
    println!(
        "{:>3} {:>12} {:>8} {:>9} {:>10} {:>8} {:>7} {:>10} {:>8}",
        "b", "slots", "usados", "ocup%", "tab.bytes", "b/B", "esc%", "red.horiz", "limiar"
    );
    for w in all_widths() {
        let t = Table::train_m2(w, treino);
        let (tb, usados, ocup) = (t.table_bytes(), t.slots_used(), t.occupancy());
        let m = Model::new(t);
        let (_, s) = m.encode(teste).expect("encode");
        assert_eq!(
            m.decode(&m.encode(teste).unwrap().0).unwrap(),
            teste,
            "roundtrip"
        );
        let marca = if s.bits_per_byte() < 8.0 { " <" } else { "" };
        println!(
            "{:>3} {:>12} {:>8} {:>8.3}% {:>10} {:>8.3} {:>6.1}% {:>9.1}% {:>7.1}%{}",
            w.bits(),
            dynamic_slots(w),
            usados,
            ocup * 100.0,
            tb,
            s.bits_per_byte(),
            s.escape_rate() * 100.0,
            s.horiz_reduction() * 100.0,
            w.threshold() * 100.0,
            marca
        );
    }
}

#[test]
#[ignore = "exploração: imprime tabelas, não verifica nada. A sweep é a E4."]
fn panorama_por_corpus_sintetico() {
    tabela("logs sintéticos", &logs(1200, 1), &logs(400, 999));
    tabela(
        "DNA (alfabeto de 4)",
        &de_alfabeto(b"ACGT", 200_000, 1),
        &de_alfabeto(b"ACGT", 50_000, 999),
    );
    tabela(
        "dígitos (alfabeto de 10)",
        &de_alfabeto(b"0123456789", 200_000, 2),
        &de_alfabeto(b"0123456789", 50_000, 888),
    );
    tabela(
        "bytes aleatórios (controle negativo)",
        &de_alfabeto(&(0..=255u8).collect::<Vec<_>>(), 200_000, 3),
        &de_alfabeto(&(0..=255u8).collect::<Vec<_>>(), 50_000, 777),
    );
}

/// Quanto o cabeçalho fixo de 36 bytes pesa, por tamanho de entrada.
///
/// É o insumo para a ambiguidade §4.6 de `results/ETAPA-00.md`: se a régua
/// `base-8` pagar o cabeçalho, ela sai de 8,000 b/B e "vencer a régua" fica
/// mais fácil do que deveria em entradas pequenas.
#[test]
#[ignore = "exploração: insumo para a decisão da régua na E4."]
fn custo_do_cabecalho_por_tamanho_de_entrada() {
    println!("\n=== overhead do cabeçalho de 36 B em base-8 (docs/02 §4) ===");
    println!("{:>12} {:>10} {:>12}", "entrada B", "b/B", "overhead%");
    let m = Model::base8();
    for n in [1_000usize, 10_000, 100_000, 1_000_000, 10_000_000] {
        let entrada = vec![b'x'; n];
        let (_, s) = m.encode(&entrada).expect("encode");
        println!(
            "{:>12} {:>10.4} {:>11.3}%",
            n,
            s.bits_per_byte(),
            36.0 / s.encoded_bytes as f64 * 100.0
        );
    }
}

/// Um símbolo por largura: quantos bytes ele precisa cobrir para pagar.
#[test]
#[ignore = "exploração: aritmética de docs/00 §3, não medição."]
fn cobertura_media_por_simbolo() {
    println!("\n=== bytes por símbolo medido vs. exigido (docs/00 §3.1) ===");
    let treino = logs(1200, 1);
    let teste = logs(400, 999);
    println!(
        "{:>3} {:>14} {:>14} {:>8}",
        "b", "bytes/símbolo", "exigido b/8", "folga"
    );
    for w in all_widths().into_iter().filter(|w| w.bits() >= 8) {
        let m = Model::new(Table::train_m2(w, &treino));
        let (_, s) = m.encode(&teste).expect("encode");
        let medido = teste.len() as f64 / s.n_symbols as f64;
        let exigido = f64::from(w.bits()) / 8.0;
        println!(
            "{:>3} {:>14.3} {:>14.3} {:>7.2}x",
            w.bits(),
            medido,
            exigido,
            medido / exigido
        );
    }
    let _ = Width::BASE;
}
