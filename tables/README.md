# tables — tabelas pré-acordadas versionadas

Nome do artefato: `<b>-<variant>-<corpus_treino>-<sha8>.tbl`
(ex.: `14-M2-logs-apache-9f3c1a2b.tbl`). O `table_id` no cabeçalho `.hgr` são
os 8 primeiros bytes do sha256 da tabela (`docs/01 §8`).

Cada tabela carrega, nos metadados: `b`, variante, sha256 do corpus de
**treino**, número de entradas, `slots_uteis` e tamanho serializado
(`docs/01 §4.4`).

A regra que não se negocia: a tabela é construída **só** no corpus de treino
e avaliada em corpus de teste disjunto (`docs/02 §2.3`). O corpus de treino
fica registrado no nome e nos metadados justamente para que uma violação
seja visível no `results/`.

Tabelas entram a partir da Etapa E2; as três variantes por largura de
interesse, na E6.
