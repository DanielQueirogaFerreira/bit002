-- 0001_init — esquema inicial do D1 `bit002` (docs/04 §4.1).
--
-- Uma linha de `runs` por run, com os campos de docs/02 §6.1.
-- `env` separa o nativo (medição oficial) do wasm (secundário, docs/02 §6.7):
-- os dois nunca são agregados juntos.

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
