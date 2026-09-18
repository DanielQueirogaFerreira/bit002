# web — UI da Arquitetura Higher

HTML/CSS/JS puro + o WASM do `higher-core`. Especificação completa (layout
9:16, action button arrastável, version badge com `age`) em
[`../docs/04-UI-DEPLOY.md`](../docs/04-UI-DEPLOY.md).

**Estado:** só a fundação. A UI de verdade é a Etapa E9.

Gerados pelo build e fora do git: `pkg/` (wasm-pack), `dist/` (bundle servido
pelo Worker) e `version.json` (`npm run gen-version`).

Lembrete de `docs/02 §6.7`: tempo medido no browser é **secundário** e sai
rotulado `env=wasm`. O número oficial de processamento vem do nativo.
