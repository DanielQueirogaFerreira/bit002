# Alvos do bit002. Cada etapa de docs/03-ETAPAS.md acrescenta os seus.
# Alvos ainda não implementados falham dizendo em que etapa eles entram,
# em vez de produzir saída vazia que pareça um resultado.

CARGO ?= cargo
WASM_TARGET := wasm32-unknown-unknown
BASELINE ?= e1

.DEFAULT_GOAL := help
.PHONY: help test lint fmt fmt-check clippy wasm build-web check corpora bench \
	bench-baseline bench-compare report clean

help: ## Lista os alvos disponíveis
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

check: fmt-check clippy test wasm ## Tudo que a CI roda, na mesma ordem

test: ## Testes do workspace (roundtrip a partir da E2)
	$(CARGO) test --workspace --all-targets

fmt: ## Formata o código
	$(CARGO) fmt --all

fmt-check: ## Verifica a formatação sem alterar arquivos
	$(CARGO) fmt --all --check

clippy: ## Clippy com warnings tratados como erro
	$(CARGO) clippy --workspace --all-targets -- -D warnings

lint: fmt-check clippy ## fmt-check + clippy

wasm: ## Compila o higher-wasm para wasm32-unknown-unknown
	$(CARGO) build -p higher-wasm --target $(WASM_TARGET) --release

build-web: ## wasm-pack + version.json para a UI (docs/04 §3)
	npm run build

corpora: ## [E3] Baixa os corpora reais e confere os sha256
	@echo "make corpora: ainda não existe — é a Etapa E3 de docs/03-ETAPAS.md." >&2
	@exit 2

bench: ## Microbench criterion de pack/unpack por b (E1)
	$(CARGO) bench -p higher-core

bench-baseline: ## Roda o bench e salva como baseline `$(BASELINE)`
	$(CARGO) bench -p higher-core -- --save-baseline $(BASELINE)

bench-compare: ## Compara o bench atual com o baseline `$(BASELINE)`
	$(CARGO) bench -p higher-core -- --baseline $(BASELINE)

report: ## [E10] Gera results/REPORT.md a partir dos runs
	@echo "make report: ainda não existe — é a Etapa E10 de docs/03-ETAPAS.md." >&2
	@exit 2

clean: ## Remove artefatos de build
	$(CARGO) clean
	rm -rf web/pkg web/dist web/version.json
