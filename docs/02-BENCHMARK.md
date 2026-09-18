# 02 — Benchmark: armazenamento, transmissão e processamento

## 1. Matriz do experimento

```
modelos    = base-8, lower-1..7, higher-9..32         (32 larguras)
variantes  = M1, M2 (todas as larguras); M4 (b ∈ {12,14,16,20,24})
parsers    = P-greedy (todas); P-optimal (b ∈ {8,12,14,16,20,24})
corpora    = §2
tamanhos   = 10 KB, 100 KB, 1 MB, 10 MB
baselines  = gzip -6, zstd -3/-19, brotli -5/-11, lz4, BPE (vocab = 2^b para b 9..16)
empilhado  = higher-b:M2 → zstd -3   (hipótese H7)
seeds      = 5 por configuração sintética
```

Rode a sweep completa `--bits 1..32`, número por número, sem pular.

## 2. Corpora

### 2.1 Reais (resultados principais)

| Categoria | Fonte sugerida | Observação |
|---|---|---|
| Texto EN | enwik8 (primeiros 10 MB) | referência clássica |
| Texto PT | dump da Wikipédia PT (amostra) | idioma do projeto |
| Código | Silesia `samba`/`mozilla` ou repo público misto | |
| Logs | Loghub (Apache, HDFS, Linux) | alta repetição |
| Estruturado | JSON/CSV de dados abertos | chaves repetidas |
| Binário | executáveis (Silesia `ooffice`, `x-ray`) | espera-se perda |
| Já comprimido | PNG/JPEG/MP3 | controle negativo: espera-se perda em todo `b` |
| Alfabeto restrito | DNA (Silesia não tem; usar genoma público FASTA), dígitos | onde os LOWER devem ganhar |
| Silesia completo | 12 arquivos | comparabilidade com a literatura |

Os dados reais ficam fora do git. `corpora/real/manifest.json` guarda a URL,
o sha256 e a licença, e `make corpora` baixa e confere tudo.

### 2.2 Sintéticos (isolar variáveis)

Geradores com seed que controlam **uma variável por vez**:

- tamanho do alfabeto (2, 4, 16, 64, 256);
- entropia de ordem 0 (bits/byte alvo);
- taxa de repetição de blocos e comprimento médio dos blocos;
- distribuição de Zipf do vocabulário (expoente variável).

Sintéticos servem para explicar curvas, não para sustentar a tese.

### 2.3 Treino e teste

Cada categoria real é dividida em **treino (50%) / validação (10%) / teste (40%)**
por arquivo ou bloco contíguo, nunca por amostragem de linhas. As tabelas são
construídas só no treino. Também rode a matriz **cruzada** (tabela treinada
em A, testada em B) para medir generalização. Esse é o cenário real de tabela
pré-acordada.

## 3. Métricas — armazenamento

| Métrica | Fórmula |
|---|---|
| `encoded_bytes` | payload + cabeçalho |
| `bits_per_byte` (b/B) | `encoded_bytes·8 / input_bytes`. **Métrica principal de tamanho** |
| `ratio` | `input_bytes / encoded_bytes` |
| `n_symbols`, `horiz_reduction` | `1 − n_symbols / input_bytes` |
| `escape_rate` (LOWER) | escapes / símbolos |
| `table_bytes` | tamanho serializado da tabela |
| `slots_used` / `slots_useful` | entradas na tabela / entradas usadas no teste |
| `breakeven_files` | nº de arquivos do mesmo tipo a partir do qual `table_bytes` se paga (se a tabela for distribuída uma vez) |

## 4. Métricas — transmissão

| Métrica | Fórmula / método |
|---|---|
| `wire_bytes` | `encoded_bytes` + deltas (M4) |
| `overhead_header` | 36 B / wire_bytes |
| `overhead_sync` (M4) | bits de delta / bits totais |
| `t_transfer` simulado | `wire_bytes / banda + RTT`, bandas {1 Mbps, 10 Mbps, 100 Mbps, 1 Gbps}, RTT {5, 50, 200 ms} |
| `t_end_to_end` | encode + transferência + decode. Mostra onde um codec lento anula o ganho de bytes |
| `error_propagation` | flip de 1 bit aleatório (1.000 trials) → bytes corrompidos após decode, com e sem sync markers |

## 5. Métricas — processamento (foco)

Tudo em Rust release (`-C target-cpu=native` **e** genérico, reportar os dois).

| Métrica | Método |
|---|---|
| `enc_ns_per_byte`, `dec_ns_per_byte` | criterion, mediana + IC95% |
| `enc_MBps`, `dec_MBps` | derivado |
| `cycles/byte`, `instructions/byte`, `IPC` | `perf stat` |
| `cache_misses/KB` (L1d, LLC) | `perf stat`, relacionar com `table_bytes` vs tamanho de cache |
| `branch_misses/KB` | `perf stat` |
| `peak_rss` | `/usr/bin/time -v` ou contador interno |
| `energy_J`, `J/MB` | RAPL (`perf stat -e power/energy-pkg/`) quando disponível; senão marcar `N/D`, nunca estimar sem rótulo |
| `op_speedup[op]` | tempo da op em bytes / tempo em símbolos, para cada op de `01 §10`, nas três representações |
| `amortization_reads` | nº de leituras em que `Σ op_speedup` paga o custo do encode (H4) |
| `random_access_ns` | i-ésimo elemento: Higher O(1) vs zstd (descomprime até i) |

## 6. Protocolo de medição

1. Máquina dedicada. Registrar CPU, microcódigo, RAM, kernel e governor (`performance`).
2. Fixar o processo em um núcleo (`taskset`), com turbo registrado (ligado/desligado).
3. Warmup mais ≥ 10 repetições. Reportar mediana, p5/p95 e IC95%.
4. Verificar o roundtrip em **todo** run. Run com `ok = false` invalida a linha.
5. Seed fixa e registrada. O mesmo input vale para todos os modelos do mesmo run.
6. Gravar o JSON bruto por run em `results/<timestamp>-<commit>/`. Os relatórios são derivados.
7. Timing em WASM/browser é **secundário** e rotulado `env=wasm`. Não misturar com o nativo.

### 6.1 Esquema de resultado (uma linha por run)

```json
{
  "run_id": "…", "commit": "…", "timestamp_ms": 0, "machine_id": "…", "env": "native|wasm",
  "model": "higher-14", "b": 14, "variant": "M2", "parser": "greedy",
  "table_id": "…", "table_train_corpus": "logs-apache:train",
  "corpus": "logs-apache:test", "corpus_sha256": "…", "input_bytes": 0, "seed": 42,
  "encoded_bytes": 0, "header_bytes": 36, "table_bytes": 0, "n_symbols": 0,
  "bits_per_byte": 0.0, "ratio": 0.0, "horiz_reduction": 0.0, "escape_rate": 0.0,
  "slots_used": 0, "slots_useful": 0, "overhead_sync": 0.0,
  "enc_ns_per_byte": 0.0, "dec_ns_per_byte": 0.0, "cycles_per_byte": 0.0,
  "ipc": 0.0, "l1d_miss_per_kb": 0.0, "llc_miss_per_kb": 0.0, "peak_rss_kb": 0,
  "energy_j": null, "op_speedup": {"find": 0.0, "histogram": 0.0},
  "ok": true
}
```

## 7. Critérios de decisão

Herdados da especificação M0–M4 e endurecidos:

1. **Ganho líquido positivo** (`bits_per_byte < 8` **incluindo** cabeçalho e
   tabela amortizada) em ≥ 3 categorias **reais**, no conjunto de **teste**.
2. **Throughput de decode ≥ 0,8× `base-8` + memcpy** (não pode ser proibitivo).
3. **`overhead_sync` < 5%** para M4 ser mantido.
4. **Robustez a erro** comparável a `base-8` com sync markers ligados.
5. **Implementável** em hardware/software comum, sem hardware dedicado.
6. **Posição vs baselines:** reportar onde fica cada `b` na fronteira de
   Pareto (tamanho × velocidade de decode × processamento de ops). A tese se
   sustenta onde Higher fica **fora da região dominada** por zstd/lz4/BPE.
   Isso é mais provável em processamento no domínio de símbolos e em acesso
   aleatório do que em razão de compressão.

Regras de desempate: se `higher-16` empatar com `higher-14` (diferença < 2%
em b/B), escolher 16 pela simplicidade. Se 14 vencer por margem clara,
documentar o packing e seu custo. Se M4 não se pagar, restringir a streams
longos e repetitivos.

## 8. Relatórios e gráficos obrigatórios

1. `b/B × b` (1..32) por corpus, com linha em 8 e bandas das baselines.
2. `horiz_reduction × b` com a curva do limiar `1 − 8/b` sobreposta. O ganho
   existe onde a medida fica acima da curva.
3. `dec_MBps × b` e `cycles/byte × b`, com as larguras nativas (8, 16, 32) destacadas.
4. `slots_useful / 2^b × b`: onde a capacidade deixa de ser aproveitada.
5. `llc_miss_per_kb × table_bytes`: onde a tabela sai do cache.
6. `op_speedup × b` por operação.
7. Fronteira de Pareto: b/B × dec_MBps, com todos os modelos e baselines.
8. Matriz cruzada treino × teste (heatmap de b/B).
9. Propagação de erro com e sem sync markers.

Cada gráfico traz no rodapé o commit, a máquina, a data e se os dados são `[MEDIDO]`.
