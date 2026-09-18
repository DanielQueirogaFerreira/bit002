# corpora/real — corpora reais

Os dados ficam em `data/`, que está no `.gitignore`. O que é versionado é o
`manifest.json`: URL, sha256 e licença de cada arquivo. `make corpora` baixa
e confere os hashes (Etapa E3).

Se um hash não bater, o download falha — corpus diferente invalida a
comparação com qualquer run anterior.
