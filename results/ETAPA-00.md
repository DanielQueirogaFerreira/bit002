# ETAPA E0 — Fundação do repositório `bit002`

Fechamento da etapa E0 de [`docs/03-ETAPAS.md`](../docs/03-ETAPAS.md).

- Commit da etapa: `1a8c6d2`
- Data: 2026-09-18
- Toolchain: `rustc 1.94.1 (e408947bf 2026-03-25)`, fixada em `rust-toolchain.toml`

Nenhum número de compressão ou de tempo aparece aqui, porque o codec começa
na E1. O que esta etapa entrega é estrutura.

---

## 1. O que foi feito

| Item | Onde |
|---|---|
| Especificação versionada (CLAUDE.md + 6 docs) | `CLAUDE.md`, `docs/00`…`docs/05` |
| Workspace Cargo com os três crates | `Cargo.toml`, `crates/{higher-core,bitbench,higher-wasm}` |
| Toolchain fixa + `clippy` + `rustfmt` + target wasm32 | `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` |
| CI GitHub Actions: fmt, clippy, test, build wasm, script da UI | `.github/workflows/ci.yml` |
| `.gitignore` conforme E0 | `.gitignore` |
| `README.md` curto apontando para `CLAUDE.md` e `docs/` | `README.md` |
| Estrutura de diretórios do `CLAUDE.md` | `corpora/{gen,real}`, `tables/`, `results/`, `web/`, `worker/`, `analysis/` |
| Esqueleto do Worker + migração inicial do D1 | `worker/wrangler.toml`, `worker/migrations/0001_init.sql`, `worker/src/worker.js` |
| `scripts/gen-version.mjs` (version badge, `docs/04 §3`) | `scripts/gen-version.mjs`, `package.json` |
| `Makefile` com os alvos das etapas | `Makefile` |

### Conteúdo de código na E0

`higher-core` recebeu só o **vocabulário** do projeto, em `model.rs`:
[`Width`] (1..=32, validada), [`Band`] (Lower/Base/Higher), [`Variant`]
(M0/M1/M2/M4 com o byte do cabeçalho `.hgr`) e os derivados de
`docs/00 §3.3`: capacidade `2^b`, contêiner de RAM, grupo de packing
`lcm(b,8)/b` e o limiar (`1 − 8/b` em HIGHER, `b/8` em LOWER).

Isso é fundação, não codec: são as convenções de `CLAUDE.md` e a aritmética
de `docs/00 §3` escritas uma vez, em Rust, com teste — em vez de repetidas
em cada crate e na UI. `BitWriter`/`BitReader` são a E1; `Model` e `.hgr`
são a E2.

`bitbench` sabe `version` e `models`. `sweep` sai com código 2 dizendo que é
a E4. `make corpora`, `make bench` e `make report` fazem o mesmo para E3, E5
e E10. A escolha é deliberada: um comando que roda e imprime zeros vira
"resultado" no relatório seguinte, que é o defeito D9 de `docs/05 §3`.

---

## 2. Critérios de aceite

> **Aceite (E0):** `cargo test --workspace` verde (mesmo vazio). CI verde.
> Estrutura igual ao `CLAUDE.md`.

### 2.1 `cargo test --workspace` — verde [MEDIDO]

```
$ cargo test --workspace --all-targets
test result: ok. 0 passed;  0 failed  (bitbench, sem testes)
test result: ok. 11 passed; 0 failed  (higher-core)
test result: ok. 1 passed;  0 failed  (higher-wasm)
```

12 testes, 0 falhas. Os 11 de `higher-core` checam a convenção de nomes, a
recusa de largura fora de 1..=32, o corte de faixa em 8, as capacidades, os
contêineres, as larguras nativas, os grupos de packing contra a coluna de
`docs/00 §3.3`, o fechamento em byte de todo grupo, os limiares e a ausência
de `M3` no fio.

### 2.2 Lint — verde [MEDIDO]

```
$ cargo fmt --all --check     # sem diferenças
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile
```

Com `clippy::pedantic` ligado nos três crates e `missing_docs` em
`higher-core`.

### 2.3 Build WASM — verde [MEDIDO]

```
$ cargo build -p higher-wasm --target wasm32-unknown-unknown --release
    Finished `release` profile [optimized]
$ ls -l target/wasm32-unknown-unknown/release/higher_wasm.wasm
43375 bytes
```

### 2.4 CI — verde [MEDIDO]

Run [#1](https://github.com/DanielQueirogaFerreira/bit002/actions/runs/35320721817)
no commit `1a8c6d2`, branch `claude/serene-johnson-xt84l2`. Os dois jobs
concluíram com sucesso, todos os passos verdes:

| Job | Passo | Conclusão |
|---|---|---|
| `test + clippy + wasm` | toolchain de `rust-toolchain.toml` | success |
| | Formatação (`cargo fmt --all --check`) | success |
| | Clippy (`-D warnings`) | success |
| | Testes (`cargo test --workspace --all-targets`) | success |
| | Build WASM (`wasm32-unknown-unknown --release`) | success |
| `scripts da UI` | `node scripts/gen-version.mjs` | success |
| | campos de `docs/04 §3` em `web/version.json` | success |

Duração total: ~38 s. A toolchain fixa de `rust-toolchain.toml` foi
resolvida pelo runner sem precisar de `dtolnay/rust-toolchain`, o que
mantém uma única fonte da verdade para a versão do compilador.

### 2.5 Estrutura igual ao `CLAUDE.md` — conferida

Todos os diretórios da árvore de `CLAUDE.md` existem. Os que ainda não têm
conteúdo carregam um `README.md` dizendo em que etapa eles se preenchem, em
vez de um `.gitkeep` mudo.

---

## 3. Desvios da especificação

| # | Desvio | Motivo |
|---|---|---|
| V1 | `.gitignore` abre exceção para `results/ETAPA-*.md` e `results/REPORT.md`, além de `results/reference/` | A regra da E0 (`results/*` exceto `results/reference/`) ignoraria justamente os relatórios de etapa que `docs/03` manda escrever em `results/`. Sem a exceção, este arquivo não seria versionável. |
| V2 | O `schema.sql` de `docs/04 §4.1` foi criado como `worker/migrations/0001_init.sql`, e não como `worker/schema.sql` | O próprio `docs/04 §4.1` exige que mudança de esquema só entre por migração numerada. Manter os dois arquivos criaria duas fontes da verdade que divergem na primeira alteração. `worker/README.md` registra a decisão. |
| V3 | `[profile.release]` não usa `panic = "abort"` | Foi considerado (menos landing pads na medição), mas quebraria `cargo test --release` e futuros fuzz/proptest em release. `lto = "fat"` e `codegen-units = 1` ficam, que é o que remove variação de layout entre runs. |
| V4 | `worker/wrangler.toml` tem `database_id = ""` | Só é preenchível depois de `wrangler d1 create bit002`, que é a E9. Deixar em branco falha o deploy de forma explícita; um id inventado falharia silenciosamente contra o banco errado. |
| V5 | O crate `higher-core` já tem código (o módulo `model`) | O aceite permite workspace vazio. Preferi consolidar as convenções de nomes e a aritmética de limiar de `docs/00 §3` em um lugar testado, antes que elas fossem reescritas à mão em `bitbench`, na UI e no Worker. Nada de codec entrou. |

---

## 4. Ambiguidades e contradições encontradas na especificação

Levantadas antes de escrever código, como pede o prompt de abertura de
`docs/03`. Nenhuma bloqueia a E1; as marcadas **(decidir)** precisam de
resposta antes da etapa indicada.

1. **`M3` não existe como variante no fio.** `docs/01 §2` lista M0–M4, mas
   `§8` define `variant` como `0=M0 1=M1 2=M2 4=M4` — sem `3`. O próprio `§2`
   resolve dizendo que M3 é `higher-16:M2`. Implementado assim: `Variant`
   não tem `M3` e `from_wire(3)` devolve `None`.
2. **Nome de run da régua.** `docs/01 §2` dá o exemplo `base-8`, sem sufixo,
   mas `M0` é uma variante. Adotado: `base-8` sozinho quando a variante é
   `M0`; qualquer outra combinação leva sufixo. Isso torna `base-8:M2` um
   nome possível, e a E2 precisa decidir se ele é legítimo (tabela de blocos
   com b=8, sem espaço para legado) ou erro. **(decidir na E2)**
3. **Limiar do LOWER com blocos multibyte.** `docs/00 §3.2` deriva
   `c > b/8` "considerando só símbolos de 1 byte", e logo adiante permite
   blocos multibyte, que mudam o limiar. A coluna "cobertura sem escape" de
   `§3.3` vale, então, só no caso de 1 byte. O valor exposto por
   `Width::threshold()` é esse, e está documentado como tal. A E4 precisa
   reportar a cobertura medida junto, senão a comparação com o limiar engana.
4. **`slots_uteis` vs `slots_useful`.** `docs/01 §4.4` escreve em português,
   `docs/02 §3` e o JSON de `§6.1` em inglês. Adotado o inglês
   (`slots_useful`) nos campos serializados, porque é o que o esquema do D1
   em `docs/04 §4.1` usa. **(fixar na E4)**
5. **Tamanho do cabeçalho vs `crc32_payload`.** `docs/01 §8` diz cabeçalho
   fixo de 36 bytes e lista `crc32_payload` em offset 32 (4 bytes), fechando
   em 36 — consistente. Mas `flags.bit1` (sync markers) e o bloco adaptativo
   ficam **fora** dos 36 bytes, então `header_bytes = 36` em `docs/02 §6.1`
   subestima o overhead real em M4 e com markers ligados. A E7/E8 precisam
   reportar esses bytes em campo separado, ou `overhead_header` fica errado.
   **(decidir na E7)**
6. **`base-8` "sem packing" vs `bits_per_byte` incluindo cabeçalho.**
   `docs/01 §1` diz que `b=8` é identidade sem cabeçalho de packing, mas
   `docs/02 §3` define `encoded_bytes = payload + cabeçalho` para todos.
   Se a régua também pagar 36 bytes, ela tem `b/B > 8` em arquivos pequenos,
   e "ganhar de `base-8`" fica mais fácil do que deveria. Proposta: a régua é
   o arquivo cru, **sem** cabeçalho, e o `.hgr` de `b=8` é reportado à parte.
   **(decidir na E4, antes de qualquer número de H1)**
7. **`A` padrão do namespace adaptativo.** `docs/01 §3` dá `A = C/16`, que em
   `b=14` são 1024 — bate com o layout herdado `0x3C00–0x3FFF`. Mas em `b=32`
   isso são 268 milhões de slots adaptativos, o que não é implementável como
   tabela densa. `A` precisa de um teto absoluto. **(decidir na E7)**
8. **Tamanho dos corpora vs custo da tabela.** `docs/02 §1` manda rodar
   10 KB…10 MB, e `docs/05 §2` já observa que uma tabela de 14 bits são ~28 KB.
   Em 10 KB, qualquer `b ≥ 12` perde por definição se a tabela contar no
   `encoded_bytes`. Isso não é contradição, é resultado esperado — mas o
   relatório precisa separar "tabela transmitida" de "tabela pré-acordada,
   amortizada em N arquivos" (`breakeven_files`, `docs/02 §3`), senão a curva
   por tamanho fica ilegível.
9. **`--corpus all` na E4 inclui corpora reais que a E3 baixa.** O comando de
   exemplo em `CLAUDE.md` roda `--corpus all` com dados que estão fora do git.
   A E4 precisa falhar com mensagem clara quando `corpora/real/data/` estiver
   vazio, em vez de rodar só nos sintéticos e chamar o resultado de "all" —
   isso seria o defeito D2 de `docs/05 §3` reaparecendo por acidente.
10. **Medição em container.** `docs/02 §6` pede máquina dedicada, governor
    `performance` e `taskset`. Um runner de CI ou um container compartilhado
    não oferece nada disso. Os números de processamento da E5 têm de sair de
    uma máquina registrada no relatório, e a CI serve para verificar
    correção, nunca para produzir `enc_ns_per_byte`. **(vale a partir da E5)**

---

## 5. Próxima etapa

**E1 — Packing de bits 1..32.** Sem pré-requisito pendente: os quatro
critérios de aceite da E0 estão verificados (§2).
