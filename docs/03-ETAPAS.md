# 03 — Etapas de construção

Regra: cada etapa termina com os **critérios de aceite** verificados e um
resumo em `results/ETAPA-XX.md` (o que foi feito, números medidos e desvios
da especificação). Só então abre-se a próxima.

---

## E0 — Fundação do repositório `bit002`

- Criar o repositório `bit002` no GitHub (privado até a E10). Nenhum arquivo vem do rascunho `bit001`.

- Workspace Cargo (`higher-core`, `bitbench`, `higher-wasm`), `web/`, `worker/`, `corpora/`, `tables/`, `results/`, `analysis/`.
- `rust-toolchain.toml` fixo, `clippy` + `rustfmt`, CI no GitHub Actions (test + clippy + build wasm).
- `.gitignore`: `results/*` (exceto `results/reference/`), `corpora/real/data/`, `target/`, `node_modules/`, `.wrangler/`.
- `README.md` curto, apontando para `CLAUDE.md` e `docs/`.

**Aceite:** `cargo test --workspace` verde (mesmo vazio). CI verde. Estrutura igual ao `CLAUDE.md`.

## E1 — Packing de bits 1..32

- `BitWriter`/`BitReader` MSB-first, acumulador u64, para qualquer `1 ≤ b ≤ 32`.
- Caminhos rápidos para `b ∈ {8,16,32}`.
- Testes com proptest: roundtrip de sequências aleatórias de valores `< 2^b` para cada `b`.
  Casos de borda: vazio, 1 símbolo, valores `0` e `2^b−1`, comprimentos que não fecham byte.

**Aceite:** 32 larguras com roundtrip em ≥ 10⁵ casos aleatórios cada. Microbench criterion de pack/unpack por `b`, salvo como baseline.

## E2 — Modelo paramétrico e formato `.hgr`

- `Model { b, variant, table }` conforme `01 §1–3`.
- Tabela M2 mínima: candidatos n-grama 2..16 + palavras, ranking por `ganho(s)`, sem re-pontuação ainda.
- Tokenizer P-greedy com trie.
- LOWER com escape E1. BASE identidade. HIGHER com legado `0..255`.
- Formato `.hgr` com cabeçalho de 36 bytes e crc32 (`01 §8`). Serialização de tabela com `table_id` = sha256.

**Aceite:** roundtrip em todo `b` 1..32 sobre fuzz de bytes arbitrários (incluindo os 256 valores). Decoder rejeita `table_id` errado e crc inválido com erro tipado, sem panic.

## E3 — Corpora

- Geradores sintéticos com seed (`02 §2.2`).
- `corpora/real/manifest.json` + script de download com verificação sha256.
- Divisão treino/validação/teste por bloco contíguo (`02 §2.3`).

**Aceite:** `make corpora` reproduz tudo e confere os hashes. Os sintéticos são idênticos byte a byte entre execuções com a mesma seed.

## E4 — Sweep de armazenamento e transmissão + baselines

- `bitbench sweep --bits 1..32 --corpus … --sizes … --seed … --out …`.
- Métricas de `02 §3–4`. Baselines gzip/zstd/brotli/lz4 via crates Rust. BPE (`tokenizers` ou implementação própria) com vocab `2^b` para `b` 9..16.
- Modo empilhado `higher-b:M2 → zstd`.
- Saída JSON por linha (`02 §6.1`) + CSV agregado.

**Aceite:** a sweep 1..32 roda completa em todos os corpora de teste. Nenhuma linha com `ok = false`. O CSV abre no pandas sem erro. Primeiros gráficos 1 e 2 de `02 §8` gerados, marcados `[MEDIDO]`.

## E5 — Processamento (núcleo da tese)

- Benchmarks criterion de encode/decode por `b`.
- Wrapper `perf stat` (cycles, instructions, L1d/LLC misses, branch-misses) e RAPL se houver.
- `higher-core::ops` (`01 §10`) em três representações: bytes, símbolos alinhados, símbolos empacotados.
- `random_access` vs zstd.
- Cálculo de `amortization_reads` (H4).

**Aceite:** gráficos 3, 5 e 6 de `02 §8`. Tabela resumo por `b` com `dec_MBps`, `cycles/byte` e `op_speedup` para as 6 ops. Protocolo de `02 §6` documentado no relatório da etapa, incluindo a máquina.

## E6 — Tabelas: qualidade e variantes

- Re-pontuação iterativa (`01 §4.3`) e métrica `slots_useful`.
- M1 com camada Tipo 1 (blocos Unicode frequentes em PT/EN, operadores e pontuação).
- Parser P-optimal (DP) para `b ∈ {8,12,14,16,20,24}`.
- Três tabelas pré-acordadas por largura de interesse (`01 §2`). Matriz cruzada treino × teste.

**Aceite:** gráficos 4 e 8. Relatório com o ganho de M2 iterativo vs M2 ingênuo vs M1 e o gap greedy vs optimal.

## E7 — M4 adaptativo sincronizado

- Protocolo de `01 §9`: deltas com `seq`, LRU/TTL, limite `A`, checkpoints com hash.
- Códigos adaptativos em `b` bits (válido até 32).

**Aceite:** `overhead_sync` medido por tamanho de stream (10 KB…10 MB). Verificação de H6. Roundtrip com deltas em fuzz.

## E8 — Robustez

- Sync markers a cada `K` símbolos (flag no cabeçalho).
- Experimento de flip de bit (`02 §4`), com e sem markers, vs `base-8` e zstd.

**Aceite:** gráfico 9. Custo de tamanho dos markers reportado.

## E9 — UI, WASM e Cloudflare (`04-UI-DEPLOY.md`)

- `higher-wasm` expondo `encode/decode/sweep` para o browser.
- UI vertical com action button, version badge e gráficos.
- Worker + D1 para persistir runs (nativos importados + wasm).

**Aceite:** deploy do Worker `bit002` em `bit002.daniel-queiroga.workers.dev` com o D1 `bit002`. Sweep 1..32 roda no browser de um celular (9:16) e grava no D1. Os resultados nativos importados aparecem separados dos wasm.

## E10 — Relatório de validação

- `analysis/report.py` gera `results/REPORT.md` com os 9 gráficos e a avaliação de H1–H7 (`00 §6`): **sustentada / refutada / inconclusiva**, com números e intervalos.
- Seção "o que mudaria a conclusão" (limitações e próximos experimentos).

**Aceite:** relatório reproduzível do zero com `make report` a partir de commit e seed.

## E11 (opcional) — Camada de glifos

Gerador de glifo por símbolo (`01 §11`) e visualização na UI. Não entra nas métricas.

---

## Prompts prontos para o Claude Code

**Abertura (primeira sessão):**

> Leia `CLAUDE.md` e todos os arquivos de `docs/`. Resuma em até 15 linhas o
> que entendeu da tese e das hipóteses H1–H7, e liste ambiguidades ou
> contradições que encontrar na especificação antes de escrever código. Depois
> execute a Etapa E0.

**Por etapa:**

> Execute a etapa E<N> de `docs/03-ETAPAS.md`. Siga `docs/01` e `docs/02` à
> risca. Ao terminar, verifique cada critério de aceite rodando os comandos,
> escreva `results/ETAPA-<N>.md` com números medidos (rótulo `[MEDIDO]`) e
> desvios, e pare para revisão.

**Revisão crítica (a cada 2–3 etapas):**

> Revise o código e os resultados até aqui como um revisor cético. Procure
> vazamento treino/teste, custos não contabilizados (tabela, cabeçalho,
> sync), medições de tempo contaminadas e resultados que contradigam
> `00-TESE.md §3`. Liste os achados por severidade antes de corrigir.
