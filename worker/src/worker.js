// Worker `bit002` — fundação (Etapa E0).
//
// Só `/api/version` responde. As rotas de dados são a Etapa E9
// (docs/04 §4.2) e até lá devolvem 501, em vez de 200 com corpo vazio:
// uma rota que responde "nada" é indistinguível de um banco vazio.

const ROTAS_E9 = ["/api/runs", "/api/aggregate", "/api/import"];

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname === "/api/version") {
      return Response.json({
        name: "bit002",
        version: env.APP_VERSION ?? "0.1.0",
        commit: env.COMMIT_SHA ?? "desconhecido",
        build_ts_ms: Number(env.BUILD_TS_MS ?? 0),
        stage: "E0",
      });
    }

    if (ROTAS_E9.includes(url.pathname)) {
      return Response.json(
        {
          error: "não implementado",
          detalhe: `${url.pathname} é a Etapa E9 de docs/03-ETAPAS.md`,
        },
        { status: 501 },
      );
    }

    if (url.pathname.startsWith("/api/")) {
      return Response.json({ error: "rota desconhecida" }, { status: 404 });
    }

    // Fora de /api/, quem serve é o binding de assets estáticos (web/dist).
    return env.ASSETS ? env.ASSETS.fetch(request) : new Response("Not Found", { status: 404 });
  },
};
