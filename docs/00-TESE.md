# 00 — Tese: redução de consumo por expansão vertical de símbolos

## 1. Enunciado

A computação atual usa o byte (8 bits, 256 símbolos) como símbolo base. A
Arquitetura Higher propõe que **aumentar a largura do símbolo** (`b` bits,
`2^b` símbolos) permite que **um símbolo represente mais conteúdo** (um bloco,
palavra, comando, token ou conceito). Isso reduz o **número de símbolos** que
precisam ser armazenados, transmitidos e processados.

- **Eixo vertical:** bits por símbolo (`b`), ou seja, a capacidade da tabela (`2^b`).
- **Eixo horizontal:** número de símbolos no fluxo (`N`).
- **Troca:** cada símbolo fica mais caro (`b > 8`), mas aparecem menos
  símbolos (`N_b < N_8`).

Régua do projeto (definida pelo fundador):

> Quanto podemos mudar na arquitetura dos nossos dados para que isso se
> reflita em ganho de processamento, transferência, armazenamento e outros
> fatores relevantes?

## 2. Origem conceitual (documento de especificação original)

| Camada | Definição | Natureza |
|---|---|---|
| **Tipo 1 — Mapeamento Humano** | tabela universal: alfabetos, operadores, pontuação, símbolos customizados | estática, pré-acordada |
| **Tipo 2 — Otimização de Máquina** | símbolos Higher atribuídos a blocos recorrentes de dados | estática pré-acordada (M1/M2/M3) |
| **Tipo 4 — Adaptativa** | namespace que muda durante o fluxo, com uma tarefa que sincroniza o vocabulário | dinâmica (M4) |

Esclarecimento do fundador que orienta tudo: a tabela "dinâmica" **não é uma
tabela única que muda**. São **várias tabelas fixas de 2^b símbolos,
pré-acordadas nos dois lados**, com versões diferentes. Os 256 símbolos do byte
são mantidos, e a escolha dos símbolos adicionais é feita por avaliação de
ganho. O adaptativo de verdade é testado à parte, em um quarto modelo (M4).

Ideia paralela (camada de apresentação, **fora do núcleo de compressão**):
cada símbolo pode ter um glifo/imagem que um humano entenda intuitivamente.
Por exemplo, 14 bits = uma matriz 128×128 de 1 bit, ou um glifo procedural
com 7 bits de forma e 7 de modificador. Ficou registrada como etapa opcional.

## 3. Matemática do limiar

### 3.1 HIGHER (b > 8)

Custo do fluxo: `bits = N_b · b` (mais cabeçalho). Para ganhar do byte:

```
N_b · b < N_8 · 8   ⇔   N_b / N_8 < 8 / b   ⇔   redução_horizontal > 1 − 8/b
```

Em outras palavras, cada símbolo precisa cobrir **em média mais que `b/8`
bytes**. No `higher-14`, isso é 1,75 byte por símbolo. No `higher-32`, são 4 bytes.

### 3.2 LOWER (b < 8), com escape

O código `0` significa ESCAPE e é seguido do byte bruto (`b + 8` bits). Se `c`
é a fração de bytes cobertos sem escape (considerando só símbolos de 1 byte):

```
bits/byte = c·b + (1−c)·(b+8) = b + 8·(1−c)   <  8   ⇔   c > b/8
```

Se o LOWER também puder mapear blocos multibyte, o limiar melhora. A
especificação permite isso (ver `01`).

### 3.3 Tabela completa 1..32

| b | Faixa | Nome | Capacidade 2^b | Contêiner RAM | Packing (símbolos→bytes) | Limiar |
|---:|---|---|---:|---|---|---|
| 1 | LOWER | `lower-1` | 2 | u8 | 8→1 | cobertura sem escape > 12,5% |
| 2 | LOWER | `lower-2` | 4 | u8 | 4→1 | cobertura sem escape > 25,0% |
| 3 | LOWER | `lower-3` | 8 | u8 | 8→3 | cobertura sem escape > 37,5% |
| 4 | LOWER | `lower-4` | 16 | u8 | 2→1 | cobertura sem escape > 50,0% |
| 5 | LOWER | `lower-5` | 32 | u8 | 8→5 | cobertura sem escape > 62,5% |
| 6 | LOWER | `lower-6` | 64 | u8 | 4→3 | cobertura sem escape > 75,0% |
| 7 | LOWER | `lower-7` | 128 | u8 | 8→7 | cobertura sem escape > 87,5% |
| 8 | BASE | `base-8` | 256 | u8 | 1→1 | régua |
| 9 | HIGHER | `higher-9` | 512 | u16 | 8→9 | redução de símbolos > 11,1% |
| 10 | HIGHER | `higher-10` | 1.024 | u16 | 4→5 | redução > 20,0% |
| 11 | HIGHER | `higher-11` | 2.048 | u16 | 8→11 | redução > 27,3% |
| 12 | HIGHER | `higher-12` | 4.096 | u16 | 2→3 | redução > 33,3% |
| 13 | HIGHER | `higher-13` | 8.192 | u16 | 8→13 | redução > 38,5% |
| 14 | HIGHER | `higher-14` | 16.384 | u16 | 4→7 | redução > 42,9% |
| 15 | HIGHER | `higher-15` | 32.768 | u16 | 8→15 | redução > 46,7% |
| 16 | HIGHER | `higher-16` | 65.536 | u16 | 1→2 (nativo) | redução > 50,0% |
| 17 | HIGHER | `higher-17` | 131.072 | u32 | 8→17 | redução > 52,9% |
| 18 | HIGHER | `higher-18` | 262.144 | u32 | 4→9 | redução > 55,6% |
| 19 | HIGHER | `higher-19` | 524.288 | u32 | 8→19 | redução > 57,9% |
| 20 | HIGHER | `higher-20` | 1.048.576 | u32 | 2→5 | redução > 60,0% |
| 21 | HIGHER | `higher-21` | 2.097.152 | u32 | 8→21 | redução > 61,9% |
| 22 | HIGHER | `higher-22` | 4.194.304 | u32 | 4→11 | redução > 63,6% |
| 23 | HIGHER | `higher-23` | 8.388.608 | u32 | 8→23 | redução > 65,2% |
| 24 | HIGHER | `higher-24` | 16.777.216 | u32 | 1→3 | redução > 66,7% |
| 25 | HIGHER | `higher-25` | 33.554.432 | u32 | 8→25 | redução > 68,0% |
| 26 | HIGHER | `higher-26` | 67.108.864 | u32 | 4→13 | redução > 69,2% |
| 27 | HIGHER | `higher-27` | 134.217.728 | u32 | 8→27 | redução > 70,4% |
| 28 | HIGHER | `higher-28` | 268.435.456 | u32 | 2→7 | redução > 71,4% |
| 29 | HIGHER | `higher-29` | 536.870.912 | u32 | 8→29 | redução > 72,4% |
| 30 | HIGHER | `higher-30` | 1.073.741.824 | u32 | 4→15 | redução > 73,3% |
| 31 | HIGHER | `higher-31` | 2.147.483.648 | u32 | 8→31 | redução > 74,2% |
| 32 | HIGHER | `higher-32` | 4.294.967.296 | u32 | 1→4 (nativo) | redução > 75,0% |

Larguras nativas: 8, 16 e 32. Entre elas, o custo de packing (shifts e
máscaras) e o desalinhamento são variáveis a medir, não a presumir.

## 4. Os três planos de ganho

| Plano | O que a tese promete | Como pode falhar |
|---|---|---|
| **Armazenamento** | menos bits totais | a tabela ocupa memória/disco; em `b ≥ ~20` a tabela não tem conteúdo útil suficiente para preencher e símbolos ficam caros |
| **Transmissão** | menos bytes no fio | cabeçalho/dicionário; no M4, deltas de sincronização; erro de 1 bit desalinha o fluxo empacotado |
| **Processamento** | menos elementos para iterar, comparar, buscar, hashear | custo de encode/decode; packing não alinhado; tabela grande não cabe em cache (L1 32–48 KB, L2 ~1–2 MB) |

**Processamento é o foco.** Para medi-lo, separe três representações:

1. **Fio/disco:** empacotado em `b` bits.
2. **RAM alinhada:** cada símbolo num contêiner `u8/u16/u32`. Operações rodam aqui.
3. **RAM empacotada:** operações rodam direto nos bits empacotados.

A hipótese de processamento mais defensável é esta: operar **no domínio de
símbolos** (sem voltar a bytes) sobre N_b elementos custa menos que operar
sobre N_8 bytes, **e** a largura fixa permite acesso aleatório O(1). zstd e
gzip não oferecem isso sem descompressão total. Essa é a possível vantagem
competitiva contra compressores gerais e deve ser testada diretamente.

## 5. Contra-argumentos que o estudo precisa enfrentar

1. **Teoria da informação.** A largura do símbolo, sozinha, não cria ganho.
   O ganho vem do **dicionário** (modelagem de redundância). Um código de
   largura fixa com dicionário estático é parente do LZW/BPE. Espera-se que
   ele perca, em razão de compressão, para codificadores de entropia (zstd,
   brotli). O estudo deve medir **quanto** perde e o **que ganha em troca**
   (velocidade, acesso aleatório, processamento no domínio comprimido).
2. **Vazamento de treino.** Tabela construída no próprio corpus de teste
   infla o resultado. O protótipo anterior fazia isso (ver `05`).
3. **Corpora sintéticos favoráveis.** Geradores com vocabulário pequeno
   premiam dicionários. Os resultados principais vêm de corpora reais.
4. **Python não mede processamento.** O tempo do interpretador domina. Por isso o núcleo é em Rust.
5. **Capacidade ≠ preenchimento.** `higher-32` tem 4,29 bilhões de slots, mas
   nenhum corpus de treino realista preenche isso com ganho positivo. Isso
   precisa aparecer como "slots úteis" medidos.

## 6. Hipóteses falseáveis

| ID | Hipótese | Refutada se |
|---|---|---|
| H1 | Existe um `b*` em 9..32 com bits/byte menor que `base-8` em ≥ 3 categorias de corpus real, com tabela treinada em corpus disjunto | nenhum `b` vence `base-8` em ≥ 3 categorias |
| H2 | O sweet spot está em 12–16 | o mínimo da curva cai fora de 12–16 em todas as categorias |
| H3 | `higher-16` empata ou vence `higher-14` no custo total (tamanho + processamento) | `higher-14` vence por margem > 5% em tamanho **e** não perde em processamento |
| H4 | Operações no domínio de símbolos (busca, contagem, igualdade, hash) ficam mais rápidas que em bytes na proporção de N_8/N_b, descontado o custo de encode, para dados lidos muitas vezes | speedup < 1 após amortizar o encode em ≥ 10 leituras |
| H5 | LOWER (1–7) só vence `base-8` em alfabetos restritos (DNA, dígitos, enums) | LOWER vence em texto geral **ou** perde até em alfabetos restritos |
| H6 | M4 adaptativo vale só em streams longos (≥ 1 MB) e repetitivos, com overhead de sincronização < 5% | vale em streams curtos, **ou** nunca fica < 5% |
| H7 | Higher + zstd empilhados produzem saída menor que zstd sozinho | zstd sozinho é menor ou igual em todas as categorias |

Critérios de decisão finais: `02-BENCHMARK.md §7`.
