# CLAUDE.md — bit002 (Arquitetura Higher)

> Arquivo-raiz para o Claude Code. Leia inteiro antes de qualquer etapa.
> Detalhes em `docs/`. Em conflito, a ordem de autoridade é:
> `docs/00-TESE.md` > `docs/01-ESPEC-MODELOS.md` > `docs/02-BENCHMARK.md` > demais.

## Identidade do projeto

| Item                         | Valor                                                |
| ---------------------------- | ---------------------------------------------------- |
| Repositório GitHub           | `bit002`                                             |
| Worker Cloudflare            | `bit002` (URL: `bit002.daniel-queiroga.workers.dev`) |
| Banco D1                     | `bit002` (binding `DB`)                              |
| Pacote npm / workspace Cargo | `bit002`                                             |
| Versão inicial               | `0.1.0`                                              |

`bit001` foi o rascunho gerado na fase de ideação no DeepSeek. Ele **não é
base de código** deste repositório: nada é copiado dele. Serve só como
referência histórica (ver `docs/05-HERANCA-DEEPSEEK.md`). Não use o nome
`bit001` em código, configuração ou recursos da Cloudflare.

## O que é este projeto

Laboratório de estudo e validação da **Arquitetura Higher**: testar se aumentar
**verticalmente** a quantidade de símbolos disponíveis (bits por símbolo `b`)
reduz **horizontalmente** o número de símbolos do fluxo o suficiente para
gerar ganho líquido em **armazenamento**, **transmissão** e **principalmente
processamento**.

Faixa completa, um modelo por inteiro de 1 a 32:

| Faixa  | Nomes                    | Papel                                   |
| ------ | ------------------------ | --------------------------------------- |
| LOWER  | `lower-7` … `lower-1`    | inverso da tese: menos bits por símbolo |
| BASE   | `base-8`                 | régua absoluta (byte a byte)            |
| HIGHER | `higher-9` … `higher-32` | a tese: mais bits por símbolo           |

## Postura científica (obrigatória)

1. **Tese é hipótese, não dogma.** O objetivo é medir, não confirmar. Um
   resultado negativo bem medido é um entregável válido.
2. **Nunca relatar previsão como resultado.** Tabelas de "comportamento
   esperado" ficam marcadas `[PREVISÃO]`; números medidos ficam `[MEDIDO]`
   com hash do commit, máquina e seed.
3. **Treino ≠ teste.** Tabelas pré-acordadas são construídas em corpus de
   treino e avaliadas em corpus de teste disjunto. Sem exceção.
4. **Todo custo entra na conta:** tabela/dicionário (memória e, se transmitido,
   bytes), cabeçalho, packing, sincronização (M4), escapes.
5. **Baselines reais sempre presentes:** `base-8` cru, gzip, zstd, brotli, lz4.
   Ganho só contra `base-8` cru não prova a tese.
6. Linguagem probabilística nos relatórios ("tende a", "nesta amostra"),
   nunca "sempre"/"prova".

## Stack (decisão inicial — revisável na Etapa 0)

| Camada                  | Tecnologia                                                                               | Motivo                                                                         |
| ----------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Núcleo codec + ops      | **Rust** (`crates/higher-core`)                                                          | medir processamento de verdade; Python mede o interpretador, não a arquitetura |
| CLI de bench            | Rust (`crates/bitbench`) + `criterion`                                                   | repetibilidade, warmup, estatística                                            |
| Contadores HW / energia | `perf stat` (cycles, instructions, cache-misses, branch-misses) + RAPL quando disponível | processamento e energia                                                        |
| Browser / UI            | mesmo núcleo compilado para **WASM**                                                     | um só código para CLI, UI e Worker                                             |
| Persistência            | Cloudflare Workers + **D1**                                                              | histórico de runs                                                              |
| Análise                 | Python (pandas/matplotlib) só para relatórios offline                                    | nunca para medir tempo                                                         |

## Estrutura alvo do repositório

```
bit002/
├─ CLAUDE.md
├─ docs/                  # estas especificações
├─ crates/
│  ├─ higher-core/        # BitWriter/BitReader, tabelas, encoder/decoder, ops no domínio de símbolos
│  ├─ bitbench/           # CLI: sweep 1..32, corpora, métricas, export JSON/CSV
│  └─ higher-wasm/        # bindings wasm-bindgen para UI/Worker
├─ corpora/
│  ├─ gen/                # geradores sintéticos com seed
│  └─ real/               # scripts de download + manifest com SHA-256 (dados fora do git)
├─ tables/                # tabelas treinadas versionadas (hash + corpus de treino)
├─ results/               # runs JSON/CSV (fora do git, exceto snapshots de referência)
├─ web/                   # UI (HTML/JS puro + WASM)
├─ worker/                # Cloudflare Worker + schema.sql (D1) + wrangler.toml
└─ analysis/              # notebooks/scripts de relatório
```

## Convenções

- Nomes de modelo exatamente: `lower-N`, `base-8`, `higher-N`. Variantes de
  tabela: sufixo `:M1`, `:M2`, `:M4` (ex.: `higher-14:M2`). Ver `docs/01`.
- Bit order: **MSB-first**, big-endian em todos os cabeçalhos.
- Todo run grava: `commit`, `model`, `b`, `variant`, `corpus`, `corpus_sha256`,
  `table_sha256`, `seed`, `machine_id`, `timestamp_ms`.
- Testes: roundtrip obrigatório (`decode(encode(x)) == x`) para todo `b` 1..32,
  incluindo entrada vazia, 1 byte, todos os 256 bytes, e fuzz (`cargo fuzz` ou proptest).
- Unidades de trabalho chamam-se **etapas** (ver `docs/03-ETAPAS.md`). Feche uma
  etapa com critérios de aceite verificados antes de abrir a próxima.
- Commits pequenos, mensagem em português, prefixo da etapa: `[E2] packing genérico 1..32`.

## Comandos (a criar na Etapa 0/1)

```bash
cargo test --workspace                     # roundtrip + unidade
cargo run -p bitbench -- sweep --bits 1..32 --corpus all --seed 42 --out results/
cargo run -p bitbench -- sweep --bits 8,12,14,16 --corpus text,logs --baselines
cargo bench -p higher-core                 # criterion
perf stat -e cycles,instructions,cache-misses,branch-misses target/release/bitbench ...
```

## Documentos

- `docs/00-TESE.md` — o core da proposta, matemática do limiar 1..32, hipóteses falseáveis.
- `docs/01-ESPEC-MODELOS.md` — especificação dos modelos, tabelas, escape, packing, formato binário.
- `docs/02-BENCHMARK.md` — corpora, métricas de armazenamento/transmissão/processamento, protocolo de medição, critérios de decisão.
- `docs/03-ETAPAS.md` — roadmap em etapas com critérios de aceite e prompts prontos.
- `docs/04-UI-DEPLOY.md` — UI vertical, action button, version badge, Worker + D1.
- `docs/05-HERANCA-DEEPSEEK.md` — o que vem da fase de ideação no DeepSeek e defeitos conhecidos a não repetir.
