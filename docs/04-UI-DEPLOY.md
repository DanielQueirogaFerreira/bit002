# 04 — UI, versionamento e deploy (GitHub + Cloudflare Workers + D1)

Repositório: **`bit002`** (GitHub). Worker e D1 na Cloudflare também se chamam **`bit002`**. URL pública: **`https://bit002.daniel-queiroga.workers.dev`** (a API responde em `/api/*` no mesmo domínio). A UI serve para **visualizar os testes**. Ela não é
o instrumento de medição principal: os tempos de processamento oficiais vêm
do nativo (`02 §6.7`).

## 1. Layout geral

- Otimizada para **vertical 9:16** (mobile-first) e funcional em **16:9 horizontal**.
- Sem barra de navegação fixa. **Toda a navegação fica dentro de um único action button (FAB).**
- Telas: `Sweep` (rodar 1..32 no browser), `Resultados` (gráficos de `02 §8`),
  `Comparar` (modelos × baselines, Pareto), `Runs` (histórico do D1, filtros),
  `Tese` (resumo de `00` e status de H1–H7), `Config` (corpus, tamanhos, seed).
- HTML/CSS/JS puro + WASM do `higher-core`. Gráficos em canvas/SVG próprios,
  sem dependência pesada.

## 2. Action button (FAB)

| Requisito | Detalhe |
|---|---|
| Posição padrão | canto **inferior direito** (modo destro) |
| Flutuante | sempre visível, acima de todo conteúdo, `position: fixed`, respeita `safe-area-inset` |
| Arrastável | pointer events (touch + mouse). Pode ser solto **em qualquer ponto** da tela, com snap suave opcional às bordas. Posição persistida por viewer (`localStorage` com try/catch) |
| Projeção dinâmica do menu | ao abrir, calcula o quadrante/borda mais próximo e abre as opções **para o lado com mais espaço livre** (leque radial ou lista), escolhendo o ângulo que não sai da viewport. Recalcula em resize/rotação |
| Conteúdo | itens de navegação das telas + **alternador destro/canhoto** |
| Acessibilidade | alvo ≥ 48 px, `aria-expanded`, foco por teclado, `Esc` fecha. Distinguir arraste de toque por limiar de ~6 px |

**Alternador destro/canhoto:** no modo canhoto, o FAB vai para o canto
inferior **esquerdo**, e o version badge **e** seu botão de revelar/ocultar
passam para o lado oposto. Tudo espelha junto. Preferência persistida.

## 3. Version badge

| Requisito | Detalhe |
|---|---|
| Posição | canto inferior **esquerdo** no modo destro (lado oposto ao FAB). Espelha no modo canhoto |
| Estilo | version badge pequeno, técnico, com **text scrim** (fundo semitransparente atrás do texto para legibilidade sobre qualquer conteúdo) |
| Conteúdo | `vX.Y.Z` · `build` · **timestamp em milissegundos** (epoch ms e ISO) · **age** |
| Revelar/ocultar | botão pequeno ao lado do badge. Oculto mostra só um indicador mínimo |

**Formato do age:** dois dígitos de valor + duas letras de escala, usando a
maior escala com valor ≥ 1:

| Escala | Código | Limite |
|---|---|---|
| segundos | `sc` | < 60 s |
| minutos | `mn` | < 60 min |
| horas | `hr` | < 24 h |
| dias | `dy` | < 7 d |
| semanas | `wk` | < ~4,35 wk (30,44 d) |
| meses | `mt` | < 12 mt |
| anos | `yr` | 99yr é o teto |

Exemplos: `07sc`, `42mn`, `03hr`, `02wk`, `11mt`, `01yr`. Atualiza a cada
segundo enquanto o valor estiver em `sc`, e a cada minuto depois.

**Geração de versão:** `scripts/gen-version.mjs` no build grava
`web/version.json` com `{version, commit, build_ts_ms}`. A versão vem do
`package.json` (`"name": "bit002"`, começando em `0.1.0`)/tag git, e o `build_ts_ms` vem de `Date.now()` no build.

## 4. Worker + D1

`worker/wrangler.toml` com binding `DB` (D1) e assets estáticos de `web/`:

```toml
name = "bit002"
main = "src/worker.js"
compatibility_date = "2026-09-01"

[assets]
directory = "../web/dist"

[[d1_databases]]
binding = "DB"
database_name = "bit002"
database_id = "<preencher após wrangler d1 create bit002>"
migrations_dir = "migrations"
```

### 4.1 `schema.sql`

Estende o esquema herdado do DeepSeek com os campos de `02 §6.1`:

```sql
CREATE TABLE IF NOT EXISTS runs (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id           TEXT UNIQUE NOT NULL,
  created_at       INTEGER NOT NULL,        -- ms, servidor
  client_ts        INTEGER,                 -- ms, cliente
  client_version   TEXT,
  commit_sha       TEXT,
  session_id       TEXT,
  machine_id       TEXT,
  env              TEXT NOT NULL,           -- 'native' | 'wasm'
  model            TEXT NOT NULL,           -- 'higher-14'
  bits             INTEGER NOT NULL CHECK (bits BETWEEN 1 AND 32),
  variant          TEXT NOT NULL,           -- 'M0'|'M1'|'M2'|'M4'
  parser           TEXT NOT NULL DEFAULT 'greedy',
  table_id         TEXT,
  train_corpus     TEXT,
  corpus           TEXT NOT NULL,
  corpus_sha256    TEXT,
  corpus_size      INTEGER NOT NULL,
  seed             INTEGER,
  encoded_bytes    INTEGER NOT NULL,
  header_bytes     INTEGER,
  table_bytes      INTEGER,
  symbols          INTEGER NOT NULL,
  ratio            REAL NOT NULL,
  bits_per_byte    REAL NOT NULL,
  horiz_reduction  REAL,
  escape_rate      REAL,
  slots_used       INTEGER,
  slots_useful     INTEGER,
  overhead_sync    REAL,
  enc_ns_per_byte  REAL,
  dec_ns_per_byte  REAL,
  cycles_per_byte  REAL,
  energy_j         REAL,
  metrics_json     TEXT,                    -- op_speedup, perf counters, etc.
  ok               INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_runs_model   ON runs(bits, variant);
CREATE INDEX IF NOT EXISTS idx_runs_corpus  ON runs(corpus);
CREATE INDEX IF NOT EXISTS idx_runs_env     ON runs(env);
CREATE INDEX IF NOT EXISTS idx_runs_created ON runs(created_at);

CREATE TABLE IF NOT EXISTS baselines (
  id INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL,
  codec TEXT NOT NULL, level TEXT, corpus TEXT NOT NULL, corpus_size INTEGER NOT NULL,
  encoded_bytes INTEGER NOT NULL, bits_per_byte REAL NOT NULL,
  enc_ns_per_byte REAL, dec_ns_per_byte REAL, env TEXT NOT NULL, created_at INTEGER NOT NULL
);
```

Mudanças de esquema são feitas só por migrações numeradas (`worker/migrations/000N_*.sql`).

### 4.2 API

| Método | Rota | Função |
|---|---|---|
| `GET` | `/api/version` | versão, commit e build_ts_ms |
| `POST` | `/api/runs` | insere um lote de runs (validação de esquema, máx. N por request) |
| `GET` | `/api/runs` | lista com filtros `bits, variant, corpus, env, since`, paginada |
| `GET` | `/api/aggregate` | mediana/p5/p95 por `(corpus, bits, variant, env)` |
| `POST` | `/api/import` | importa o JSONL da sweep nativa (`env=native`) |

Escrita protegida por token (secret do Worker). Leitura pública ou protegida:
decidir na E9.

## 5. Deploy

```bash
npm i -g wrangler
wrangler d1 create bit002            # copiar database_id para wrangler.toml
wrangler d1 migrations apply bit002 --remote
npm run build                        # wasm-pack + gen-version
wrangler deploy                      # publica em https://bit002.daniel-queiroga.workers.dev
```

GitHub: repo `bit002`, com CI fazendo o deploy no push para `main` (secret
`CLOUDFLARE_API_TOKEN`) depois de testes verdes.
