# worker — Cloudflare Worker `bit002`

Persiste o histórico de runs no D1 `bit002`. A especificação completa (rotas,
esquema, deploy) está em [`../docs/04-UI-DEPLOY.md`](../docs/04-UI-DEPLOY.md).

**Estado:** só a fundação. `src/worker.js` responde `/api/version` e devolve
501 no resto. A API de verdade (`/api/runs`, `/api/aggregate`, `/api/import`)
é a Etapa E9.

## Esquema

O `schema.sql` de `docs/04 §4.1` vive aqui como **migração numerada**, em
`migrations/0001_init.sql`, e essa é a única fonte da verdade. Não existe um
`schema.sql` solto: dois arquivos com o mesmo esquema divergem na primeira
alteração. Toda mudança entra como `migrations/000N_*.sql`.

## Deploy (E9)

```bash
npm i -g wrangler
wrangler d1 create bit002                 # copiar o database_id para wrangler.toml
wrangler d1 migrations apply bit002 --remote
npm run build                             # wasm-pack + gen-version
wrangler deploy --config worker/wrangler.toml
```

O token de escrita (`/api/runs`, `/api/import`) é secret do Worker, nunca
arquivo versionado.
