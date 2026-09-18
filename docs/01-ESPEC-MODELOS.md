# 01 — Especificação dos modelos

## 1. Modelo paramétrico único

Um só tipo, `Model { b: 1..=32, variant, table }`. Não crie código separado
por largura. Toda largura passa pelo mesmo encoder e decoder, e só os
parâmetros mudam.

| Faixa | Regra da tabela |
|---|---|
| LOWER `b < 8` | código `0` = **ESCAPE** (seguido de 8 bits brutos). Códigos `1..2^b−1` = entradas de maior ganho (bytes frequentes **e**, opcionalmente, blocos multibyte) |
| BASE `b = 8` | identidade: 256 bytes. Sem escape e sem packing |
| HIGHER `b > 8` | códigos `0..255` = os 256 bytes legados (garantem roundtrip sem escape). Códigos `256..2^b−1` = blocos escolhidos por ganho |

## 2. Variantes (herdadas de M0–M4, generalizadas para qualquer `b`)

| Variante | Definição | Sincronização | Objetivo |
|---|---|---|---|
| `M0` | `base-8` | nenhuma | régua |
| `M1` | legado + **Tipo 1** (camada humana: alfabetos Unicode frequentes, operadores, pontuação, dígrafos) + blocos, com a ordem de preenchimento fixada à mão | nenhuma, pré-acordada | ganho da capacidade vertical sem otimização |
| `M2` | legado + blocos escolhidos por **função de ganho** no corpus de treino | nenhuma, pré-acordada | teto do ganho estático |
| `M3` | `M2` com `b = 16`, 1 símbolo = 2 bytes, sem packing | nenhuma | testar se o alinhamento nativo compensa. Na sweep 1..32 isso é simplesmente `higher-16:M2`, e `higher-32:M2` é o análogo em u32 |
| `M4` | tabela base `M2` + **namespace adaptativo** sincronizado | obrigatória | ganho real da adaptação menos o custo de sincronização |

Nome de run: `higher-14:M2`, `lower-6:M2`, `higher-16:M4`, `base-8`.

**Várias tabelas fixas pré-acordadas** (pedido do fundador): cada tabela é um
artefato versionado em `tables/` (`<b>-<variant>-<corpus_treino>-<sha8>.tbl`).
Comece com três por largura de interesse: (a) só legado + Tipo 1, (b) M2
treinada em texto e código, (c) M2 treinada em logs e dados estruturados. O
fluxo declara no cabeçalho qual tabela usa.

## 3. Layout de namespace (generalizado)

Para `b > 8`, com `C = 2^b`:

| Faixa | Conteúdo |
|---|---|
| `[0, 256)` | legado 8 bits (sempre) |
| `[256, 256 + T1)` | Tipo 1 humano (só M1). `T1` configurável; o layout herdado em `b = 14` dá 12.032 slots (~73% de `C`, `0x0100–0x2FFF`). Em M1 o que não for preenchido fica livre para Tipo 2 |
| `[…, C − A)` | Tipo 2 máquina (blocos por ganho) |
| `[C − A, C)` | adaptativo (só M4). `A` é configurável, padrão = `C/16` (em 14 bits dá 1024, herdado de `0x3C00–0x3FFF`) |

Layout de referência herdado para `b = 14`: `0x0000–0x00FF` legado,
`0x0100–0x2FFF` Tipo 1, `0x3000–0x3BFF` Tipo 2, `0x3C00–0x3FFF` adaptativo.

## 4. Construção de tabela (M2)

Função de ganho de um candidato `s` (bloco de bytes):

```
ganho(s) = freq(s) · (8·len(s) − b) − custo_slot(s)
custo_slot = 0 se pré-acordado; = bits para transmitir a entrada se enviado (M4)
```

Algoritmo:

1. Gerar candidatos no corpus de **treino**: n-gramas de bytes 2..`MAX_MATCH`
   (padrão 16), palavras (split em espaço/pontuação) e tokens estruturais
   (chaves JSON, prefixos de log, timestamps normalizados).
2. Ordenar por ganho e inserir até `C − reservados`.
3. **Re-pontuação iterativa** (obrigatória a partir da Etapa 3): candidatos se
   sobrepõem, então a frequência "real" depende do parser. Rode o tokenizer
   no treino, recalcule a frequência efetiva de uso de cada entrada, remova as
   de ganho ≤ 0, preencha com os próximos candidatos e repita até convergir ou
   atingir K iterações. Registre `slots_uteis` (entradas com uso > 0 no teste).
4. Salvar a tabela com metadados: `b`, variante, corpus de treino (sha256),
   nº de entradas, `slots_uteis`, tamanho em bytes.

Limite prático: para `b ≥ 20`, o número de candidatos com ganho positivo
provavelmente fica muito abaixo de `2^b`. Não tente alocar `2^b` entradas em
memória. Use `HashMap`/trie esparsa e reporte a ocupação.

## 5. Tokenização (parser)

- **P-greedy:** maior casamento via trie (Aho-Corasick ou trie de bytes). É o padrão.
- **P-optimal:** programação dinâmica que minimiza o total de bits. É mais caro.
  Serve para medir quanto o greedy deixa na mesa. Rode em `b ∈ {8, 12, 14, 16, 20, 24}`.

## 6. Escape (LOWER)

- **E1 (padrão):** código `0` + 8 bits brutos.
- **E2 (etapa posterior):** múltiplos códigos de escape por classe (ex.: escape
  para dígitos com 4 bits de payload). Só depois que E1 estiver medido.

## 7. Packing de bits

- `BitWriter`/`BitReader` genéricos para `1 ≤ b ≤ 32`, MSB-first, acumulador de 64 bits.
- Caminhos especializados para `b ∈ {8, 16, 32}` (cópia direta, sem shifts) e
  para `b ∈ {12, 14, 24}` (grupos de `lcm(b,8)/b` símbolos). O ganho do caminho
  especializado **é um resultado a reportar**.
- O último byte é completado com zeros. O cabeçalho informa `n_symbols`.

## 8. Formato binário do fluxo (`.hgr`)

```
offset  campo            tipo      nota
0       magic            4 bytes   "HGR1"
4       version          u8        formato = 1
5       b                u8        1..32
6       variant          u8        0=M0 1=M1 2=M2 4=M4
7       flags            u8        bit0 = há bloco adaptativo, bit1 = há sync markers
8       table_id         [u8; 8]   primeiros 8 bytes do sha256 da tabela pré-acordada
16      n_symbols        u64
24      n_input_bytes    u64
32      crc32_payload    u32
36      [bloco adaptativo, se flags.bit0]
        [payload empacotado]
```

Cabeçalho fixo = 36 bytes. Ele **entra** na conta de bytes transmitidos e
armazenados, e também é reportado separado.

**Sync markers (robustez):** a cada `K` símbolos (padrão 4096), alinha ao byte
e grava o offset do símbolo. Isso limita a propagação de erro. Meça com e sem.

## 9. M4 — adaptativo sincronizado

- Base: tabela `M2` pré-acordada. Namespace adaptativo de tamanho `A`.
- **Protocolo:** o emissor mantém uma janela de frequência. Quando
  `ganho(s) > custo_delta(s)`, emite `DELTA{code, len, bytes}` antes do
  primeiro uso. O receptor aplica o delta. Política de evicção LRU, TTL em
  símbolos, limite de expansão `A`.
- Cada delta carrega `seq` e o fluxo tem checkpoint da tabela adaptativa a
  cada `K` símbolos (hash). Divergência significa erro detectável.
- Códigos no bloco adaptativo usam `b` bits (**não** `u16` fixo, pois o
  protótipo anterior quebrava em `b > 16`).
- Métrica obrigatória: `overhead_sync = bits_de_delta / bits_totais`.

## 10. Operações no domínio de símbolos (para medir processamento)

Implementar em `higher-core::ops`, cada uma em duas versões (bytes e
símbolos), com o mesmo resultado lógico:

| Op | Descrição | Notas |
|---|---|---|
| `count_distinct` | nº de símbolos distintos | |
| `histogram` | frequência por símbolo | |
| `find(pattern)` | busca de substring | o padrão é tokenizado com a mesma tabela. **Atenção:** a tokenização depende do contexto e pode não casar nas fronteiras. Implemente o fallback correto e reporte a taxa de fallback |
| `eq` / `hash` | igualdade e hash de registros | |
| `random_access(i)` | i-ésimo símbolo | O(1) na largura fixa. Compare com zstd, que exige descompressão |
| `sort_records` | ordenar linhas por chave | |

Cada op roda em três representações: bytes (`base-8`), símbolos em RAM
alinhada (`u16/u32`) e símbolos empacotados.

## 11. Camada de apresentação (opcional, Etapa 9)

Glifo por símbolo: ID → bitmap (ex.: 128×128 × 1 bit em `b = 14`) ou glifo
procedural. Isso é interface humana, não compressão, e **não entra** nas
métricas de ganho.
