# corpora/gen — geradores sintéticos

Cada gerador controla **uma variável por vez** (`docs/02 §2.2`):

- tamanho do alfabeto (2, 4, 16, 64, 256);
- entropia de ordem 0 (bits/byte alvo);
- taxa de repetição de blocos e comprimento médio dos blocos;
- distribuição de Zipf do vocabulário (expoente variável).

Requisito da Etapa E3: com a mesma seed, a saída é **idêntica byte a byte**
entre execuções e entre máquinas. Isso exige PRNG próprio e determinístico,
não o do sistema.

Implementação na Etapa E3.
