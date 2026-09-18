//! Aceite da Etapa E2.
//!
//! > Roundtrip em todo `b` 1..32 sobre fuzz de bytes arbitrários (incluindo
//! > os 256 valores). Decoder rejeita `table_id` errado e crc inválido com
//! > erro tipado, sem panic.
//!
//! O roundtrip roda sobre **duas** tabelas por largura: a `legacy_only`
//! (piso da faixa, sem treino) e uma M2 treinada num corpus de treino
//! **disjunto** do corpus de teste. O segundo regime é o que exercita a
//! trie, os blocos multibyte e o escape do LOWER de verdade.

use std::sync::OnceLock;

use higher_core::codec::{DecodeError, Model};
use higher_core::hgr::{Header, HEADER_BYTES};
use higher_core::model::{all_widths, Band, Variant, Width};
use higher_core::table::{Table, TableError};
use proptest::prelude::*;

/// Seed registrada (`docs/02 §6.5`).
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

    #[allow(clippy::cast_possible_truncation)]
    fn byte(&mut self) -> u8 {
        self.next_u64() as u8
    }
}

/// Corpus de **treino**: texto com vocabulário repetido, que é onde um
/// dicionário tem o que aprender.
fn corpus_treino() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..200 {
        v.extend_from_slice(b"GET /api/runs?bits=14&variant=M2 HTTP/1.1 200 OK\n");
        v.extend_from_slice(b"{\"model\":\"higher-14\",\"ok\":true,\"seed\":42}\n");
        v.extend_from_slice(
            format!("2026-09-18T07:{:02}:00Z INFO encode done\n", i % 60).as_bytes(),
        );
    }
    v
}

/// Corpus de **teste**: mesma natureza, conteúdo diferente. Nunca é o de treino.
fn corpus_teste() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..150 {
        v.extend_from_slice(b"POST /api/import?env=native HTTP/1.1 201 CREATED\n");
        v.extend_from_slice(b"{\"model\":\"higher-16\",\"ok\":false,\"seed\":7}\n");
        v.extend_from_slice(
            format!("2026-09-19T21:{:02}:30Z WARN decode slow\n", i % 60).as_bytes(),
        );
    }
    v
}

/// Todas as entradas de borda que o aceite exige, mais fuzz determinístico.
fn entradas_de_borda() -> Vec<(&'static str, Vec<u8>)> {
    let mut rng = SplitMix64(SEED);
    let mut casos: Vec<(&'static str, Vec<u8>)> = vec![
        ("vazio", Vec::new()),
        ("um byte 0x00", vec![0x00]),
        ("um byte 0xFF", vec![0xFF]),
        ("os 256 bytes", (0..=255u8).collect()),
        ("os 256 bytes invertidos", (0..=255u8).rev().collect()),
        ("tudo zero, 1000", vec![0u8; 1000]),
        ("tudo 0xFF, 1000", vec![0xFFu8; 1000]),
        ("corpus de teste", corpus_teste()),
    ];
    casos.push(("fuzz 4096 bytes", (0..4096).map(|_| rng.byte()).collect()));
    casos.push((
        "fuzz de alfabeto restrito",
        (0..4096)
            .map(|_| b"ACGT"[(rng.byte() % 4) as usize])
            .collect(),
    ));
    casos
}

/// Tabelas M2 das 32 larguras, treinadas uma vez só.
///
/// Sem o cache, os testes que varrem as larguras (e o proptest, que sorteia
/// uma por caso) retreinam a mesma tabela centenas de vezes e a suíte passa
/// de um minuto. Treinar é determinístico, então cachear não muda o que
/// está sendo verificado.
fn m2_da_largura(w: Width) -> &'static Table {
    static CACHE: OnceLock<Vec<Table>> = OnceLock::new();
    let tabelas = CACHE.get_or_init(|| {
        let treino = corpus_treino();
        all_widths()
            .into_iter()
            .map(|w| Table::train_m2(w, &treino))
            .collect()
    });
    &tabelas[usize::from(w.bits()) - 1]
}

fn modelos_da_largura(w: Width) -> Vec<(&'static str, Model)> {
    vec![
        (
            "legacy_only",
            Model::new(Table::legacy_only(w, Variant::M1)),
        ),
        ("M2 treinada", Model::new(m2_da_largura(w).clone())),
    ]
}

#[test]
fn roundtrip_em_toda_largura_e_toda_entrada_de_borda() {
    let casos = entradas_de_borda();

    for w in all_widths() {
        for (nome_tab, modelo) in modelos_da_largura(w) {
            for (nome_caso, entrada) in &casos {
                let (fluxo, stats) = modelo.encode(entrada).unwrap_or_else(|e| {
                    panic!("encode falhou em b={w} {nome_tab} {nome_caso}: {e}")
                });

                assert_eq!(stats.input_bytes, entrada.len() as u64);
                assert_eq!(stats.header_bytes, HEADER_BYTES as u64);
                assert_eq!(stats.encoded_bytes, fluxo.len() as u64);

                let voltou = modelo.decode(&fluxo).unwrap_or_else(|e| {
                    panic!("decode falhou em b={w} {nome_tab} {nome_caso}: {e}")
                });
                assert_eq!(&voltou, entrada, "b={w} {nome_tab} {nome_caso}");

                // O roundtrip sozinho é cego: continuaria passando com a trie
                // desligada, porque todo byte cairia no código legado.
                // Confirmado por mutação — daí a asserção abaixo.
                if nome_tab == "M2 treinada"
                    && *nome_caso == "corpus de teste"
                    && w.band() == Band::Higher
                {
                    assert!(
                        stats.horiz_reduction() > 0.0,
                        "b={w}: a M2 não reduziu símbolo nenhum — a trie não está casando"
                    );
                    assert!(
                        stats.n_symbols < entrada.len() as u64,
                        "b={w}: n_symbols não caiu abaixo do número de bytes"
                    );
                }
            }
        }
    }
}

#[test]
fn treino_e_teste_sao_disjuntos_neste_arquivo() {
    // A garantia mais importante do projeto (docs/02 §2.3, defeito D1 de
    // docs/05 §3) é que a tabela nunca vê o corpus de teste. Aqui isso é
    // estrutural, mas fica asseverado para que ninguém "simplifique" os
    // dois corpora num só depois.
    let treino = corpus_treino();
    let teste = corpus_teste();
    assert_ne!(treino, teste);
    assert!(!treino
        .windows(48)
        .any(|w| teste.windows(48).any(|t| t == w)));
}

#[test]
fn decoder_rejeita_table_id_errado_com_erro_tipado() {
    let entrada = corpus_teste();

    for w in all_widths() {
        let certo = Model::new(m2_da_largura(w).clone());
        // Outra tabela: mesma largura e variante, treino diferente.
        let outro = Model::new(Table::train_m2(
            w,
            b"um corpus de treino completamente diferente ",
        ));
        assert_ne!(certo.table().id(), outro.table().id(), "b={w}");

        let (fluxo, _) = certo.encode(&entrada).unwrap();
        match outro.decode(&fluxo) {
            Err(DecodeError::TableMismatch { expected, got }) => {
                assert_eq!(expected, certo.table().id8(), "b={w}");
                assert_eq!(got, outro.table().id8(), "b={w}");
            }
            outro_resultado => {
                panic!("b={w}: esperado TableMismatch, veio {outro_resultado:?}")
            }
        }
    }
}

#[test]
fn decoder_rejeita_crc_invalido_com_erro_tipado() {
    let entrada = corpus_teste();

    for w in all_widths() {
        let modelo = Model::new(m2_da_largura(w).clone());
        let (fluxo, _) = modelo.encode(&entrada).unwrap();
        assert!(fluxo.len() > HEADER_BYTES, "b={w}: payload vazio");

        // Vira um bit do payload sem mexer no cabeçalho.
        let mut corrompido = fluxo.clone();
        corrompido[HEADER_BYTES] ^= 0b0000_0001;
        match modelo.decode(&corrompido) {
            Err(DecodeError::CrcMismatch { expected, got }) => {
                assert_ne!(expected, got, "b={w}");
            }
            outro => panic!("b={w}: esperado CrcMismatch, veio {outro:?}"),
        }

        // E o caminho inverso: mexer só no campo de crc do cabeçalho.
        let mut crc_torto = fluxo.clone();
        crc_torto[35] ^= 0xFF;
        assert!(
            matches!(
                modelo.decode(&crc_torto),
                Err(DecodeError::CrcMismatch { .. })
            ),
            "b={w}: crc adulterado no cabeçalho devia falhar"
        );
    }
}

#[test]
fn decoder_nao_entra_em_panico_com_fluxo_arbitrario() {
    // O aceite pede "sem panic". Qualquer sequência de bytes é entrada
    // possível para um decoder que um dia leia arquivo de disco ou corpo de
    // request; nenhuma delas pode derrubar o processo.
    let mut rng = SplitMix64(SEED ^ 0xDEAD);

    for w in all_widths() {
        let modelo = Model::new(m2_da_largura(w).clone());
        let (valido, _) = modelo.encode(&corpus_teste()).unwrap();

        for _ in 0..200 {
            // Lixo puro.
            let n = usize::try_from(rng.next_u64() % 200).unwrap();
            let lixo: Vec<u8> = (0..n).map(|_| rng.byte()).collect();
            let _ = modelo.decode(&lixo);

            // Fluxo válido com um byte corrompido em posição aleatória.
            let mut mutante = valido.clone();
            if !mutante.is_empty() {
                let pos = usize::try_from(rng.next_u64() % mutante.len() as u64).unwrap();
                mutante[pos] ^= rng.byte();
                let _ = modelo.decode(&mutante);
            }

            // Fluxo válido truncado em posição aleatória.
            let corte = usize::try_from(rng.next_u64() % (valido.len() as u64 + 1)).unwrap();
            let _ = modelo.decode(&valido[..corte]);
        }
    }
}

#[test]
fn tabela_serializada_faz_roundtrip_e_preserva_o_id() {
    for w in all_widths() {
        for original in [Table::legacy_only(w, Variant::M1), m2_da_largura(w).clone()] {
            let bytes = original.as_bytes().to_vec();
            let lida = Table::from_bytes(&bytes).unwrap_or_else(|e| panic!("b={w}: {e}"));

            assert_eq!(lida.width(), original.width(), "b={w}");
            assert_eq!(lida.variant(), original.variant(), "b={w}");
            assert_eq!(lida.slots_used(), original.slots_used(), "b={w}");
            assert_eq!(lida.train_sha256(), original.train_sha256(), "b={w}");
            assert_eq!(
                lida.id(),
                original.id(),
                "b={w}: table_id mudou no roundtrip"
            );
            assert_eq!(lida.as_bytes(), original.as_bytes(), "b={w}");
        }
    }
}

#[test]
fn tabela_rejeita_artefato_invalido_com_erro_tipado() {
    let w = Width::new(14).unwrap();
    let bytes = m2_da_largura(w).as_bytes().to_vec();

    let mut magic_ruim = bytes.clone();
    magic_ruim[0] = b'X';
    assert!(matches!(
        Table::from_bytes(&magic_ruim),
        Err(TableError::BadMagic { .. })
    ));

    let mut versao_ruim = bytes.clone();
    versao_ruim[4] = 99;
    assert!(matches!(
        Table::from_bytes(&versao_ruim),
        Err(TableError::BadVersion { got: 99 })
    ));

    let mut largura_ruim = bytes.clone();
    largura_ruim[5] = 33;
    assert!(matches!(
        Table::from_bytes(&largura_ruim),
        Err(TableError::BadWidth { got: 33 })
    ));

    let mut variante_ruim = bytes.clone();
    variante_ruim[6] = 3;
    assert!(matches!(
        Table::from_bytes(&variante_ruim),
        Err(TableError::BadVariant { got: 3 })
    ));

    assert!(matches!(
        Table::from_bytes(&bytes[..20]),
        Err(TableError::Truncated { .. })
    ));

    let mut sobrando = bytes.clone();
    sobrando.push(0);
    assert!(matches!(
        Table::from_bytes(&sobrando),
        Err(TableError::TrailingBytes { extra: 1 })
    ));

    // E lixo arbitrário nunca entra em pânico.
    let mut rng = SplitMix64(SEED ^ 0xBEEF);
    for _ in 0..2000 {
        let n = usize::try_from(rng.next_u64() % 120).unwrap();
        let lixo: Vec<u8> = (0..n).map(|_| rng.byte()).collect();
        let _ = Table::from_bytes(&lixo);
    }
}

#[test]
fn decoder_recusa_flags_ainda_nao_implementadas() {
    // M4 (E7) e sync markers (E8) mudam o layout do payload. Ignorar a flag
    // devolveria bytes verossímeis a partir de um layout que este código não
    // sabe ler.
    let w = Width::new(14).unwrap();
    let modelo = Model::new(m2_da_largura(w).clone());
    let (fluxo, _) = modelo.encode(&corpus_teste()).unwrap();

    for flag in [0b0000_0001u8, 0b0000_0010, 0b1000_0000] {
        let mut com_flag = fluxo.clone();
        com_flag[7] = flag;
        assert!(
            matches!(
                modelo.decode(&com_flag),
                Err(DecodeError::UnsupportedFlags { .. })
            ),
            "flag {flag:#010b} devia ser recusada"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Roundtrip com largura e entrada sorteadas pelo proptest.
    #[test]
    fn roundtrip_proptest(bits in 1u8..=32, entrada in prop::collection::vec(any::<u8>(), 0..2000)) {
        let w = Width::new(bits).unwrap();
        let modelo = Model::new(m2_da_largura(w).clone());
        let (fluxo, stats) = modelo.encode(&entrada).unwrap();
        prop_assert_eq!(stats.input_bytes, entrada.len() as u64);
        prop_assert_eq!(modelo.decode(&fluxo).unwrap(), entrada);
    }

    /// Cabeçalho arbitrário nunca derruba o parser.
    #[test]
    fn cabecalho_arbitrario_nao_panica(bytes in prop::collection::vec(any::<u8>(), 0..80)) {
        let _ = Header::from_bytes(&bytes);
    }

    /// Fluxo arbitrário nunca derruba o decoder.
    #[test]
    fn fluxo_arbitrario_nao_panica(
        bits in 1u8..=32,
        bytes in prop::collection::vec(any::<u8>(), 0..300),
    ) {
        let w = Width::new(bits).unwrap();
        let modelo = Model::new(Table::legacy_only(w, Variant::M1));
        let _ = modelo.decode(&bytes);
    }
}
