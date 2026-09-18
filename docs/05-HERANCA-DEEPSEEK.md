# 05 — Herança da fase de ideação (DeepSeek) e defeitos a não repetir

Fonte: conversa "Análise arquitetura Higher" no DeepSeek (12 mensagens,
set/2026). O código gerado lá (inclusive o rascunho `bit001`) **não** é trazido para o `bit002`. Serve
só como referência conceitual. Tudo é reimplementado a partir de `01` e `02`.

## 1. Linha do tempo

| # | Etapa | Resultado aproveitado |
|---|---|---|
| 1 | Documento de especificação original (Tipo 1 humano, Tipo 2 máquina, 14 bits, roadmap) | tese e camadas → `00 §2` |
| 2 | Análise crítica | alertas: 14 bits não é nativo; Tipo 2 exige sincronização; compressão não é exponencial; 4×14 = 56 bits = 7 bytes |
| 3 | Fundador esclarece: várias tabelas fixas pré-acordadas + um quarto modelo adaptativo; pede o inverso (< 8 bits), 16 bits e o sweet spot | modelos M0–M4, matemática do limiar → `00 §3`, `01 §2` |
| 4 | Especificação formal M0–M4 | layout de namespace, função de ganho, protocolo M4, métricas, critérios de decisão → `01`, `02 §7` |
| 5 | Protótipo Python M0–M4 | descartado (ver §3) |
| 6 | Protótipo paramétrico 1..32 com escape para LOWER | a ideia do modelo único e do escape `0` → `01 §1, §6` |
| 7 | Rascunho `bit001` (JS + Worker + D1 + UI) | requisitos de UI e esquema D1 → `04`. O código não é reaproveitado: o repositório real é `bit002` |

## 2. Números herdados que continuam válidos (derivação matemática, não medição)

- Limiar de redução `1 − 8/b` (b=9: 11,1%; 12: 33,3%; 14: 42,9%; 16: 50%).
- Custo de uma tabela completa de 14 bits transmitida: 16.384 × 14 bits ≈ 28 KB.
  Isso é ~2,8% de um stream de 1 MB, ~28% de 100 KB, e fica inviável em 10 KB.
  Esse é o motivo de o M4 transmitir só deltas.
- 14 bits: 4 símbolos = 7 bytes. Em palavra de 64 bits sobram 8 bits
  (desperdício, ou flags/ECC que anulam o ganho).
- 14 vs 16: o 14 economiza 12,5% por símbolo em relação ao 16 se o packing
  for perfeito. O 16 é nativo (memória, SIMD, barramento).

## 3. Defeitos conhecidos do protótipo anterior (não repetir)

| # | Defeito | Consequência | Correção nesta especificação |
|---|---|---|---|
| D1 | Tabela treinada no corpus `mixed`, que também era corpus de teste. Os geradores sintéticos compartilhavam vocabulário | ganho inflado por vazamento | treino/teste disjuntos + matriz cruzada (`02 §2.3`) |
| D2 | Corpora 100% sintéticos, com vocabulário pequeno | favorece dicionário artificialmente | corpora reais como base (`02 §2.1`) |
| D3 | Tempo medido em Python/JS | mede o interpretador, não a arquitetura | núcleo em Rust + criterion + perf (`02 §5–6`) |
| D4 | Candidatos só por palavras (`tokenize_words`), ganho sem considerar sobreposição | tabela subótima, ganho mal estimado | n-gramas + palavras + re-pontuação iterativa (`01 §4`) |
| D5 | Bloco adaptativo serializado com código `u16` | quebra para `b > 16` | códigos em `b` bits (`01 §9`) |
| D6 | Adaptativo só habilitado para `b ≥ 12`, sem deltas incrementais (dicionário inteiro no cabeçalho) | M4 não era M4 de fato | protocolo de deltas com seq/LRU/TTL/checkpoint |
| D7 | Cabeçalho de 4 bytes só com `n_tokens`, sem id de tabela nem checksum | decoder aceita tabela errada em silêncio | cabeçalho `.hgr` de 36 bytes com `table_id` e crc (`01 §8`) |
| D8 | Sweep padrão pulava larguras (`…10,12,14,16,20,24,32`) | curva incompleta | sweep 1..32 inteiro, obrigatório |
| D9 | Tabela "comportamento esperado 1..32" apresentada junto dos resultados | previsão confundida com medição | rótulos `[PREVISÃO]`/`[MEDIDO]` |
| D10 | Sem baselines reais no código (só listadas na especificação) | ganho só contra byte cru | gzip/zstd/brotli/lz4/BPE obrigatórios (`02 §1`) |
| D11 | `bit001` com ~16 arquivos gerados de uma vez, nunca executados | inconsistências prováveis entre módulos | construção por etapas com critério de aceite (`03`) |

## 4. `[PREVISÃO]` herdada, a confirmar ou refutar

Tabela de "comportamento esperado" do DeepSeek. **Não é resultado.**

| Faixa | Previsão |
|---|---|
| 1–3 bits | escape domina, b/B entre 9 e 16, piora em tudo |
| 4–5 bits | viável só com alfabeto minúsculo |
| 6–7 bits | empata ou ganha levemente em repetitivo e logs |
| 8 bits | régua |
| 9–12 bits | ganho começa em logs e repetitivo |
| 13–16 bits | sweet spot |
| 17–24 bits | ganho marginal decrescente, tabela cresce |
| 25–32 bits | packing caro, tabela enorme, ganho quase nulo |

O relatório final (E10) compara esta tabela com os valores medidos, linha a linha.
