# ETAPA E2 — Modelo paramétrico e formato `.hgr`

Fechamento da etapa E2 de [`docs/03-ETAPAS.md`](../docs/03-ETAPAS.md).

- Commit da etapa: `fd973e4`
- Data: 2026-09-18
- Toolchain: `rustc 1.94.1 (e408947bf 2026-03-25)`

> **Aviso que vale para o documento inteiro.** Existe codec funcionando de
> ponta a ponta, e por isso existem, pela primeira vez, números de tamanho.
> **Nenhum deles sustenta ou refuta H1–H7.** Não há corpus real (E3) e não
> há baseline alguma — gzip, zstd, brotli, lz4, BPE são obrigatórias por
> `docs/02 §1` e chegam na E4. Tudo na §3 é smoke check sobre corpora
> sintéticos, que `docs/02 §2.2` diz servirem para **explicar curvas**, não
> para sustentar a tese.

---

## 1. O que foi feito

Três módulos novos em `higher-core`:

| Módulo | Conteúdo | Especificação |
|---|---|---|
| `table` | layout de namespace, construção M2, trie do P-greedy, serialização canônica | `docs/01 §1, §3, §4, §5` |
| `hgr` | cabeçalho de 36 bytes, big-endian | `docs/01 §8` |
| `codec` | `Model` com `tokenize`/`encode`/`decode`, `EncodeStats` | `docs/01 §1, §6` |

### Layout de namespace

| Faixa | Código 0 | Códigos seguintes | Slots dinâmicos |
|---|---|---|---|
| LOWER `b < 8` | **ESCAPE** + 8 bits brutos | `1..2^b−1`, por ganho | `2^b − 1` |
| BASE `b = 8` | byte `0x00` | `1..255`, identidade | **0** |
| HIGHER `b > 8` | byte `0x00` | `1..255` legado, `256..2^b−1` por ganho | `2^b − 256` |

### Construção M2

Candidatos por n-grama de 1 a 16 bytes e por palavra (corte em 64 bytes),
com frequência mínima 2, ranqueados por `ganho(s) = freq(s) · (8·len(s) − b)`
e inseridos até encher os slots. Ordem de desempate fixa (ganho, depois
comprimento, depois os bytes), porque sem determinismo dois treinos do mesmo
corpus dariam `table_id` diferentes e **nenhum run seria reproduzível**.

A contagem usa **poda estilo Apriori**: um n-grama de tamanho `L` só é
contado se o prefixo de `L−1` já é frequente. Sem isso, contar 1..16 num
corpus de 10 MB seriam 160 milhões de chaves, e a construção da tabela
estouraria a memória antes de a E4 medir qualquer coisa. Um teste fixa que a
poda não perde n-grama frequente — ela é otimização de memória, não pode
mudar a tabela.

### Treino nunca é teste

A tabela carrega o sha256 do corpus de treino e ele vai para o artefato
serializado. É o antídoto ao defeito D1 de `docs/05 §3`: sem o corpus de
treino registrado junto do resultado, um vazamento é indetectável depois do
fato. O arquivo de teste do aceite tem uma asserção explícita de que os dois
corpora são disjuntos, para ninguém "simplificar" os dois num só depois.

---

## 2. Critérios de aceite

> **Aceite (E2):** roundtrip em todo `b` 1..32 sobre fuzz de bytes
> arbitrários (incluindo os 256 valores). Decoder rejeita `table_id` errado e
> crc inválido com erro tipado, sem panic.

### 2.1 Roundtrip em todo `b` 1..32 — verde [MEDIDO]

```
$ cargo test --workspace
test result: ok. 60 passed; 0 failed   (unitários de higher-core)
test result: ok. 11 passed; 0 failed   (tests/hgr_roundtrip.rs — o aceite)
test result: ok.  6 passed; 0 failed   (tests/packing_roundtrip.rs — E1)
test result: ok.  1 passed; 0 failed   (higher-wasm)
test result: ok.  0 passed; 0 failed; 3 ignored   (tests/exploracao_e2.rs)
```

78 testes, 0 falhas. O roundtrip roda nas 32 larguras × 2 tabelas
(`legacy_only` e M2 treinada) × 10 entradas:

| Entrada | Por que está na lista |
|---|---|
| vazia | fronteira de `n_symbols = 0` |
| 1 byte (`0x00` e `0xFF`) | fronteira de símbolo único |
| os 256 bytes, e invertidos | exigido pelo aceite e por `CLAUDE.md` |
| 1000 bytes iguais (`0x00`, `0xFF`) | casamento máximo da trie |
| corpus de teste (disjunto do treino) | o caminho normal, com blocos multibyte |
| fuzz de 4096 bytes | bytes arbitrários, seed 42 |
| fuzz de alfabeto restrito (ACGT) | o cenário de H5 |

Mais proptest com largura e entrada sorteadas (256 casos).

### 2.2 Decoder rejeita `table_id` errado e crc inválido — verde [MEDIDO]

Nas **32** larguras, com erro tipado:

| Cenário | Erro | Teste |
|---|---|---|
| fluxo escrito com outra tabela | `DecodeError::TableMismatch` | `decoder_rejeita_table_id_errado_com_erro_tipado` |
| 1 bit virado no payload | `DecodeError::CrcMismatch` | `decoder_rejeita_crc_invalido_com_erro_tipado` |
| campo de crc adulterado | `DecodeError::CrcMismatch` | idem |
| largura trocada | `DecodeError::WidthMismatch` | `decoder_rejeita_largura_e_variante_trocadas` |
| variante trocada | `DecodeError::VariantMismatch` | idem |
| `n_input_bytes` mentiroso | `DecodeError::LengthMismatch` | `decoder_rejeita_n_input_bytes_mentiroso` |
| payload maior do que `n_symbols` explica | `DecodeError::PaddingNotZero` | `decoder_rejeita_payload_maior_do_que_n_symbols_explica` |
| flag de M4 ou de sync markers | `DecodeError::UnsupportedFlags` | `decoder_recusa_flags_ainda_nao_implementadas` |

### 2.3 Sem panic — verde [MEDIDO]

"Sem panic" foi tratado como critério, não como detalhe: **não há um único
`unwrap`, `expect` ou indexação não verificada nos caminhos de decode**. Os
dois `expect` que existiam foram eliminados, não documentados —
`Width::BASE` virou constante e `BitReader::remaining_is_zero` substituiu a
construção de uma `Width` a partir do número de bits sobrando.

Um caso merece nota: a capacidade do buffer de saída sai de `n_symbols`, que
já foi validado contra o tamanho do payload, e **não** de `n_input_bytes`,
que o cabeçalho declara. Um fluxo com crc válido e `n_input_bytes = 2^63`
pediria uma alocação absurda antes de qualquer verificação; quem pega a
mentira é o `LengthMismatch` no fim, sem reservar memória para ela.

Verificado por fuzz: 6 400 fluxos arbitrários por largura (lixo puro, fluxo
válido com byte corrompido, fluxo válido truncado), mais 2 000 artefatos de
tabela aleatórios, mais proptest sobre cabeçalho e fluxo arbitrários.

### 2.4 Os testes foram verificados por mutação

| Mutação | Quem pegou |
|---|---|
| a trie nunca casa nada | `higher_prefere_o_maior_bloco`, `a_redução_horizontal_e_o_limiar_da_tese` |
| decoder aceita qualquer crc | `decoder_rejeita_crc_invalido_com_erro_tipado` |
| decoder aceita qualquer `table_id` | `decoder_rejeita_table_id_errado_com_erro_tipado` |

A primeira mutação expôs um ponto cego real: **o roundtrip sozinho continua
passando com a trie desligada**, porque todo byte cai no código legado e
volta igual. Um codec que não comprime nada passaria no aceite literal. Por
isso o teste de aceite passou a exigir, para HIGHER com M2 no corpus de
teste, `horiz_reduction > 0` e `n_symbols < input_bytes`.

---

## 3. Smoke check [MEDIDO] — e o que ele não é

Reproduzível com:

```bash
cargo test -p higher-core --release --test exploracao_e2 -- --ignored --nocapture
```

Os testes são `#[ignore]` porque **imprimem, não verificam**. Repetindo o
aviso do topo: sem corpus real, sem baseline, sem repetição, sem intervalo de
confiança. Nada aqui entra na avaliação de H1–H7. **b/B em negrito** marca
onde ficou abaixo de `base-8`.

**logs sintéticos — treino 56859 B, teste 18930 B**

| `b` | slots | usados | ocup. | tab.bytes | b/B | esc. | red.horiz | limiar |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 100.000% | 56 | **6.950** | 97.3% | 21.1% | 12.5% |
| 2 | 3 | 3 | 100.000% | 78 | **7.739** | 97.3% | 21.1% | 25.0% |
| 3 | 7 | 7 | 100.000% | 117 | 8.524 | 97.3% | 21.1% | 37.5% |
| 4 | 15 | 15 | 100.000% | 184 | **7.622** | 93.6% | 33.8% | 50.0% |
| 7 | 127 | 127 | 100.000% | 1 343 | 8.024 | 92.4% | 44.4% | 87.5% |
| 8 | 0 | 0 | 0.000% | 44 | 8.015 | 0.0% | 0.0% | 0.0% |
| 9 | 256 | 256 | 100.000% | 2 727 | **4.300** | 0.0% | 52.4% | 11.1% |
| 11 | 1 792 | 1 792 | 100.000% | 19 825 | **3.080** | 0.0% | 72.1% | 27.3% |
| 12 | 3 840 | 3 840 | 100.000% | 43 647 | **2.090** | 0.0% | 82.7% | 33.3% |
| 13 | 7 936 | 7 936 | 100.000% | 93 973 | **1.907** | 0.0% | 85.5% | 38.5% |
| 14 | 16 128 | 16 128 | 100.000% | 201 139 | **1.654** | 0.0% | 88.3% | 42.9% |
| 15 | 32 512 | 32 512 | 100.000% | 426 726 | **1.570** | 0.0% | 89.6% | 46.7% |
| 16 | 65 280 | 57 530 | 88.128% | 649 113 | **1.510** | 0.0% | 90.7% | 50.0% |
| 20 | 1 048 320 | 57 530 | 5.488% | 649 113 | **1.884** | 0.0% | 90.7% | 60.0% |
| 24 | 16 776 960 | 56 636 | 0.338% | 645 537 | **2.263** | 0.0% | 90.6% | 66.7% |
| 32 | 4 294 967 040 | 54 504 | 0.001% | 634 877 | **3.178** | 0.0% | 90.1% | 75.0% |

**DNA (alfabeto de 4) — treino 200000 B, teste 50000 B**

| `b` | slots | usados | ocup. | tab.bytes | b/B | esc. | red.horiz | limiar |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 100.000% | 46 | **7.014** | 75.1% | 0.0% | 12.5% |
| 2 | 3 | 3 | 100.000% | 50 | **4.014** | 25.1% | 0.0% | 25.0% |
| 3 | 7 | 7 | 100.000% | 61 | **2.553** | 0.0% | 15.1% | 37.5% |
| 4 | 15 | 15 | 100.000% | 85 | **2.418** | 0.0% | 39.7% | 50.0% |
| 7 | 127 | 127 | 100.000% | 571 | **2.218** | 0.0% | 68.4% | 87.5% |
| 8 | 0 | 0 | 0.000% | 44 | 8.006 | 0.0% | 0.0% | 0.0% |
| 9 | 256 | 256 | 100.000% | 1 228 | **2.450** | 0.0% | 72.8% | 11.1% |
| 11 | 1 792 | 1 792 | 100.000% | 10 796 | **2.160** | 0.0% | 80.4% | 27.3% |
| 12 | 3 840 | 3 840 | 100.000% | 25 132 | **2.145** | 0.0% | 82.2% | 33.3% |
| 13 | 7 936 | 7 936 | 100.000% | 56 284 | **2.119** | 0.0% | 83.7% | 38.5% |
| 14 | 16 128 | 16 128 | 100.000% | 121 889 | **2.112** | 0.0% | 85.0% | 42.9% |
| 15 | 32 512 | 32 512 | 100.000% | 264 219 | **2.105** | 0.0% | 86.0% | 46.7% |
| 16 | 65 280 | 65 280 | 100.000% | 591 969 | **2.159** | 0.0% | 86.5% | 50.0% |
| 20 | 1 048 320 | 144 171 | 13.753% | 1 368 953 | **2.507** | 0.0% | 87.5% | 60.0% |
| 24 | 16 776 960 | 144 107 | 0.859% | 1 368 697 | **3.007** | 0.0% | 87.5% | 66.7% |
| 32 | 4 294 967 040 | 143 851 | 0.003% | 1 367 417 | **4.008** | 0.0% | 87.5% | 75.0% |

**bytes aleatórios (controle negativo) — treino 200000 B, teste 50000 B**

| `b` | slots | usados | ocup. | tab.bytes | b/B | esc. | red.horiz | limiar |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 100.000% | 46 | 8.976 | 99.6% | 0.0% | 12.5% |
| 2 | 3 | 3 | 100.000% | 50 | 9.913 | 98.8% | 0.0% | 25.0% |
| 3 | 7 | 7 | 100.000% | 58 | 10.793 | 97.3% | 0.0% | 37.5% |
| 4 | 15 | 15 | 100.000% | 74 | 11.544 | 94.2% | 0.0% | 50.0% |
| 7 | 127 | 127 | 100.000% | 298 | 11.044 | 50.5% | 0.0% | 87.5% |
| 8 | 0 | 0 | 0.000% | 44 | 8.006 | 0.0% | 0.0% | 0.0% |
| 9 | 256 | 256 | 100.000% | 812 | 8.969 | 0.0% | 0.4% | 11.1% |
| 11 | 1 792 | 1 792 | 100.000% | 5 435 | 10.704 | 0.0% | 2.7% | 27.3% |
| 12 | 3 840 | 3 840 | 100.000% | 12 836 | 11.534 | 0.0% | 3.9% | 33.3% |
| 13 | 7 936 | 7 936 | 100.000% | 25 124 | 11.794 | 0.0% | 9.3% | 38.5% |
| 14 | 16 128 | 16 128 | 100.000% | 49 700 | 11.398 | 0.0% | 18.6% | 42.9% |
| 15 | 32 512 | 32 512 | 100.000% | 98 852 | 10.160 | 0.0% | 32.3% | 46.7% |
| 16 | 65 280 | 1 269 | 1.944% | 5 123 | 16.003 | 0.0% | 0.0% | 50.0% |
| 20 | 1 048 320 | 1 269 | 0.121% | 5 123 | 20.002 | 0.0% | 0.0% | 60.0% |
| 24 | 16 776 960 | 3 | 0.000% | 59 | 24.006 | 0.0% | 0.0% | 66.7% |
| 32 | 4 294 967 040 | 0 | 0.000% | 44 | 32.006 | 0.0% | 0.0% | 75.0% |

### 3.1 O que estes números mostram, e só isso

1. **O codec funciona e comprime onde deveria.** Em logs sintéticos a curva
   tem mínimo em `b = 16` (1,510 b/B); em DNA, `b = 15` chega a 2,105 b/B,
   perto dos 2 bits/base que o alfabeto de 4 letras permite.
2. **O controle negativo se comporta como controle negativo.** Em bytes
   aleatórios nenhuma largura vence `base-8`. Era o resultado esperado, e é
   bom que tenha aparecido sem ajuste.
3. **A ocupação despenca.** Em `b = 16` a tabela usa 88% dos slots; em
   `b = 24`, 0,3%; em `b = 32`, 0,001%. É o "capacidade ≠ preenchimento" de
   `docs/00 §5.5` aparecendo pela primeira vez em número.
4. **A tabela é enorme perto do corpus.** Em `b = 16`, `table_bytes` = 649 113
   contra 18 930 bytes de corpus de teste: a tabela é **34×** o que ela
   comprime. `bits_per_byte` não a inclui, por `docs/02 §3`, mas
   `breakeven_files` vai mandar a conta na E4 — e é ela que decide se esse
   1,510 significa alguma coisa.
5. **O LOWER escapa quase tudo em alfabeto grande e nada em alfabeto
   restrito.** 97,3% de escape nos logs, 0% em DNA a partir de `b = 3`. É
   exatamente a forma de H5 (`docs/00 §6`), ainda sem valor de verdade.

---

## 4. Dois achados que mudam decisões

### 4.1 A régua `base-8` paga o cabeçalho — [MEDIDO]

A ambiguidade §4.6 da E0 deixou de ser hipotética:

| entrada | `base-8` b/B | overhead do cabeçalho |
|---:|---:|---:|
| 1 000 B | 8,2880 | 3,475% |
| 10 000 B | 8,0288 | 0,359% |
| 100 000 B | 8,0029 | 0,036% |
| 1 000 000 B | 8,0003 | 0,004% |
| 10 000 000 B | 8,0000 | 0,000% |

`docs/02 §1` manda rodar a partir de 10 KB. Ali a régua está em 8,029 b/B, e
"vencer `base-8`" ganha 0,36% de folga que não vem da arquitetura, vem de
36 bytes de cabeçalho. **Continua valendo a proposta da E0:** a régua é o
arquivo cru (8,000 exatos) e o `.hgr` de `b = 8` é reportado à parte.
**(decidir na E4, antes de qualquer número de H1)**

### 4.2 A função de ganho tem um degrau em `b = 16` — [MEDIDO]

`docs/01 §4` define `ganho(s) = freq(s) · (8·len(s) − b)`. Isso mede economia
contra **bytes crus**, e é coerente com `docs/00 §3.1` ("cada símbolo precisa
cobrir em média mais que `b/8` bytes"). A consequência é um corte duro: um
bloco de `L` bytes só entra se `8L > b`. Em `b = 16`, todo bloco de 2 bytes
pontua **exatamente zero** e some da tabela; em `b = 24`, somem os de 3.

Dentro do modelo, porém, um bloco de 2 bytes em `b = 16` custa 16 bits contra
32 do par de códigos legados — economiza metade. Medi as duas fórmulas nos
mesmos corpora (`alternativa − b`, com `alternativa = len·b` no HIGHER e
`len·(b+8)` no LOWER):

| Corpus | `b` | entradas (spec → alt) | b/B (spec → alt) | diferença |
|---|---:|---|---|---:|
| bytes aleatórios | 16 | 1 269 → 54 248 | 16,003 → 8,861 | **44,6%** |
| bytes aleatórios | 24 | 3 → 54 248 | 24,006 → 13,289 | **44,6%** |
| bytes aleatórios | 32 | 0 → 54 248 | 32,006 → 17,716 | **44,6%** |
| dígitos | 32 | 79 356 → 90 456 | 9,633 → 6,945 | **27,9%** |
| logs | 11 | 1 792 → 1 792 | 3,080 → 2,757 | 10,5% |
| logs | 13 | 7 936 → 7 936 | 1,907 → 1,718 | 9,9% |
| logs | 32 | 54 504 → 57 741 | 3,178 → 3,004 | 5,5% |
| DNA | todas | ~iguais | ~iguais | < 1% |

Implementei **a fórmula da especificação**, que é a autoritativa, e registro
a alternativa como insumo medido. Duas observações para quem for decidir:

- a conclusão qualitativa não muda onde importa: em bytes aleatórios as duas
  fórmulas perdem de `base-8`, como tem de ser;
- mas a **forma da curva** muda muito, e o gráfico 1 de `docs/02 §8` é uma
  curva. Um degrau de 44,6% em `b = 16` num gráfico de b/B × b seria lido
  como propriedade da arquitetura, quando é propriedade do critério de
  seleção.

**(decidir na E6**, que é onde `docs/03` põe a re-pontuação iterativa e o
relatório "M2 iterativo vs M2 ingênuo vs M1"**)**

---

## 5. Desvios da especificação

| # | Desvio | Motivo |
|---|---|---|
| V1 | Candidatos n-grama de **1** a 16, não de 2 a 16 | No LOWER, bloco de 1 byte é o que evita escape e tem ganho positivo (`8 − b > 0` para todo `b < 8`). No HIGHER a própria função de ganho o exclui (`8 − b < 0`), então gerar 1..16 é uniforme e não muda a tabela — verificado em teste. |
| V2 | Frequência mínima de 2 para um candidato existir | Com `custo_slot = 0`, a fórmula de `docs/01 §4` dá ganho positivo até para bloco visto uma vez, o que encheria a tabela de entradas que nunca reaparecem. É também o que torna a poda Apriori possível. |
| V3 | Palavras cortadas em 64 bytes | Em dados binários, "sequência sem espaço nem pontuação" pode ser o arquivo inteiro. |
| V4 | Tokens estruturais (chaves JSON, prefixos de log, timestamps) **não** implementados | `docs/01 §4.1` os lista, mas `docs/03` define o escopo da E2 como "n-grama 2..16 + palavras". Ficam para a E6, junto da re-pontuação. |
| V5 | `Table::legacy_only` rotulada `M1` nos testes, com a camada Tipo 1 **vazia** | `docs/01 §2` define M1 como "legado + Tipo 1 + blocos". O Tipo 1 (blocos Unicode, operadores, pontuação) é explicitamente E6. Até lá, M1 é só o legado — o piso da faixa. |
| V6 | `unpack` estrito da E1 **não** é usado pelo codec | No LOWER o fluxo mistura larguras (código de `b` bits, byte de 8), então o decode usa `BitReader` direto. A checagem de completamento em zeros foi reimplementada no decoder (`PaddingNotZero`), para a garantia não se perder. |
| V7 | Dependências novas: `sha2` e `crc32fast` | `docs/01 §8` exige sha256 e crc32. Escrevê-los à mão seriam ~150 linhas de código criptográfico não testado num projeto cujo produto é medição. Ambos compilam para `wasm32-unknown-unknown`, o que foi verificado. |

---

## 6. Ambiguidades resolvidas e novas

### Resolvidas

- **§4.2 da E0 — `base-8:M2` é nome legítimo ou erro?** É legítimo, e é
  inofensivo. `base-8` tem capacidade 256 e 256 bytes legados, logo **zero**
  slots dinâmicos: a tabela sai vazia mesmo treinada, e o payload é byte a
  byte igual ao de `base-8`. Só variante e `table_id` no cabeçalho diferem.
  Fixado no teste `base_8_m2_e_identico_a_base_8`.

### Novas

1. **`n_symbols` conta o ESCAPE como um símbolo.** `docs/01 §8` não diz, e a
   escolha afeta `horiz_reduction` (`docs/02 §3`) em toda a faixa LOWER.
   Adotado: o ESCAPE é **um** símbolo, e os 8 bits brutos são carga dele.
   A alternativa (contar dois) faria a redução horizontal do LOWER parecer
   pior sem nenhuma mudança de bits no fio. **(confirmar na E4)**
2. **`bits_per_byte` não inclui `table_bytes`.** É o que `docs/02 §3`
   descreve, ao listar `table_bytes` e `breakeven_files` como métricas
   separadas. Mas com tabela 34× maior que o corpus (§3.1), relatar só
   `bits_per_byte` seria enganoso. A E4 precisa publicar as duas colunas
   lado a lado em todo gráfico de tamanho, não em tabelas separadas.
   **(decidir na E4)**

As dez ambiguidades da E0 (`results/ETAPA-00.md §4`) e as duas da E1
(`results/ETAPA-01.md §5`) continuam abertas, menos a §4.2 resolvida acima.

---

## 7. Próxima etapa

**E3 — Corpora.** Geradores sintéticos com seed, `manifest.json` com sha256
dos corpora reais, e a divisão treino/validação/teste por bloco contíguo. É
o que tira os números da §3 do terreno sintético — e sem ela a E4 não tem o
que medir.
