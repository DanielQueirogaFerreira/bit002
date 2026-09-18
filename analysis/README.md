# analysis — relatórios offline

Python (pandas/matplotlib) só para **derivar relatórios** dos JSON/CSV que a
sweep nativa gerou. Nunca para medir tempo: o tempo do interpretador domina
e mediria o Python, não a arquitetura (defeito D3, `docs/05 §3`).

Saída da Etapa E10: `analysis/report.py` gera `results/REPORT.md` com os 9
gráficos de `docs/02 §8` e o veredito de H1–H7 — sustentada, refutada ou
inconclusiva, com números e intervalos.

Cada gráfico traz no rodapé o commit, a máquina, a data e se os dados são
`[MEDIDO]`.
