//! Packing de bits MSB-first para qualquer largura de 1 a 32 (`docs/01 §7`).
//!
//! Um único [`BitWriter`] e um único [`BitReader`] servem as 32 larguras: não
//! há código por largura. O que muda é a **estratégia** de despacho
//! ([`Strategy`]), escolhida a partir de `b`:
//!
//! | Estratégia | Quando | Como |
//! |---|---|---|
//! | [`Strategy::Native`] | `b ∈ {8, 16, 32}` | cópia direta de 1, 2 ou 4 bytes, sem shift |
//! | [`Strategy::Group`] | `lcm(b,8)/b · b ≤ 64` | grupo inteiro num `u64`, escrito de uma vez |
//! | [`Strategy::Generic`] | o resto | acumulador `u64`, byte a byte |
//!
//! As três produzem **exatamente os mesmos bytes** — isso é invariante
//! testada, não suposição. A diferença entre elas é só de velocidade, e
//! quanto ela vale é resultado a medir (`docs/01 §7`), não a presumir.
//!
//! Convenções que valem em toda a faixa:
//!
//! - **MSB-first:** o primeiro símbolo ocupa os bits mais significativos do
//!   primeiro byte (`CLAUDE.md`, seção "Convenções").
//! - **Último byte completado com zeros.** O número de símbolos não está no
//!   fluxo: quem decodifica precisa saber `n_symbols`, que no formato `.hgr`
//!   vem do cabeçalho (`docs/01 §8`).

use crate::model::Width;
use core::fmt;

/// Erro ao empacotar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackError {
    /// Um valor não cabe em `bits` bits.
    ///
    /// Mascarar silenciosamente corromperia o fluxo sem que nada acusasse:
    /// o roundtrip falharia muito depois, já dentro de um run de benchmark.
    ValueTooWide {
        /// Posição do valor na sequência.
        index: usize,
        /// O valor recusado.
        value: u32,
        /// A largura em que ele não cabe.
        bits: u8,
    },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueTooWide { index, value, bits } => write!(
                f,
                "valor {value} na posição {index} não cabe em {bits} bits (máximo {})",
                max_value(*bits)
            ),
        }
    }
}

impl std::error::Error for PackError {}

/// Erro ao desempacotar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnpackError {
    /// O buffer é menor do que `n_symbols` símbolos de `b` bits exigem.
    Truncated {
        /// Bytes necessários para `n_symbols` símbolos.
        expected_bytes: usize,
        /// Bytes recebidos.
        got_bytes: usize,
    },
    /// Sobraram bytes além do que `n_symbols` símbolos ocupam.
    TrailingBytes {
        /// Bytes necessários para `n_symbols` símbolos.
        expected_bytes: usize,
        /// Bytes recebidos.
        got_bytes: usize,
    },
    /// Os bits de completamento do último byte não são zero.
    ///
    /// `docs/01 §7` manda completar com zeros. Qualquer outra coisa indica
    /// truncamento, corrupção ou um `n_symbols` errado — e é melhor falhar
    /// aqui do que devolver símbolos plausíveis e errados.
    PaddingNotZero {
        /// Quantos bits de completamento o último byte tem.
        padding_bits: u32,
        /// O último byte, como veio.
        last_byte: u8,
    },
}

impl fmt::Display for UnpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated {
                expected_bytes,
                got_bytes,
            } => write!(
                f,
                "fluxo truncado: {expected_bytes} bytes esperados, {got_bytes} recebidos"
            ),
            Self::TrailingBytes {
                expected_bytes,
                got_bytes,
            } => write!(
                f,
                "bytes sobrando: {expected_bytes} bytes esperados, {got_bytes} recebidos"
            ),
            Self::PaddingNotZero {
                padding_bits,
                last_byte,
            } => write!(
                f,
                "completamento de {padding_bits} bits não é zero no último byte {last_byte:#04x}"
            ),
        }
    }
}

impl std::error::Error for UnpackError {}

/// Maior valor representável em `bits` bits.
#[must_use]
pub const fn max_value(bits: u8) -> u32 {
    if bits >= 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}

/// Bytes ocupados por `n_symbols` símbolos de largura `w`, com o completamento.
#[must_use]
pub const fn packed_len(n_symbols: usize, w: Width) -> usize {
    let bits = n_symbols * w.bits() as usize;
    bits.div_ceil(8)
}

/// Estratégia de despacho escolhida para uma largura (`docs/01 §7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// `b ∈ {8, 16, 32}`: um símbolo é 1, 2 ou 4 bytes. Cópia direta.
    Native {
        /// Bytes por símbolo: 1, 2 ou 4.
        bytes: u8,
    },
    /// Grupo de `lcm(b,8)/b` símbolos que cabe num `u64` e fecha em bytes.
    Group {
        /// Símbolos por grupo.
        symbols: u8,
        /// Bytes por grupo.
        bytes: u8,
    },
    /// Acumulador genérico de 64 bits, byte a byte.
    ///
    /// É a estratégia das larguras cujo grupo passa de 64 bits (`higher-9`
    /// precisaria de 8 símbolos × 9 bits = 72).
    Generic,
}

impl Strategy {
    /// Estratégia da largura `w`.
    #[must_use]
    pub const fn of(w: Width) -> Self {
        if w.is_native() {
            return Self::Native {
                bytes: w.bits() / 8,
            };
        }
        let symbols = w.pack_group_symbols();
        if (symbols as u32) * (w.bits() as u32) <= 64 {
            Self::Group {
                symbols,
                bytes: w.pack_group_bytes(),
            }
        } else {
            Self::Generic
        }
    }

    /// Nome curto para relatório e benchmark.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native { .. } => "native",
            Self::Group { .. } => "group",
            Self::Generic => "generic",
        }
    }
}

/// Tabela de despacho dos grupos: `(b, símbolos, bytes)`.
///
/// São as 13 larguras não nativas cujo grupo `lcm(b,8)/b` cabe num `u64`
/// (ver [`Strategy::of`]). Estar aqui como literais não é repetição do que
/// `Width` já calcula: é o que permite passar `b`, o tamanho do grupo e o
/// número de bytes como **constantes de tipo**, e com isso o compilador
/// desenrola o laço interno e especializa a cópia. Com os mesmos valores
/// vindo de variáveis, o caminho "rápido" chegava a perder do genérico.
///
/// A coerência entre esta tabela e [`Width`] é verificada em teste.
macro_rules! para_cada_grupo {
    ($mac:ident) => {
        $mac! {
            (1, 8, 1),
            (2, 4, 1),
            (3, 8, 3),
            (4, 2, 1),
            (5, 8, 5),
            (6, 4, 3),
            (7, 8, 7),
            (10, 4, 5),
            (12, 2, 3),
            (14, 4, 7),
            (20, 2, 5),
            (24, 1, 3),
            (28, 2, 7),
        }
    };
}

/// Escreve `cheios` em grupos de `NS` símbolos de `BITS` bits, `NB` bytes cada.
fn escreve_grupos<const BITS: u32, const NS: usize, const NB: usize>(
    out: &mut Vec<u8>,
    cheios: &[u32],
) {
    for chunk in cheios.chunks_exact(NS) {
        let mut word = 0u64;
        for &v in chunk {
            word = (word << BITS) | u64::from(v);
        }
        let be = word.to_be_bytes();
        let mut arr = [0u8; NB];
        arr.copy_from_slice(&be[size_of::<u64>() - NB..]);
        out.extend_from_slice(&arr);
    }
}

/// Lê `grupos` grupos de `NS` símbolos de `BITS` bits a partir de `src`.
fn le_grupos<const BITS: u32, const NS: usize, const NB: usize>(
    out: &mut Vec<u32>,
    src: &[u8],
    grupos: usize,
) {
    let mascara = (1u64 << BITS) - 1;
    for chunk in src[..grupos * NB].chunks_exact(NB) {
        let mut buf = [0u8; 8];
        buf[size_of::<u64>() - NB..].copy_from_slice(chunk);
        let word = u64::from_be_bytes(buf);
        for i in 0..NS {
            // `NS <= 8`, então o cast é exato.
            #[allow(clippy::cast_possible_truncation)]
            let shift = BITS * (NS - 1 - i) as u32;
            #[allow(clippy::cast_possible_truncation)]
            out.push(((word >> shift) & mascara) as u32);
        }
    }
}

/// Escritor de bits MSB-first com acumulador de 64 bits.
///
/// Os bits pendentes ficam nos `pending` bits **baixos** de `acc`, com o
/// primeiro bit escrito na posição mais alta desses `pending`. `pending`
/// nunca chega a 8 entre chamadas, então `acc` comporta um símbolo de 32
/// bits com folga.
#[derive(Debug, Default)]
pub struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    pending: u32,
}

impl BitWriter {
    /// Escritor vazio.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            out: Vec::new(),
            acc: 0,
            pending: 0,
        }
    }

    /// Escritor com espaço reservado para `n_symbols` símbolos de largura `w`.
    #[must_use]
    pub fn with_capacity_for(n_symbols: usize, w: Width) -> Self {
        Self {
            out: Vec::with_capacity(packed_len(n_symbols, w)),
            acc: 0,
            pending: 0,
        }
    }

    /// `true` quando não há bits pendentes e a escrita está alinhada ao byte.
    #[must_use]
    pub const fn is_byte_aligned(&self) -> bool {
        self.pending == 0
    }

    /// Bits já escritos, contando os pendentes.
    #[must_use]
    pub const fn bits_written(&self) -> u64 {
        self.out.len() as u64 * 8 + self.pending as u64
    }

    /// Escreve um símbolo de largura `w`.
    ///
    /// # Errors
    ///
    /// [`PackError::ValueTooWide`] se `value` não couber em `w` bits.
    pub fn push(&mut self, value: u32, w: Width) -> Result<(), PackError> {
        let bits = w.bits();
        if bits < 32 && value > max_value(bits) {
            return Err(PackError::ValueTooWide {
                index: 0,
                value,
                bits,
            });
        }
        self.push_unchecked(value, u32::from(bits));
        Ok(())
    }

    /// Escreve `bits` bits de `value`, assumindo que ele cabe.
    ///
    /// Sem `unsafe`: se `value` for largo demais, os bits altos entram no
    /// fluxo e o roundtrip acusa. Por isso todo caminho público valida antes.
    fn push_unchecked(&mut self, value: u32, bits: u32) {
        self.acc = (self.acc << bits) | u64::from(value);
        self.pending += bits;
        while self.pending >= 8 {
            self.pending -= 8;
            // Trunca de propósito: queremos exatamente os 8 bits do topo.
            #[allow(clippy::cast_possible_truncation)]
            self.out.push((self.acc >> self.pending) as u8);
        }
        // Mantém só os bits pendentes, para o shift do próximo push não
        // arrastar lixo dos símbolos já emitidos.
        self.acc &= (1u64 << self.pending) - 1;
    }

    /// Escreve uma sequência inteira, despachando pela estratégia de `w`.
    ///
    /// Valida a sequência toda antes de escrever um bit sequer: assim o
    /// laço quente não repete a comparação, e um erro no meio não deixa o
    /// escritor com meio fluxo gravado.
    ///
    /// # Errors
    ///
    /// [`PackError::ValueTooWide`] com o índice do primeiro valor largo demais.
    pub fn push_all(&mut self, values: &[u32], w: Width) -> Result<(), PackError> {
        let bits = w.bits();
        if bits < 32 {
            let limite = max_value(bits);
            if let Some((index, &value)) = values.iter().enumerate().find(|(_, &v)| v > limite) {
                return Err(PackError::ValueTooWide { index, value, bits });
            }
        }

        // As estratégias rápidas só valem com a escrita alinhada ao byte.
        // Desalinhado, o genérico é o único correto.
        if !self.is_byte_aligned() {
            for &v in values {
                self.push_unchecked(v, u32::from(bits));
            }
            return Ok(());
        }

        match Strategy::of(w) {
            Strategy::Native { bytes } => self.push_all_native(values, bytes),
            Strategy::Group { symbols, .. } => {
                self.push_all_group(values, w, symbols as usize);
            }
            Strategy::Generic => {
                for &v in values {
                    self.push_unchecked(v, u32::from(bits));
                }
            }
        }
        Ok(())
    }

    /// `b ∈ {8, 16, 32}`: cópia direta, sem shift nem acumulador.
    ///
    /// O `match` não é decoração: com o número de bytes vindo de uma
    /// variável, cada símbolo vira um `extend_from_slice` de fatia de
    /// tamanho desconhecido, e o compilador não consegue vetorizar. Com os
    /// três casos constantes ele consegue. A diferença apareceu no bench.
    fn push_all_native(&mut self, values: &[u32], bytes: u8) {
        self.out.reserve(values.len() * bytes as usize);
        match bytes {
            1 => {
                #[allow(clippy::cast_possible_truncation)]
                self.out.extend(values.iter().map(|&v| v as u8));
            }
            2 => {
                for &v in values {
                    #[allow(clippy::cast_possible_truncation)]
                    self.out.extend_from_slice(&(v as u16).to_be_bytes());
                }
            }
            _ => {
                for &v in values {
                    self.out.extend_from_slice(&v.to_be_bytes());
                }
            }
        }
    }

    /// Grupo de `symbols` símbolos montado num `u64` e emitido de uma vez.
    fn push_all_group(&mut self, values: &[u32], w: Width, symbols: usize) {
        let bits = u32::from(w.bits());
        self.out.reserve(packed_len(values.len(), w));
        let (cheios, resto) = values.split_at(values.len() - values.len() % symbols);

        macro_rules! despacha {
            ($(($b:literal, $ns:literal, $nb:literal),)+) => {
                match w.bits() {
                    $($b => escreve_grupos::<$b, $ns, $nb>(&mut self.out, cheios),)+
                    // Largura sem grupo não chega aqui: `push_all` só
                    // despacha para cá em `Strategy::Group`.
                    _ => {
                        for &v in cheios {
                            self.push_unchecked(v, bits);
                        }
                    }
                }
            };
        }
        para_cada_grupo!(despacha);

        // O resto do grupo não fecha em bytes: vai pelo genérico e fica
        // pendente no acumulador até o próximo push ou o finish.
        for &v in resto {
            self.push_unchecked(v, bits);
        }
    }

    /// Fecha o fluxo completando o último byte com zeros e devolve os bytes.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        if self.pending > 0 {
            let padding = 8 - self.pending;
            // Trunca de propósito: sobra exatamente 1 byte.
            #[allow(clippy::cast_possible_truncation)]
            self.out.push((self.acc << padding) as u8);
            self.pending = 0;
            self.acc = 0;
        }
        self.out
    }
}

/// Leitor de bits MSB-first, espelho do [`BitWriter`].
#[derive(Debug)]
pub struct BitReader<'a> {
    src: &'a [u8],
    /// Posição de leitura, em bits desde o início de `src`.
    bit_pos: u64,
}

impl<'a> BitReader<'a> {
    /// Leitor sobre `src`.
    #[must_use]
    pub const fn new(src: &'a [u8]) -> Self {
        Self { src, bit_pos: 0 }
    }

    /// Bits ainda não lidos.
    #[must_use]
    pub const fn bits_remaining(&self) -> u64 {
        self.src.len() as u64 * 8 - self.bit_pos
    }

    /// `true` quando a leitura está alinhada ao byte.
    #[must_use]
    pub const fn is_byte_aligned(&self) -> bool {
        self.bit_pos.is_multiple_of(8)
    }

    /// Lê um símbolo de largura `w`, ou `None` se não houver bits suficientes.
    pub fn read(&mut self, w: Width) -> Option<u32> {
        let bits = u32::from(w.bits());
        if self.bits_remaining() < u64::from(bits) {
            return None;
        }
        Some(self.read_unchecked(bits))
    }

    /// Lê `bits` bits, assumindo que existem.
    fn read_unchecked(&mut self, bits: u32) -> u32 {
        let mut restante = bits;
        let mut valor = 0u64;
        while restante > 0 {
            #[allow(clippy::cast_possible_truncation)]
            let byte_idx = (self.bit_pos / 8) as usize;
            #[allow(clippy::cast_possible_truncation)]
            let offset = (self.bit_pos % 8) as u32;
            let disponiveis = 8 - offset;
            let tomar = restante.min(disponiveis);
            let byte = u64::from(self.src[byte_idx]);
            // Descarta os bits já lidos deste byte, depois os que ficam para
            // a próxima iteração.
            let pedaco = (byte >> (disponiveis - tomar)) & ((1u64 << tomar) - 1);
            valor = (valor << tomar) | pedaco;
            self.bit_pos += u64::from(tomar);
            restante -= tomar;
        }
        // `bits <= 32` por construção de `Width`.
        #[allow(clippy::cast_possible_truncation)]
        {
            valor as u32
        }
    }

    /// Lê `n_symbols` símbolos para `out`, despachando pela estratégia de `w`.
    ///
    /// # Errors
    ///
    /// [`UnpackError::Truncated`] se faltarem bits.
    pub fn read_all_into(
        &mut self,
        out: &mut Vec<u32>,
        n_symbols: usize,
        w: Width,
    ) -> Result<(), UnpackError> {
        let bits = u32::from(w.bits());
        let necessarios = n_symbols as u64 * u64::from(bits);
        if self.bits_remaining() < necessarios {
            #[allow(clippy::cast_possible_truncation)]
            return Err(UnpackError::Truncated {
                expected_bytes: packed_len(n_symbols, w),
                got_bytes: (self.bits_remaining() / 8) as usize,
            });
        }
        out.reserve(n_symbols);

        if !self.is_byte_aligned() {
            for _ in 0..n_symbols {
                out.push(self.read_unchecked(bits));
            }
            return Ok(());
        }

        match Strategy::of(w) {
            Strategy::Native { bytes } => self.read_all_native(out, n_symbols, bytes as usize),
            Strategy::Group { symbols, bytes } => {
                self.read_all_group(out, n_symbols, w, symbols as usize, bytes as usize);
            }
            Strategy::Generic => {
                for _ in 0..n_symbols {
                    out.push(self.read_unchecked(bits));
                }
            }
        }
        Ok(())
    }

    /// `b ∈ {8, 16, 32}`: leitura direta de 1, 2 ou 4 bytes.
    ///
    /// Mesma razão do lado da escrita: os três casos precisam ser constantes
    /// para o compilador especializar, e `chunks_exact` dá a ele o tamanho
    /// do passo estaticamente.
    fn read_all_native(&mut self, out: &mut Vec<u32>, n_symbols: usize, bytes: usize) {
        #[allow(clippy::cast_possible_truncation)]
        let inicio = (self.bit_pos / 8) as usize;
        let fatia = &self.src[inicio..inicio + n_symbols * bytes];
        match bytes {
            1 => out.extend(fatia.iter().map(|&b| u32::from(b))),
            2 => out.extend(
                fatia
                    .chunks_exact(2)
                    .map(|c| u32::from(u16::from_be_bytes([c[0], c[1]]))),
            ),
            _ => out.extend(
                fatia
                    .chunks_exact(4)
                    .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]])),
            ),
        }
        self.bit_pos += (n_symbols * bytes * 8) as u64;
    }

    /// Grupo lido de uma vez num `u64` e fatiado em `symbols` símbolos.
    fn read_all_group(
        &mut self,
        out: &mut Vec<u32>,
        n_symbols: usize,
        w: Width,
        symbols: usize,
        bytes: usize,
    ) {
        let bits = u32::from(w.bits());
        let grupos = n_symbols / symbols;
        #[allow(clippy::cast_possible_truncation)]
        let inicio = (self.bit_pos / 8) as usize;
        let src = &self.src[inicio..];

        macro_rules! despacha {
            ($(($b:literal, $ns:literal, $nb:literal),)+) => {
                match w.bits() {
                    $($b => le_grupos::<$b, $ns, $nb>(out, src, grupos),)+
                    _ => unreachable!("largura sem grupo não chega ao caminho de grupo"),
                }
            };
        }
        para_cada_grupo!(despacha);

        self.bit_pos += (grupos * bytes * 8) as u64;
        // O resto do grupo não fecha em bytes: volta ao genérico.
        for _ in 0..(n_symbols % symbols) {
            out.push(self.read_unchecked(bits));
        }
    }
}

/// Empacota `values` na largura `w`.
///
/// # Errors
///
/// [`PackError::ValueTooWide`] se algum valor não couber em `w` bits.
pub fn pack(values: &[u32], w: Width) -> Result<Vec<u8>, PackError> {
    let mut writer = BitWriter::with_capacity_for(values.len(), w);
    writer.push_all(values, w)?;
    Ok(writer.finish())
}

/// Desempacota exatamente `n_symbols` símbolos de largura `w`.
///
/// Estrito de propósito: exige o número exato de bytes e o completamento em
/// zeros. `n_symbols` vem do cabeçalho `.hgr` (`docs/01 §8`), então divergir
/// dele significa cabeçalho e payload inconsistentes — coisa que precisa
/// falhar alto, não devolver símbolos plausíveis.
///
/// # Errors
///
/// [`UnpackError`] se o buffer for curto, sobrar byte, ou o completamento do
/// último byte não for zero.
pub fn unpack(bytes: &[u8], n_symbols: usize, w: Width) -> Result<Vec<u32>, UnpackError> {
    let esperados = packed_len(n_symbols, w);
    if bytes.len() < esperados {
        return Err(UnpackError::Truncated {
            expected_bytes: esperados,
            got_bytes: bytes.len(),
        });
    }
    if bytes.len() > esperados {
        return Err(UnpackError::TrailingBytes {
            expected_bytes: esperados,
            got_bytes: bytes.len(),
        });
    }

    let bits_uteis = n_symbols as u64 * u64::from(w.bits());
    #[allow(clippy::cast_possible_truncation)]
    let padding = (esperados as u64 * 8 - bits_uteis) as u32;
    if padding > 0 {
        let ultimo = bytes[esperados - 1];
        if ultimo & ((1u8 << padding) - 1) != 0 {
            return Err(UnpackError::PaddingNotZero {
                padding_bits: padding,
                last_byte: ultimo,
            });
        }
    }

    let mut out = Vec::with_capacity(n_symbols);
    BitReader::new(bytes).read_all_into(&mut out, n_symbols, w)?;
    Ok(out)
}

/// Empacota forçando o caminho genérico, ignorando a estratégia de `w`.
///
/// Existe por dois motivos, os dois da especificação:
///
/// 1. `docs/01 §7` manda **reportar o ganho** do caminho especializado, e
///    para isso é preciso medir o genérico na mesma largura;
/// 2. os testes comparam byte a byte as duas saídas — se divergirem, o
///    caminho rápido está errado.
///
/// Não use em produção: é a referência, não o caminho de trabalho.
///
/// # Errors
///
/// [`PackError::ValueTooWide`] se algum valor não couber em `w` bits.
pub fn pack_via_generic(values: &[u32], w: Width) -> Result<Vec<u8>, PackError> {
    let bits = w.bits();
    if bits < 32 {
        let limite = max_value(bits);
        if let Some((index, &value)) = values.iter().enumerate().find(|(_, &v)| v > limite) {
            return Err(PackError::ValueTooWide { index, value, bits });
        }
    }
    let mut writer = BitWriter::with_capacity_for(values.len(), w);
    for &v in values {
        writer.push_unchecked(v, u32::from(bits));
    }
    Ok(writer.finish())
}

/// Desempacota forçando o caminho genérico. Contraparte de [`pack_via_generic`].
///
/// # Errors
///
/// [`UnpackError::Truncated`] se faltarem bits para `n_symbols` símbolos.
pub fn unpack_via_generic(
    bytes: &[u8],
    n_symbols: usize,
    w: Width,
) -> Result<Vec<u32>, UnpackError> {
    let bits = u32::from(w.bits());
    let mut reader = BitReader::new(bytes);
    if reader.bits_remaining() < n_symbols as u64 * u64::from(bits) {
        #[allow(clippy::cast_possible_truncation)]
        return Err(UnpackError::Truncated {
            expected_bytes: packed_len(n_symbols, w),
            got_bytes: bytes.len(),
        });
    }
    let mut out = Vec::with_capacity(n_symbols);
    for _ in 0..n_symbols {
        out.push(reader.read_unchecked(bits));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::all_widths;

    fn w(bits: u8) -> Width {
        Width::new(bits).unwrap()
    }

    #[test]
    fn msb_first_em_vetores_conhecidos() {
        // b = 1: os bits entram do mais significativo para o menos.
        assert_eq!(
            pack(&[1, 0, 1, 1, 0, 0, 0, 1], w(1)).unwrap(),
            [0b1011_0001]
        );
        // b = 4: dois símbolos por byte, o primeiro no nibble alto.
        assert_eq!(pack(&[0xA, 0x5], w(4)).unwrap(), [0xA5]);
        // b = 8: identidade.
        assert_eq!(pack(&[0x00, 0x7F, 0xFF], w(8)).unwrap(), [0x00, 0x7F, 0xFF]);
        // b = 12: 2 símbolos em 3 bytes.
        assert_eq!(pack(&[0xABC, 0xDEF], w(12)).unwrap(), [0xAB, 0xCD, 0xEF]);
        // b = 16 e 32: big-endian nativo (CLAUDE.md, "Convenções").
        assert_eq!(pack(&[0xBEEF], w(16)).unwrap(), [0xBE, 0xEF]);
        assert_eq!(
            pack(&[0xDEAD_BEEF], w(32)).unwrap(),
            [0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    #[test]
    fn ultimo_byte_e_completado_com_zeros() {
        // 3 símbolos de 1 bit = 3 bits úteis, 5 bits de completamento.
        assert_eq!(pack(&[1, 1, 1], w(1)).unwrap(), [0b1110_0000]);
        // 1 símbolo de 12 bits = 12 bits úteis, 4 de completamento.
        assert_eq!(pack(&[0xFFF], w(12)).unwrap(), [0xFF, 0xF0]);
    }

    #[test]
    fn entrada_vazia_produz_fluxo_vazio_em_toda_largura() {
        for width in all_widths() {
            let bytes = pack(&[], width).unwrap();
            assert!(bytes.is_empty(), "b = {width}");
            assert_eq!(unpack(&bytes, 0, width).unwrap(), Vec::<u32>::new());
        }
    }

    #[test]
    fn um_simbolo_nos_extremos_em_toda_largura() {
        for width in all_widths() {
            for valor in [0, max_value(width.bits())] {
                let bytes = pack(&[valor], width).unwrap();
                assert_eq!(bytes.len(), packed_len(1, width), "b = {width}");
                assert_eq!(
                    unpack(&bytes, 1, width).unwrap(),
                    vec![valor],
                    "b = {width}"
                );
            }
        }
    }

    #[test]
    fn valores_extremos_alternados_em_toda_largura() {
        for width in all_widths() {
            let max = max_value(width.bits());
            // 37 é primo e não fecha nenhum grupo de packing, então o resto
            // do último grupo cai no caminho genérico em toda largura.
            let valores: Vec<u32> = (0..37).map(|i| if i % 2 == 0 { 0 } else { max }).collect();
            let bytes = pack(&valores, width).unwrap();
            assert_eq!(
                unpack(&bytes, valores.len(), width).unwrap(),
                valores,
                "b = {width}"
            );
        }
    }

    #[test]
    fn estrategia_rapida_produz_os_mesmos_bytes_que_a_generica() {
        for width in all_widths() {
            let max = max_value(width.bits());
            // Comprimentos que fecham grupo e comprimentos que não fecham.
            for n in [0usize, 1, 2, 3, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 100] {
                let valores: Vec<u32> = (0..n as u64)
                    .map(|i| {
                        let x = i.wrapping_mul(0x9E37_79B9) ^ 0x5DEE_CE66;
                        u32::try_from(x % (u64::from(max) + 1)).unwrap()
                    })
                    .collect();
                let rapido = pack(&valores, width).unwrap();
                let generico = pack_via_generic(&valores, width).unwrap();
                assert_eq!(rapido, generico, "b = {width}, n = {n}");
                assert_eq!(
                    unpack(&rapido, n, width).unwrap(),
                    unpack_via_generic(&generico, n, width).unwrap(),
                    "b = {width}, n = {n}"
                );
            }
        }
    }

    #[test]
    fn comprimentos_que_nao_fecham_byte_fazem_roundtrip() {
        for width in all_widths() {
            let max = max_value(width.bits());
            for n in 1..=40usize {
                let valores: Vec<u32> = (0..u32::try_from(n).unwrap())
                    .map(|i| (i * 7) % (max.min(1000) + 1))
                    .collect();
                let bytes = pack(&valores, width).unwrap();
                assert_eq!(bytes.len(), packed_len(n, width), "b = {width}, n = {n}");
                assert_eq!(
                    unpack(&bytes, n, width).unwrap(),
                    valores,
                    "b = {width}, n = {n}"
                );
            }
        }
    }

    #[test]
    fn os_256_bytes_fazem_roundtrip_em_toda_largura_que_os_comporta() {
        let todos: Vec<u32> = (0..=255).collect();
        for width in all_widths().into_iter().filter(|x| x.bits() >= 8) {
            let bytes = pack(&todos, width).unwrap();
            assert_eq!(
                unpack(&bytes, todos.len(), width).unwrap(),
                todos,
                "b = {width}"
            );
        }
    }

    #[test]
    fn valor_largo_demais_e_recusado_com_indice() {
        let erro = pack(&[0, 1, 4, 2], w(2)).unwrap_err();
        assert_eq!(
            erro,
            PackError::ValueTooWide {
                index: 2,
                value: 4,
                bits: 2
            }
        );
        // b = 32 comporta qualquer u32: nada é largo demais.
        assert!(pack(&[u32::MAX], w(32)).is_ok());
        // O limite é inclusivo em toda largura.
        for width in all_widths() {
            assert!(
                pack(&[max_value(width.bits())], width).is_ok(),
                "b = {width}"
            );
            if width.bits() < 32 {
                assert!(
                    pack(&[max_value(width.bits()) + 1], width).is_err(),
                    "b = {width}"
                );
            }
        }
    }

    #[test]
    fn valor_largo_demais_nao_deixa_fluxo_meio_gravado() {
        let mut bw = BitWriter::new();
        assert!(bw.push_all(&[1, 2, 99], w(4)).is_err());
        assert_eq!(bw.bits_written(), 0, "nada podia ter sido escrito");
    }

    #[test]
    fn unpack_recusa_truncado_sobra_e_completamento_sujo() {
        let width = w(12);
        let bytes = pack(&[0xABC, 0xDEF], width).unwrap();
        assert_eq!(bytes.len(), 3);

        assert_eq!(
            unpack(&bytes[..2], 2, width).unwrap_err(),
            UnpackError::Truncated {
                expected_bytes: 3,
                got_bytes: 2
            }
        );

        let mut sobrando = bytes.clone();
        sobrando.push(0x00);
        assert_eq!(
            unpack(&sobrando, 2, width).unwrap_err(),
            UnpackError::TrailingBytes {
                expected_bytes: 3,
                got_bytes: 4
            }
        );

        // 1 símbolo de 12 bits deixa 4 bits de completamento, que têm de ser zero.
        let mut sujo = pack(&[0xABC], width).unwrap();
        sujo[1] |= 0x0F;
        assert_eq!(
            unpack(&sujo, 1, width).unwrap_err(),
            UnpackError::PaddingNotZero {
                padding_bits: 4,
                last_byte: 0xCF
            }
        );
    }

    #[test]
    fn larguras_misturadas_fazem_roundtrip_desalinhadas() {
        // O caminho rápido só vale alinhado ao byte. Misturar larguras força
        // o genérico no meio do fluxo, que é o que o M4 vai fazer na E7.
        let plano = [
            (3u8, 5u32),
            (14, 9000),
            (8, 255),
            (1, 1),
            (32, u32::MAX),
            (7, 100),
        ];
        let mut bw = BitWriter::new();
        for &(bits, valor) in &plano {
            bw.push(valor, w(bits)).unwrap();
        }
        let bytes = bw.finish();

        let mut br = BitReader::new(&bytes);
        for &(bits, valor) in &plano {
            assert_eq!(br.read(w(bits)), Some(valor), "b = {bits}");
        }
    }

    #[test]
    fn push_all_desalinhado_bate_com_push_um_a_um() {
        for width in all_widths() {
            let max = max_value(width.bits());
            let valores: Vec<u32> = (0u32..23).map(|i| (i * 13) % (max.min(500) + 1)).collect();

            // Desalinha com 3 bits antes da sequência.
            let mut em_lote = BitWriter::new();
            em_lote.push(0b101, w(3)).unwrap();
            em_lote.push_all(&valores, width).unwrap();

            let mut um_a_um = BitWriter::new();
            um_a_um.push(0b101, w(3)).unwrap();
            for &v in &valores {
                um_a_um.push(v, width).unwrap();
            }

            assert_eq!(em_lote.finish(), um_a_um.finish(), "b = {width}");
        }
    }

    #[test]
    fn read_devolve_none_quando_faltam_bits() {
        let bytes = pack(&[0xFFF], w(12)).unwrap();
        let mut br = BitReader::new(&bytes);
        assert_eq!(br.read(w(12)), Some(0xFFF));
        assert_eq!(br.bits_remaining(), 4);
        assert_eq!(br.read(w(12)), None, "não há 12 bits sobrando");
        assert_eq!(br.read(w(4)), Some(0), "o completamento é zero");
    }

    #[test]
    fn packed_len_bate_com_o_tamanho_real_em_toda_largura() {
        for width in all_widths() {
            for n in [0usize, 1, 2, 7, 8, 100, 1000] {
                let valores = vec![0u32; n];
                assert_eq!(
                    pack(&valores, width).unwrap().len(),
                    packed_len(n, width),
                    "b = {width}, n = {n}"
                );
            }
        }
    }

    #[test]
    fn a_tabela_de_despacho_bate_com_o_que_width_calcula() {
        // A tabela `para_cada_grupo!` existe em literais para virar constante
        // de tipo. Se ela divergir do que `Width` deriva, o caminho rápido
        // passa a empacotar com o grupo errado — e cala.
        macro_rules! confere {
            ($(($b:literal, $ns:literal, $nb:literal),)+) => {
                let tabela: Vec<(u8, u8, u8)> = vec![$(($b, $ns, $nb)),+];

                for &(bits, ns, nb) in &tabela {
                    let width = w(bits);
                    assert_eq!(width.pack_group_symbols(), ns, "b = {bits}: símbolos do grupo");
                    assert_eq!(width.pack_group_bytes(), nb, "b = {bits}: bytes do grupo");
                    assert_eq!(
                        Strategy::of(width),
                        Strategy::Group { symbols: ns, bytes: nb },
                        "b = {bits}"
                    );
                }

                // E o inverso: nenhuma largura de grupo ficou fora da tabela.
                for width in all_widths() {
                    if let Strategy::Group { .. } = Strategy::of(width) {
                        assert!(
                            tabela.iter().any(|&(b, _, _)| b == width.bits()),
                            "b = {width} usa grupo mas não está em para_cada_grupo!"
                        );
                    }
                }
            };
        }
        para_cada_grupo!(confere);
    }

    #[test]
    fn estrategias_cobrem_as_larguras_que_a_especificacao_pede() {
        // docs/01 §7 pede caminho especializado para {8,16,32} e {12,14,24}.
        for bits in [8u8, 16, 32] {
            assert!(
                matches!(Strategy::of(w(bits)), Strategy::Native { .. }),
                "b = {bits} devia ser nativo"
            );
        }
        for bits in [12u8, 14, 24] {
            assert!(
                matches!(Strategy::of(w(bits)), Strategy::Group { .. }),
                "b = {bits} devia ter caminho de grupo"
            );
        }
        // Todo grupo cabe em u64 e fecha em bytes.
        for width in all_widths() {
            if let Strategy::Group { symbols, bytes } = Strategy::of(width) {
                let bits = u32::from(symbols) * u32::from(width.bits());
                assert!(
                    bits <= 64,
                    "b = {width}: grupo de {bits} bits não cabe em u64"
                );
                assert_eq!(bits, u32::from(bytes) * 8, "b = {width}");
            }
        }
    }
}
