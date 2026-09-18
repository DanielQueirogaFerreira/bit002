# corpora

- `gen/` — geradores sintéticos com seed (`docs/02 §2.2`). Servem para
  **explicar curvas**, isolando uma variável por vez. Não sustentam a tese.
- `real/` — `manifest.json` (URL + sha256 + licença) e o script de download.
  Os resultados principais vêm daqui (`docs/02 §2.1`).

Os **dados** reais ficam fora do git (`corpora/real/data/`, no `.gitignore`).
Só o manifest é versionado. `make corpora` baixa e confere os hashes — a
partir da Etapa E3.

Divisão obrigatória por bloco contíguo, nunca por amostragem de linhas
(`docs/02 §2.3`): treino 50% / validação 10% / teste 40%. Tabela treinada no
corpus de teste foi o defeito D1 do protótipo anterior (`docs/05 §3`).
