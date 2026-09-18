//! Tabela de símbolos: layout de namespace, construção M2 e serialização.
//!
//! A tabela é o que **de fato** produz ganho. A largura do símbolo, sozinha,
//! não cria nada (`docs/00 §5.1`): o que reduz o fluxo é um dicionário que
//! modela a redundância do corpus. Por isso tudo aqui gira em torno de duas
//! coisas: escolher bem as entradas (`docs/01 §4`) e casá-las rápido
//! (`docs/01 §5`).
//!
//! ## Layout de namespace (`docs/01 §1, §3`)
//!
//! | Faixa | Código 0 | Códigos seguintes |
//! |---|---|---|
//! | LOWER `b < 8` | **ESCAPE** — seguido de 8 bits brutos | `1..2^b−1`, entradas escolhidas por ganho |
//! | BASE `b = 8` | byte `0x00` | `1..255`, identidade. Sem entradas dinâmicas |
//! | HIGHER `b > 8` | byte `0x00` | `1..255` legado, depois `256..2^b−1` por ganho |
//!
//! ## Treino nunca é teste
//!
//! A tabela guarda o sha256 do corpus em que foi treinada, e ele vai para o
//! artefato serializado. Não é decoração: o defeito D1 de `docs/05 §3` foi
//! exatamente treinar e testar no mesmo corpus, e um vazamento desses só é
//! detectável depois se o corpus de treino estiver registrado junto do
//! resultado.

use std::collections::HashMap;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::model::{Band, Variant, Width};

/// Maior bloco candidato, em bytes (`docs/01 §4.1`, padrão 16).
pub const MAX_MATCH: usize = 16;

/// Frequência mínima para um n-grama virar candidato.
///
/// Com `custo_slot = 0` numa tabela pré-acordada, até um bloco visto uma
/// única vez tem ganho positivo pela fórmula de `docs/01 §4` — e encheria a
/// tabela de entradas que nunca mais aparecem. Exigir 2 ocorrências é o
/// corte mínimo que também torna a poda estilo Apriori possível.
pub const MIN_FREQ: u32 = 2;

/// Maior "palavra" aceita como candidato, em bytes.
///
/// Em dados binários, "sequência sem espaço nem pontuação" pode ser o
/// arquivo inteiro. O corte evita um candidato de megabytes que nunca se
/// repetiria.
pub const MAX_WORD: usize = 64;

/// Código de ESCAPE no LOWER (`docs/01 §6`, E1).
pub const ESCAPE_CODE: u32 = 0;

/// Magic do artefato de tabela serializada.
pub const TABLE_MAGIC: &[u8; 4] = b"HTB1";

/// Primeiro código disponível para entradas dinâmicas, por faixa.
///
/// - LOWER: 1, porque o 0 é o ESCAPE;
/// - BASE e HIGHER: 256, porque `0..255` são os bytes legados.
///
/// Em `base-8` isso coincide com a capacidade (256), então **não sobra
/// nenhum slot dinâmico** — `base-8` é identidade por construção, como
/// manda `docs/01 §1`.
#[must_use]
pub const fn first_dynamic_code(w: Width) -> u32 {
    match w.band() {
        Band::Lower => 1,
        Band::Base | Band::Higher => 256,
    }
}

/// Quantos slots dinâmicos a largura oferece.
#[must_use]
pub const fn dynamic_slots(w: Width) -> u64 {
    let base = first_dynamic_code(w) as u64;
    w.capacity().saturating_sub(base)
}

/// Erro ao ler uma tabela serializada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    /// Os 4 primeiros bytes não são `HTB1`.
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
    /// O artefato acabou antes do esperado.
    Truncated {
        /// Onde a leitura parou.
        offset: usize,
    },
    /// Sobraram bytes depois da última entrada.
    TrailingBytes {
        /// Quantos bytes sobraram.
        extra: usize,
    },
    /// Mais entradas do que a largura comporta.
    TooManyEntries {
        /// Entradas declaradas.
        declared: u64,
        /// Slots dinâmicos disponíveis.
        available: u64,
    },
    /// Entrada de comprimento zero, que não representa bloco nenhum.
    EmptyEntry {
        /// Índice da entrada.
        index: usize,
    },
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic { got } => write!(f, "magic de tabela inválido: {got:?}, esperado HTB1"),
            Self::BadVersion { got } => write!(f, "versão de tabela desconhecida: {got}"),
            Self::BadWidth { got } => write!(f, "largura de tabela inválida: {got}"),
            Self::BadVariant { got } => write!(f, "variante de tabela inválida: {got}"),
            Self::Truncated { offset } => write!(f, "tabela truncada no offset {offset}"),
            Self::TrailingBytes { extra } => write!(f, "{extra} bytes sobrando na tabela"),
            Self::TooManyEntries {
                declared,
                available,
            } => write!(
                f,
                "tabela declara {declared} entradas mas a largura só tem {available} slots"
            ),
            Self::EmptyEntry { index } => write!(f, "entrada {index} tem comprimento zero"),
        }
    }
}

impl std::error::Error for TableError {}

/// Trie de bytes para casamento guloso (`docs/01 §5`, P-greedy).
///
/// Os filhos ficam num `Vec` ordenado com busca binária, não num array de
/// 256 nem num `HashMap`. O array custaria 1 KB por nó — com dezenas de
/// milhares de nós em `higher-16`, dezenas de MB só de ponteiros nulos, e
/// a tabela sairia do cache (`docs/00 §4`), que é justamente o custo que a
/// tese precisa medir sem inflar de graça.
#[derive(Debug, Default, Clone)]
struct Trie {
    nodes: Vec<TrieNode>,
}

#[derive(Debug, Default, Clone)]
struct TrieNode {
    children: Vec<(u8, u32)>,
    code: Option<u32>,
}

impl Trie {
    fn new() -> Self {
        Self {
            nodes: vec![TrieNode::default()],
        }
    }

    fn insert(&mut self, bloco: &[u8], code: u32) {
        let mut atual = 0usize;
        for &b in bloco {
            let proximo = self.nodes[atual]
                .children
                .iter()
                .find(|(byte, _)| *byte == b)
                .map(|(_, idx)| *idx as usize);
            atual = if let Some(idx) = proximo {
                idx
            } else {
                let idx = self.nodes.len();
                self.nodes.push(TrieNode::default());
                #[allow(clippy::cast_possible_truncation)]
                self.nodes[atual].children.push((b, idx as u32));
                idx
            };
        }
        self.nodes[atual].code = Some(code);
    }

    /// Ordena os filhos de todo nó, para a busca binária valer.
    fn finish(&mut self) {
        for n in &mut self.nodes {
            n.children.sort_unstable_by_key(|(b, _)| *b);
            n.children.shrink_to_fit();
        }
    }

    /// Maior bloco da trie que casa com um prefixo de `input`.
    fn longest_match(&self, input: &[u8]) -> Option<(u32, usize)> {
        let mut atual = 0usize;
        let mut melhor = None;
        for (i, &b) in input.iter().enumerate() {
            let filhos = &self.nodes[atual].children;
            let Ok(pos) = filhos.binary_search_by_key(&b, |(byte, _)| *byte) else {
                break;
            };
            atual = filhos[pos].1 as usize;
            if let Some(code) = self.nodes[atual].code {
                melhor = Some((code, i + 1));
            }
        }
        melhor
    }
}

/// Tabela de símbolos de uma largura e variante.
#[derive(Debug, Clone)]
pub struct Table {
    width: Width,
    variant: Variant,
    /// Blocos dinâmicos, em ordem de código a partir de [`first_dynamic_code`].
    blocks: Vec<Box<[u8]>>,
    trie: Trie,
    train_sha256: [u8; 32],
    /// Serialização canônica, guardada para `id` e `table_bytes` não
    /// poderem divergir do conteúdo.
    serialized: Vec<u8>,
    id: [u8; 32],
}

impl Table {
    /// Tabela só com o legado da faixa, sem nenhuma entrada dinâmica.
    ///
    /// - HIGHER: os 256 códigos legados, roundtrip garantido, ganho nenhum;
    /// - BASE: identidade;
    /// - LOWER: tudo vira ESCAPE. O roundtrip vale, o tamanho é péssimo, e
    ///   isso é exatamente o piso contra o qual o ganho do LOWER se mede.
    ///
    /// É o chão da faixa: serve de controle e para testar o codec sem
    /// depender de corpus de treino.
    #[must_use]
    pub fn legacy_only(width: Width, variant: Variant) -> Self {
        Self::from_blocks(width, variant, Vec::new(), [0u8; 32])
    }

    /// Constrói a tabela M2 a partir de um corpus de **treino**.
    ///
    /// Segue `docs/01 §4`: candidatos por n-grama e por palavra, ranqueados
    /// por `ganho(s) = freq(s) · (8·len(s) − b)`, inseridos até encher os
    /// slots dinâmicos. A re-pontuação iterativa é a E6 — aqui a frequência
    /// é a bruta, que **superestima** o ganho de candidatos que se sobrepõem
    /// (defeito D4 de `docs/05 §3`, corrigido só na E6).
    #[must_use]
    pub fn train_m2(width: Width, train: &[u8]) -> Self {
        let slots = dynamic_slots(width);
        let mut escolhidos: Vec<Box<[u8]>> = Vec::new();

        if slots > 0 && !train.is_empty() {
            let mut candidatos = candidatos_com_ganho(train, width);
            // Ordem determinística: sem ela, dois treinos do mesmo corpus
            // gerariam `table_id` diferentes e nenhum run seria reproduzível.
            candidatos.sort_by(|a, b| {
                b.ganho
                    .cmp(&a.ganho)
                    .then_with(|| b.bloco.len().cmp(&a.bloco.len()))
                    .then_with(|| a.bloco.cmp(b.bloco))
            });
            let limite = usize::try_from(slots).unwrap_or(usize::MAX);
            escolhidos = candidatos
                .into_iter()
                .take(limite)
                .map(|c| c.bloco.to_vec().into_boxed_slice())
                .collect();
        }

        let mut hasher = Sha256::new();
        hasher.update(train);
        let train_sha256: [u8; 32] = hasher.finalize().into();

        Self::from_blocks(width, Variant::M2, escolhidos, train_sha256)
    }

    fn from_blocks(
        width: Width,
        variant: Variant,
        blocks: Vec<Box<[u8]>>,
        train_sha256: [u8; 32],
    ) -> Self {
        let base = first_dynamic_code(width);
        let mut trie = Trie::new();
        for (i, bloco) in blocks.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            trie.insert(bloco, base + i as u32);
        }
        trie.finish();

        let serialized = serializa(width, variant, &blocks, &train_sha256);
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let id: [u8; 32] = hasher.finalize().into();

        Self {
            width,
            variant,
            blocks,
            trie,
            train_sha256,
            serialized,
            id,
        }
    }

    /// Lê uma tabela serializada.
    ///
    /// # Errors
    ///
    /// [`TableError`] para magic, versão, largura, variante, comprimento ou
    /// contagem de entradas inválidos. Nunca entra em pânico.
    pub fn from_bytes(src: &[u8]) -> Result<Self, TableError> {
        let mut p = 0usize;
        let cab = leia(src, &mut p, 44)?;
        if &cab[0..4] != TABLE_MAGIC {
            let mut got = [0u8; 4];
            got.copy_from_slice(&cab[0..4]);
            return Err(TableError::BadMagic { got });
        }
        if cab[4] != 1 {
            return Err(TableError::BadVersion { got: cab[4] });
        }
        let width = Width::new(cab[5]).map_err(|_| TableError::BadWidth { got: cab[5] })?;
        let variant = Variant::from_wire(cab[6]).ok_or(TableError::BadVariant { got: cab[6] })?;
        // cab[7] é reservado e ignorado.
        let n_entries = u32::from_be_bytes([cab[8], cab[9], cab[10], cab[11]]);
        let mut train_sha256 = [0u8; 32];
        train_sha256.copy_from_slice(&cab[12..44]);

        let slots = dynamic_slots(width);
        if u64::from(n_entries) > slots {
            return Err(TableError::TooManyEntries {
                declared: u64::from(n_entries),
                available: slots,
            });
        }

        let mut blocks = Vec::with_capacity(n_entries as usize);
        for index in 0..n_entries as usize {
            let len = leia(src, &mut p, 1)?[0] as usize;
            if len == 0 {
                return Err(TableError::EmptyEntry { index });
            }
            blocks.push(leia(src, &mut p, len)?.to_vec().into_boxed_slice());
        }
        if p != src.len() {
            return Err(TableError::TrailingBytes {
                extra: src.len() - p,
            });
        }

        Ok(Self::from_blocks(width, variant, blocks, train_sha256))
    }

    /// Largura da tabela.
    #[must_use]
    pub const fn width(&self) -> Width {
        self.width
    }

    /// Variante da tabela.
    #[must_use]
    pub const fn variant(&self) -> Variant {
        self.variant
    }

    /// sha256 da serialização canônica — o `table_id` de `docs/01 §8`.
    #[must_use]
    pub const fn id(&self) -> &[u8; 32] {
        &self.id
    }

    /// Os 8 primeiros bytes do `table_id`, que vão no cabeçalho `.hgr`.
    #[must_use]
    pub fn id8(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out.copy_from_slice(&self.id[..8]);
        out
    }

    /// sha256 do corpus de treino (zeros quando não houve treino).
    #[must_use]
    pub const fn train_sha256(&self) -> &[u8; 32] {
        &self.train_sha256
    }

    /// Serialização canônica.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.serialized
    }

    /// `table_bytes` de `docs/02 §3`: tamanho serializado da tabela.
    #[must_use]
    pub fn table_bytes(&self) -> usize {
        self.serialized.len()
    }

    /// `slots_used` de `docs/02 §3`: entradas dinâmicas na tabela.
    #[must_use]
    pub fn slots_used(&self) -> usize {
        self.blocks.len()
    }

    /// Fração dos slots dinâmicos efetivamente ocupada.
    ///
    /// Em `b ≥ 20` isso tende a zero mesmo com corpus grande — é a métrica
    /// que torna visível o "capacidade ≠ preenchimento" de `docs/00 §5.5`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn occupancy(&self) -> f64 {
        let slots = dynamic_slots(self.width);
        if slots == 0 {
            0.0
        } else {
            self.blocks.len() as f64 / slots as f64
        }
    }

    /// Bloco de bytes que um código representa.
    ///
    /// Devolve `None` para o ESCAPE do LOWER (que não representa bloco) e
    /// para código fora da tabela.
    #[must_use]
    pub fn block_of(&self, code: u32) -> Option<&[u8]> {
        if u64::from(code) >= self.width.capacity() {
            return None;
        }
        let base = first_dynamic_code(self.width);
        match self.width.band() {
            Band::Lower => {
                if code == ESCAPE_CODE {
                    None
                } else {
                    self.blocks.get((code - base) as usize).map(AsRef::as_ref)
                }
            }
            Band::Base => None,
            Band::Higher => {
                if code < 256 {
                    None
                } else {
                    self.blocks.get((code - base) as usize).map(AsRef::as_ref)
                }
            }
        }
    }

    /// Maior entrada da tabela que casa com um prefixo de `input`.
    #[must_use]
    pub fn longest_match(&self, input: &[u8]) -> Option<(u32, usize)> {
        self.trie.longest_match(input)
    }
}

/// Serialização canônica de uma tabela.
///
/// ```text
/// 0   magic        4 bytes   "HTB1"
/// 4   version      u8        = 1
/// 5   b            u8        1..32
/// 6   variant      u8        0=M0 1=M1 2=M2 4=M4
/// 7   reservado    u8        = 0
/// 8   n_entries    u32 BE
/// 12  train_sha256 [u8; 32]
/// 44  entradas: para cada uma, len u8 (1..=255) e os bytes
/// ```
///
/// É canônica porque o `table_id` é o sha256 dela: qualquer folga de
/// codificação viraria dois ids para a mesma tabela.
fn serializa(
    width: Width,
    variant: Variant,
    blocks: &[Box<[u8]>],
    train_sha256: &[u8; 32],
) -> Vec<u8> {
    let corpo: usize = blocks.iter().map(|b| 1 + b.len()).sum();
    let mut out = Vec::with_capacity(44 + corpo);
    out.extend_from_slice(TABLE_MAGIC);
    out.push(1);
    out.push(width.bits());
    out.push(variant.to_wire());
    out.push(0);
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(blocks.len() as u32).to_be_bytes());
    out.extend_from_slice(train_sha256);
    for bloco in blocks {
        debug_assert!((1..=255).contains(&bloco.len()));
        #[allow(clippy::cast_possible_truncation)]
        out.push(bloco.len() as u8);
        out.extend_from_slice(bloco);
    }
    out
}

fn leia<'a>(src: &'a [u8], p: &mut usize, n: usize) -> Result<&'a [u8], TableError> {
    let fim = p
        .checked_add(n)
        .ok_or(TableError::Truncated { offset: *p })?;
    if fim > src.len() {
        return Err(TableError::Truncated { offset: *p });
    }
    let fatia = &src[*p..fim];
    *p = fim;
    Ok(fatia)
}

struct Candidato<'a> {
    bloco: &'a [u8],
    ganho: i64,
}

/// Candidatos com ganho positivo, por n-grama e por palavra (`docs/01 §4.1`).
fn candidatos_com_ganho(train: &[u8], width: Width) -> Vec<Candidato<'_>> {
    let b = i64::from(width.bits());
    let ganho = |len: usize, freq: u32| -> i64 {
        // ganho(s) = freq(s) · (8·len(s) − b), com custo_slot = 0 numa
        // tabela pré-acordada (docs/01 §4).
        // `len <= MAX_WORD`, então a conversão é exata.
        let len = i64::try_from(len).unwrap_or(i64::MAX);
        i64::from(freq) * (8 * len - b)
    };

    let mut freqs = ngramas_frequentes(train);
    for (bloco, freq) in palavras_frequentes(train) {
        // Uma palavra já contada como n-grama não entra duas vezes.
        freqs.entry(bloco).or_insert(freq);
    }

    freqs
        .into_iter()
        .filter_map(|(bloco, freq)| {
            let g = ganho(bloco.len(), freq);
            (g > 0).then_some(Candidato { bloco, ganho: g })
        })
        .collect()
}

/// n-gramas de 1 a [`MAX_MATCH`] bytes com frequência ≥ [`MIN_FREQ`].
///
/// A contagem é feita por comprimento crescente, e um n-grama de tamanho
/// `L` só é contado se o seu prefixo de tamanho `L−1` já é frequente
/// (poda estilo Apriori). Sem isso, contar 1..16 num corpus de 10 MB seriam
/// 160 milhões de chaves — a construção da tabela estouraria a memória
/// antes da E4 chegar a medir qualquer coisa.
fn ngramas_frequentes(train: &[u8]) -> HashMap<&[u8], u32> {
    let mut resultado: HashMap<&[u8], u32> = HashMap::new();
    let mut anterior: HashMap<&[u8], u32> = HashMap::new();

    for len in 1..=MAX_MATCH {
        if train.len() < len {
            break;
        }
        let mut atual: HashMap<&[u8], u32> = HashMap::new();
        for i in 0..=train.len() - len {
            if len > 1 && !anterior.contains_key(&train[i..i + len - 1]) {
                continue;
            }
            *atual.entry(&train[i..i + len]).or_insert(0) += 1;
        }
        atual.retain(|_, freq| *freq >= MIN_FREQ);
        if atual.is_empty() {
            break;
        }
        resultado.extend(atual.iter().map(|(k, v)| (*k, *v)));
        anterior = atual;
    }
    resultado
}

/// Palavras (sequências sem espaço nem pontuação ASCII) com frequência ≥ [`MIN_FREQ`].
fn palavras_frequentes(train: &[u8]) -> HashMap<&[u8], u32> {
    let mut freqs: HashMap<&[u8], u32> = HashMap::new();
    let mut inicio = None;
    for i in 0..=train.len() {
        let separa = i == train.len() || e_separador(train[i]);
        match (separa, inicio) {
            (true, Some(ini)) => {
                let fim = i.min(ini + MAX_WORD);
                if fim > ini {
                    *freqs.entry(&train[ini..fim]).or_insert(0) += 1;
                }
                inicio = None;
            }
            (false, None) => inicio = Some(i),
            _ => {}
        }
    }
    freqs.retain(|bloco, freq| *freq >= MIN_FREQ && bloco.len() > 1);
    freqs
}

const fn e_separador(b: u8) -> bool {
    b.is_ascii_whitespace() || b.is_ascii_punctuation()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::all_widths;

    fn w(bits: u8) -> Width {
        Width::new(bits).unwrap()
    }

    const TREINO: &[u8] = b"ao ao ao banana banana banana ao banana ao banana ao banana \
                            ao ao banana ao banana banana ao ao banana ao ao banana ";

    #[test]
    fn o_primeiro_codigo_dinamico_segue_a_faixa() {
        for width in all_widths() {
            let esperado = match width.band() {
                Band::Lower => 1,
                Band::Base | Band::Higher => 256,
            };
            assert_eq!(first_dynamic_code(width), esperado, "b = {width}");
        }
    }

    #[test]
    fn base_8_nao_tem_slot_dinamico_nenhum() {
        // docs/01 §1: BASE é identidade, 256 bytes, sem escape e sem packing.
        // A capacidade (256) é exatamente o legado, então não sobra slot.
        assert_eq!(dynamic_slots(w(8)), 0);
        let t = Table::train_m2(w(8), TREINO);
        assert_eq!(t.slots_used(), 0, "base-8 treinada continua identidade");
        assert!(t.occupancy().abs() < 1e-12);
    }

    #[test]
    fn slots_dinamicos_por_faixa() {
        assert_eq!(dynamic_slots(w(1)), 1); // só o código 1; o 0 é ESCAPE
        assert_eq!(dynamic_slots(w(7)), 127);
        assert_eq!(dynamic_slots(w(9)), 512 - 256);
        assert_eq!(dynamic_slots(w(14)), 16_384 - 256);
        assert_eq!(dynamic_slots(w(32)), 4_294_967_296 - 256);
    }

    #[test]
    fn higher_nao_recebe_bloco_de_um_byte() {
        // Em b > 8 um bloco de 1 byte custaria b bits para o que o código
        // legado já resolve: ganho = freq·(8 − b) < 0. A função de ganho de
        // docs/01 §4 exclui sozinha, e isso precisa continuar valendo.
        for bits in [9u8, 12, 14, 16, 20, 24, 32] {
            let t = Table::train_m2(w(bits), TREINO);
            let fim = 256 + u32::try_from(t.slots_used()).unwrap();
            for code in 256..fim {
                let bloco = t.block_of(code).expect("código dentro da tabela");
                assert!(bloco.len() >= 2, "b = {bits}: bloco de 1 byte na tabela");
            }
        }
    }

    /// DNA pseudoaleatório determinístico, para o cenário de H5.
    ///
    /// Aleatório de propósito: um corpus periódico (`ACGTACGT…`) faria a
    /// tabela aprender um único bloco longo e nada mais, o que é um artefato
    /// do gerador, não o comportamento em alfabeto restrito de verdade.
    fn dna(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = x;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                b"ACGT"[((z ^ (z >> 31)) % 4) as usize]
            })
            .collect()
    }

    #[test]
    fn lower_cobre_alfabeto_restrito_sem_escapar() {
        // No LOWER, ganho = freq·(8 − b) > 0 para todo b < 8, então bytes
        // isolados são candidatos legítimos. Com o alfabeto cabendo nos
        // slots, a tabela o cobre inteiro e o escape some — que é o cenário
        // de H5 (`docs/00 §6`: LOWER vence em alfabeto restrito).
        //
        // O contrário também vale e **não** é defeito: em corpus de alfabeto
        // grande, poucos slots são melhor gastos em blocos longos e a taxa de
        // escape fica altíssima. Medir isso em corpus real é a E4.
        let treino = dna(20_000, 1);
        let teste = dna(2_000, 999); // seed diferente: teste nunca é treino
        assert_ne!(treino, teste);

        let modelo = crate::codec::Model::new(Table::train_m2(w(3), &treino));
        let (fluxo, stats) = modelo.encode(&teste).expect("encode");
        assert_eq!(
            stats.n_escapes, 0,
            "lower-3 devia cobrir um alfabeto de 4 letras sem escapar"
        );
        assert!(
            stats.bits_per_byte() < 8.0,
            "lower-3 em DNA devia ficar abaixo de base-8, deu {}",
            stats.bits_per_byte()
        );
        assert_eq!(modelo.decode(&fluxo).unwrap(), teste);
    }

    #[test]
    fn o_treino_e_deterministico() {
        // Dois treinos do mesmo corpus têm de dar o mesmo `table_id`, senão
        // nenhum run é reproduzível (docs/02 §6.5).
        for width in all_widths() {
            let a = Table::train_m2(width, TREINO);
            let b = Table::train_m2(width, TREINO);
            assert_eq!(a.id(), b.id(), "b = {width}");
            assert_eq!(a.as_bytes(), b.as_bytes(), "b = {width}");
        }
    }

    #[test]
    fn corpus_de_treino_diferente_da_table_id_diferente() {
        let a = Table::train_m2(w(14), TREINO);
        let b = Table::train_m2(w(14), b"outro corpus outro corpus outro corpus ");
        assert_ne!(a.id(), b.id());
        assert_ne!(a.train_sha256(), b.train_sha256());
    }

    #[test]
    fn o_sha256_do_treino_entra_no_artefato() {
        // É o que permite detectar vazamento treino/teste depois do fato
        // (defeito D1 de docs/05 §3).
        let t = Table::train_m2(w(14), TREINO);
        assert_eq!(&t.as_bytes()[12..44], t.train_sha256());
        assert_ne!(t.train_sha256(), &[0u8; 32], "treino não vazio tem hash");

        let vazia = Table::legacy_only(w(14), Variant::M1);
        assert_eq!(vazia.train_sha256(), &[0u8; 32], "sem treino, sem hash");
    }

    #[test]
    fn a_table_id_e_o_sha256_da_serializacao() {
        let t = Table::train_m2(w(14), TREINO);
        let mut h = Sha256::new();
        h.update(t.as_bytes());
        let esperado: [u8; 32] = h.finalize().into();
        assert_eq!(t.id(), &esperado);
        assert_eq!(t.id8(), esperado[..8]);
    }

    #[test]
    fn a_trie_casa_o_maior_bloco() {
        let mut trie = Trie::new();
        trie.insert(b"ba", 10);
        trie.insert(b"banana", 11);
        trie.insert(b"ban", 12);
        trie.finish();

        assert_eq!(trie.longest_match(b"banana split"), Some((11, 6)));
        assert_eq!(trie.longest_match(b"bandeira"), Some((12, 3)));
        assert_eq!(trie.longest_match(b"barco"), Some((10, 2)));
        assert_eq!(trie.longest_match(b"xyz"), None);
        assert_eq!(trie.longest_match(b""), None);
        assert_eq!(
            trie.longest_match(b"b"),
            None,
            "prefixo sem código não casa"
        );
    }

    #[test]
    fn a_poda_apriori_nao_perde_n_grama_frequente() {
        // A poda só corta n-gramas cujo prefixo já é raro. Um n-grama
        // frequente tem prefixo frequente, então nenhum se perde — e é
        // isso que o teste fixa, porque a poda é uma otimização de memória
        // que não pode mudar a tabela.
        let treino = b"abcabcabcabcabc";
        let freqs = ngramas_frequentes(treino);
        assert_eq!(freqs.get(b"abc".as_slice()), Some(&5));
        assert_eq!(freqs.get(b"abcabc".as_slice()), Some(&4));
        assert_eq!(freqs.get(b"a".as_slice()), Some(&5));
        // MAX_MATCH corta em 16 bytes.
        assert!(freqs.keys().all(|k| k.len() <= MAX_MATCH));
        // E nada com frequência abaixo do mínimo sobrevive.
        assert!(freqs.values().all(|f| *f >= MIN_FREQ));
    }

    #[test]
    fn palavras_entram_como_candidatas() {
        let freqs = palavras_frequentes(b"banana, banana; banana. ao ao");
        assert_eq!(freqs.get(b"banana".as_slice()), Some(&3));
        assert_eq!(freqs.get(b"ao".as_slice()), Some(&2));
    }

    #[test]
    fn palavra_gigante_e_cortada_em_max_word() {
        let treino: Vec<u8> = std::iter::repeat_n(b'x', 500)
            .chain(std::iter::once(b' '))
            .chain(std::iter::repeat_n(b'x', 500))
            .collect();
        let freqs = palavras_frequentes(&treino);
        assert!(
            freqs.keys().all(|k| k.len() <= MAX_WORD),
            "nenhuma palavra pode passar de MAX_WORD"
        );
    }

    #[test]
    fn a_tabela_nunca_passa_dos_slots_da_largura() {
        for width in all_widths() {
            let t = Table::train_m2(width, TREINO);
            assert!(
                t.slots_used() as u64 <= dynamic_slots(width),
                "b = {width}: {} entradas para {} slots",
                t.slots_used(),
                dynamic_slots(width)
            );
        }
    }

    #[test]
    fn a_ocupacao_despenca_nas_larguras_grandes() {
        // "Capacidade ≠ preenchimento" (docs/00 §5.5): com corpus fixo, a
        // fração de slots úteis cai conforme 2^b cresce. Aqui isso é só
        // aritmética do corpus de teste, não medição de corpus real.
        let pequena = Table::train_m2(w(9), TREINO).occupancy();
        let grande = Table::train_m2(w(24), TREINO).occupancy();
        assert!(grande < pequena, "{grande} devia ser menor que {pequena}");
        assert!(grande < 0.001);
    }
}
