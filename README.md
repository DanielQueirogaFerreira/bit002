# bit002 — Arquitetura Higher

Laboratório de estudo da **Arquitetura Higher**: medir se aumentar
verticalmente os bits por símbolo (`b`, de 1 a 32) reduz horizontalmente o
número de símbolos do fluxo o suficiente para gerar ganho líquido em
armazenamento, transmissão e — principalmente — processamento.

A tese é hipótese, não dogma. Um resultado negativo bem medido é entregável
válido. Previsão nunca é apresentada como medição.

## Por onde começar

- **[`CLAUDE.md`](CLAUDE.md)** — identidade do projeto, postura científica,
  convenções e estrutura. Leia antes de qualquer etapa.
- **[`docs/`](docs/)** — a especificação, em ordem de autoridade:
  - [`00-TESE.md`](docs/00-TESE.md) — a tese, a matemática do limiar 1..32 e as hipóteses H1–H7
  - [`01-ESPEC-MODELOS.md`](docs/01-ESPEC-MODELOS.md) — modelos, tabelas, escape, packing, formato `.hgr`
  - [`02-BENCHMARK.md`](docs/02-BENCHMARK.md) — corpora, métricas, protocolo de medição, critérios de decisão
  - [`03-ETAPAS.md`](docs/03-ETAPAS.md) — roadmap por etapas, com critérios de aceite
  - [`04-UI-DEPLOY.md`](docs/04-UI-DEPLOY.md) — UI, Worker Cloudflare e D1
  - [`05-HERANCA-DEEPSEEK.md`](docs/05-HERANCA-DEEPSEEK.md) — o que veio da ideação e os defeitos a não repetir

## Estado

- **E0 — fundação** concluída ([`results/ETAPA-00.md`](results/ETAPA-00.md)):
  workspace, toolchain fixa, CI e estrutura.
- **E1 — packing de bits 1..32** concluída ([`results/ETAPA-01.md`](results/ETAPA-01.md)):
  `BitWriter`/`BitReader` MSB-first para toda largura de 1 a 32, com caminhos
  especializados e microbench por `b`.
- **E2 — modelo paramétrico e formato `.hgr`** concluída
  ([`results/ETAPA-02.md`](results/ETAPA-02.md)): tabela M2, tokenizador
  P-greedy com trie, escape do LOWER, cabeçalho de 36 bytes com `table_id`
  e crc32.

O codec já roda de ponta a ponta, mas **nenhum número aqui sustenta ou refuta
H1–H7**: não há corpus real (E3) nem baseline alguma — gzip, zstd, brotli,
lz4, BPE (E4). Os tempos do bench de packing são microbench de repositório,
não a medição oficial de processamento, que é a E5 com o protocolo de
[`docs/02 §6`](docs/02-BENCHMARK.md).

## Comandos

```bash
cargo test --workspace                 # testes de unidade e, a partir da E2, roundtrip
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p bitbench -- models        # as 32 larguras com faixa, capacidade e limiar
cargo bench -p higher-core             # microbench de pack/unpack por b
cargo build -p higher-wasm --target wasm32-unknown-unknown --release
```

`make help` lista os alvos disponíveis.

## Licença

MIT. Ver [`LICENSE`](LICENSE).
