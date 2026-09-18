# results

Os runs brutos (`results/<timestamp>-<commit>/*.json`) ficam **fora do git**
(`docs/02 §6.6`). O que é versionado:

- `results/ETAPA-XX.md` — o relatório de fechamento de cada etapa, com os
  números medidos, rotulados `[MEDIDO]`, e os desvios da especificação;
- `results/reference/` — snapshots de referência escolhidos a dedo;
- `results/REPORT.md` — o relatório final da Etapa E10.

Rótulo obrigatório em todo número (`CLAUDE.md`, postura científica):
`[MEDIDO]` só para o que saiu de uma execução, com commit, máquina e seed
registrados. Previsão e derivação matemática nunca entram sem rótulo próprio.
