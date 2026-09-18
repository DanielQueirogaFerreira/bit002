# ETAPA E1 — Packing de bits 1..32

Fechamento da etapa E1 de [`docs/03-ETAPAS.md`](../docs/03-ETAPAS.md).

- Commit da etapa: `09790e5`
- Data: 2026-09-18
- Toolchain: `rustc 1.94.1 (e408947bf 2026-03-25)`

Esta etapa entrega **packing**, não codec. Não há tabela, tokenizador nem
corpus, e portanto **nenhum número de compressão** existe ainda. Os tempos
abaixo são microbench de repositório, não `enc_ns_per_byte` de
`docs/02 §5` — aquilo é a E5, com o protocolo de `docs/02 §6`.

---

## 1. O que foi feito

`crates/higher-core/src/packing.rs`: `BitWriter` e `BitReader` MSB-first com
acumulador `u64`, atendendo qualquer `b` de 1 a 32 pelo mesmo código. O que
varia é a **estratégia de despacho**, derivada de `b`:

| Estratégia | Larguras | Como |
|---|---|---|
| `native` | 8, 16, 32 (3) | cópia direta de 1, 2 ou 4 bytes, sem shift |
| `group` | 1–7, 10, 12, 14, 20, 24, 28 (13) | grupo de `lcm(b,8)/b` símbolos montado num `u64` |
| `generic` | 9, 11, 13, 15, 17–19, 21–23, 25–27, 29–31 (16) | acumulador `u64`, byte a byte |

O critério de `group` é `lcm(b,8)/b · b ≤ 64`: acima disso o grupo não cabe
num `u64`. `higher-9`, por exemplo, precisaria de 8 símbolos × 9 bits = 72
bits, e por isso cai no genérico.

As três estratégias produzem **exatamente os mesmos bytes**. Isso é
invariante testada em toda largura e em 16 comprimentos diferentes, não
suposição.

Também entram:

- `pack` / `unpack` — funções de conveniência;
- `pack_via_generic` / `unpack_via_generic` — o caminho genérico exposto como
  referência, porque `docs/01 §7` manda **reportar o ganho** do caminho
  especializado e para isso é preciso medir o genérico na mesma largura;
- `Strategy::of(w)` — a estratégia de uma largura, usada pelo bench para
  rotular cada medição.

---

## 2. Critérios de aceite

> **Aceite (E1):** 32 larguras com roundtrip em ≥ 10⁵ casos aleatórios cada.
> Microbench criterion de pack/unpack por `b`, salvo como baseline.

### 2.1 Roundtrip em ≥ 10⁵ casos por largura — verde [MEDIDO]

```
$ cargo test -p higher-core
test result: ok. 28 passed; 0 failed   (unitários, src/packing.rs)
test result: ok.  6 passed; 0 failed   (tests/packing_roundtrip.rs)
```

Dois testes de volume, em `tests/packing_roundtrip.rs`, cobrem o critério:

| Teste | Por largura | Total nas 32 larguras |
|---|---:|---:|
| `volume_100k_casos_por_largura` | 100 000 símbolos num fluxo único | 3 200 000 |
| `volume_em_sequencias_curtas_de_comprimento_sorteado` | ≥ 100 000 símbolos, em sequências de 0 a 64 | ≥ 3 200 000 |

PRNG `SplitMix64` escrito à mão, **seed 42 registrada** (`docs/02 §6.5`):
a mesma seed dá a mesma sequência em qualquer máquina, então um
contraexemplo continua reproduzível na E10.

Mais 4 propriedades com proptest (512 casos cada, largura sorteada junto
com os valores): sequência aleatória, valores escolhidos pelo motor,
escrita desalinhada e recusa de valor largo demais.

### 2.2 Casos de borda pedidos pela etapa — cobertos

| Caso de borda | Teste |
|---|---|
| entrada vazia | `entrada_vazia_produz_fluxo_vazio_em_toda_largura` |
| 1 símbolo | `um_simbolo_nos_extremos_em_toda_largura` |
| valores `0` e `2^b−1` | idem, mais `valores_extremos_alternados_em_toda_largura` |
| comprimentos que não fecham byte | `comprimentos_que_nao_fecham_byte_fazem_roundtrip` (n de 1 a 40, em toda largura) |
| os 256 bytes | `os_256_bytes_fazem_roundtrip_em_toda_largura_que_os_comporta` |
| MSB-first | `msb_first_em_vetores_conhecidos` (vetores literais) |
| completamento com zeros | `ultimo_byte_e_completado_com_zeros` |

### 2.3 Os testes foram verificados por mutação

Um teste que passa à toa é pior que nenhum. Injetei um `word ^= 1` no
caminho de grupo e confirmei que a suíte **falha**:

| Mutação | Quem pegou |
|---|---|
| corrompe 1 grupo a cada 5 000 | `volume_100k_casos_por_largura`, `volume_em_sequencias_curtas…` |
| corrompe todo grupo | `estrategia_rapida_produz_os_mesmos_bytes_que_a_generica`, `msb_first_em_vetores_conhecidos` |

A primeira tentativa de mutação (`word = 1` **antes** do laço) não derrubou
nada — e estava certo não derrubar: aquele bit é deslocado para fora dos
bytes extraídos, então era uma mutação inócua, não um teste cego.

### 2.4 Microbench criterion salvo como baseline — feito [MEDIDO]

```
$ taskset -c 0 cargo bench -p higher-core -- --save-baseline e1
```

96 medições (32 pack + 32 unpack + 16 pack genérico + 16 unpack genérico).
O baseline do criterion fica em `target/criterion/`, que é ignorado pelo
git; o snapshot versionado, com metadados de máquina e carga, está em
[`results/reference/E1-packing-ns-por-simbolo.json`](reference/E1-packing-ns-por-simbolo.json).

Comparar com ele depois: `make bench-compare` (ou
`cargo bench -p higher-core -- --baseline e1`).

---

## 3. Números medidos [MEDIDO]

**Ambiente.** Intel Xeon @ 2.10 GHz, 4 núcleos, 16 GB, Linux 6.18.44.
Processo fixado em `taskset -c 0`. Perfil `bench` (herda `release`:
`lto = "fat"`, `codegen-units = 1`), **sem** `-C target-cpu=native`.
Criterion: 500 ms de warmup, 2 s de medição, 50 amostras; o valor é a
mediana. Carga: 16 384 símbolos, seed 42.

**Ressalva que vale para todos os números desta seção.** Container
compartilhado. `docs/02 §6` exige máquina dedicada, governor
`performance` e turbo registrado; nada disso é exposto nem verificável
aqui, e `perf` não está instalado. Estes números servem para **comparar
larguras entre si dentro do mesmo run** e para **detectar regressão**
contra o baseline. Não são medida absoluta, e não entram em nenhuma
avaliação de H1–H7.

| `b` | estratégia | pack | unpack | pack genérico | unpack genérico | ganho pack | ganho unpack |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | `group` | 0.781 | 0.753 | 1.810 | 2.169 | **2.32×** | **2.88×** |
| 2 | `group` | 0.859 | 0.802 | 1.898 | 2.129 | **2.21×** | **2.65×** |
| 3 | `group` | 0.864 | 0.777 | 1.916 | 2.755 | **2.22×** | **3.55×** |
| 4 | `group` | 0.993 | 0.916 | 2.005 | 2.167 | **2.02×** | **2.37×** |
| 5 | `group` | 0.931 | 0.783 | 2.135 | 3.192 | **2.29×** | **4.08×** |
| 6 | `group` | 0.919 | 0.820 | 2.174 | 3.257 | **2.37×** | **3.97×** |
| 7 | `group` | 0.936 | 0.791 | 2.233 | 3.773 | **2.38×** | **4.77×** |
| 8 | `native` | 0.756 | 0.094 | 2.305 | 2.135 | **3.05×** | **22.69×** |
| 9 | `generic` | 2.392 | 4.438 | — | — | — | — |
| 10 | `group` | 0.889 | 0.807 | 2.547 | 4.239 | **2.87×** | **5.25×** |
| 11 | `generic` | 2.539 | 4.827 | — | — | — | — |
| 12 | `group` | 1.109 | 0.968 | 2.718 | 4.294 | **2.45×** | **4.44×** |
| 13 | `generic` | 2.726 | 5.455 | — | — | — | — |
| 14 | `group` | 1.011 | 0.805 | 2.962 | 5.384 | **2.93×** | **6.69×** |
| 15 | `generic` | 2.953 | 5.825 | — | — | — | — |
| 16 | `native` | 1.319 | 0.110 | 2.979 | 4.263 | **2.26×** | **38.93×** |
| 17 | `generic` | 3.128 | 6.590 | — | — | — | — |
| 18 | `generic` | 3.296 | 6.466 | — | — | — | — |
| 19 | `generic` | 3.431 | 7.255 | — | — | — | — |
| 20 | `group` | 1.113 | 0.990 | 3.400 | 6.372 | **3.05×** | **6.43×** |
| 21 | `generic` | 3.754 | 7.548 | — | — | — | — |
| 22 | `generic` | 3.795 | 7.623 | — | — | — | — |
| 23 | `generic` | 3.887 | 8.167 | — | — | — | — |
| 24 | `group` | 1.405 | 1.114 | 3.517 | 6.441 | **2.50×** | **5.78×** |
| 25 | `generic` | 4.010 | 8.613 | — | — | — | — |
| 26 | `generic` | 4.192 | 8.669 | — | — | — | — |
| 27 | `generic` | 4.077 | 9.196 | — | — | — | — |
| 28 | `group` | 1.255 | 0.979 | 4.071 | 8.511 | **3.24×** | **8.69×** |
| 29 | `generic` | 5.364 | 9.688 | — | — | — | — |
| 30 | `generic` | 4.608 | 9.627 | — | — | — | — |
| 31 | `generic` | 4.821 | 10.120 | — | — | — | — |
| 32 | `native` | 0.748 | 0.266 | 3.938 | 8.745 | **5.27×** | **32.83×** |

### 3.1 O ganho do caminho especializado (`docs/01 §7`)

| Estratégia | Larguras | pack (ns/símbolo) | unpack (ns/símbolo) | ganho vs. genérico |
|---|---:|---|---|---|
| `native` | 3 | 0,748 – 1,319 | 0,094 – 0,266 | pack 2,26×–5,27×; unpack 22,7×–38,9× |
| `group` | 13 | 0,781 – 1,405 | 0,753 – 1,114 | pack 2,02×–3,24×; unpack 2,37×–8,69× |
| `generic` | 16 | 2,392 – 5,364 | 4,438 – 10,120 | — (é a referência) |

Nesta amostra, **toda** largura com caminho especializado ganha do
genérico, nas duas direções. O ganho tende a crescer com `b` dentro de
cada estratégia, o que é coerente: quanto mais bits por símbolo, mais
trabalho de shift o genérico faz por símbolo.

### 3.2 O bench mudou o código, não só o relatório

A primeira versão passava o número de bytes por símbolo como **variável de
runtime** e fatiava `to_be_bytes()` com esse valor. O resultado:

| | antes | depois |
|---|---:|---:|
| `pack` b=8 (16 384 símbolos) | 59,9 µs | 12,4 µs |
| `unpack` b=4, ganho vs. genérico | **0,48×** (perdia) | 2,37× |
| `unpack` b=24, ganho vs. genérico | **0,78×** (perdia) | 5,78× |

Com tamanho variável o compilador não especializa a cópia, e o caminho
"rápido" chegava a ser mais lento que o genérico. A correção foi passar
`b`, o tamanho do grupo e o número de bytes como **constantes de tipo**,
pela tabela `para_cada_grupo!`. Um teste
(`a_tabela_de_despacho_bate_com_o_que_width_calcula`) confere que os
literais da tabela batem com o que `Width` deriva, nos dois sentidos —
sem ele, uma divergência empacotaria com o grupo errado em silêncio.

Registrado aqui porque é o tipo de coisa que, sem bench, viraria "o
caminho especializado não compensa" num relatório futuro.

### 3.3 Verificação do número mais suspeito

`unpack` de `base-8` deu 0,094 ns/símbolo — 43 GB/s de saída, ou ~0,2
ciclo por símbolo. Número dessa ordem merece desconfiança antes de ir para
um relatório, então medi de novo fora do criterion, com relógio próprio e
warmup: **0,0925 ns/símbolo**, contra 0,0923 do criterion. O valor é real.

A explicação é o tamanho da carga: 16 384 símbolos × 4 B = 64 KB de
saída, que **cabe em cache** por decisão de projeto do bench (o objetivo é
medir packing, não hierarquia de memória). Fora de cache o comportamento é
outro — no mesmo teste, com 8 milhões de símbolos:

| `b` | 16 384 símbolos | 8 000 000 símbolos |
|---:|---:|---:|
| 8 | 0,093 ns/sym | 0,192 ns/sym |
| 16 | 0,113 ns/sym | 1,209 ns/sym |
| 32 | 0,265 ns/sym | 1,320 ns/sym |
| 14 | 0,808 ns/sym | 1,776 ns/sym |

Medir isso a sério, com contadores de cache, é a E5 (`docs/02 §5`). Fica
registrado como alerta: **nenhuma conclusão sobre processamento deve sair
de uma carga que cabe em cache**, e a curva `llc_miss_per_kb` do gráfico 5
de `docs/02 §8` existe exatamente por isso.

---

## 4. Desvios da especificação

| # | Desvio | Motivo |
|---|---|---|
| V1 | "≥ 10⁵ **casos** aleatórios por largura" lido como 10⁵ **símbolos** sorteados e roundtripados, não 10⁵ sequências | 10⁵ sequências × 32 larguras seriam 3,2 milhões de sequências numa suíte que roda em debug em toda CI: minutos por execução. A leitura adotada dá 3,2 milhões de símbolos por regime, em dois regimes (fluxo único e sequências curtas), em 0,3 s. Se a leitura estrita for a pretendida, cabe um teste `#[ignore]` rodado em release. |
| V2 | O caminho de grupo foi implementado para **13** larguras, não só para as três (`12, 14, 24`) que `docs/01 §7` cita | O critério `lcm(b,8)/b · b ≤ 64` é objetivo e inclui as três pedidas. Restringir às três deixaria `lower-1..7` no genérico sem razão técnica — e elas ganham de 2,0× a 4,8×. |
| V3 | `unpack` é estrito: exige o número **exato** de bytes e o completamento em zeros | `docs/01 §7` manda completar com zeros mas não diz o que fazer se não estiverem. Aceitar em silêncio significaria devolver símbolos plausíveis e errados quando o `n_symbols` do cabeçalho divergir do payload — o defeito D7 de `docs/05 §3` por outro caminho. Quem precisar de leitura frouxa usa o `BitReader` direto. |
| V4 | `push_all` valida a sequência inteira **antes** de escrever | Custa uma passada a mais, mas garante que um valor largo demais no meio não deixe meio fluxo gravado (testado em `valor_largo_demais_nao_deixa_fluxo_meio_gravado`). Em troca, a validação sai do laço quente. |
| V5 | `[lib] bench = false` em `higher-core/Cargo.toml` | O harness do libtest também é alvo de `cargo bench` e não entende `--save-baseline`, o que fazia o comando do aceite falhar. |
| V6 | Bench com 500 ms de warmup e 2 s de medição, em vez do padrão do criterion | 32 larguras × 4 benches com o padrão passariam de 10 minutos. A medição oficial (E5) usa o protocolo completo de `docs/02 §6`. |

---

## 5. Ambiguidades novas, para as próximas etapas

As dez da E0 continuam em `results/ETAPA-00.md §4`. Esta etapa acrescentou
duas:

1. **`base-8` "sem packing" (`docs/01 §1`) versus o `BitWriter`.** Aqui
   `b = 8` passa pelo `BitWriter` como qualquer outra largura, só que pelo
   caminho nativo — a saída é byte a byte idêntica à entrada, então a
   identidade vale. Mas isso significa que `base-8` **tem** um custo de
   encode medível (0,757 ns/símbolo), e a E5 precisa decidir se a régua de
   processamento é esse valor ou um `memcpy` puro. `docs/02 §7.2` fala em
   "`base-8` + memcpy", o que sugere memcpy. **(decidir na E5)**
2. **Sync markers e o alinhamento do `BitWriter`.** `docs/01 §8` diz que o
   marker "alinha ao byte" a cada `K` símbolos. O `BitWriter` já sabe
   responder `is_byte_aligned()`, mas ainda não sabe **forçar** o
   alinhamento completando com zeros no meio do fluxo. É uma operação de
   poucas linhas, e entra na E8 junto com os markers — não antes, para não
   acrescentar API sem uso nem teste. **(E8)**

---

## 6. Próxima etapa

**E2 — Modelo paramétrico e formato `.hgr`.** É onde entram tabela,
tokenizador P-greedy, escape do LOWER e o cabeçalho de 36 bytes — e onde a
ambiguidade §4.2 da E0 (`base-8:M2` é nome legítimo ou erro?) precisa de
resposta.
