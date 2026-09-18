#!/usr/bin/env node
// Grava web/version.json com {version, commit, build_ts_ms} (docs/04 §3).
//
// O version badge da UI lê esse arquivo. O `build_ts_ms` é o epoch em
// milissegundos no momento do build — é dele que sai o "age" do badge.

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const raiz = join(dirname(fileURLToPath(import.meta.url)), "..");

function gitOuNulo(...args) {
  try {
    return execFileSync("git", args, {
      cwd: raiz,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    // Build a partir de um tarball, sem .git: o commit fica desconhecido,
    // e isso precisa aparecer como tal em vez de virar string vazia.
    return null;
  }
}

const pkg = JSON.parse(readFileSync(join(raiz, "package.json"), "utf8"));
const sujo = gitOuNulo("status", "--porcelain");

const version = {
  version: pkg.version,
  commit: gitOuNulo("rev-parse", "HEAD") ?? "desconhecido",
  commit_short: gitOuNulo("rev-parse", "--short", "HEAD") ?? "desconhecido",
  dirty: sujo === null ? null : sujo.length > 0,
  build_ts_ms: Date.now(),
  build_iso: new Date().toISOString(),
};

const destino = join(raiz, "web", "version.json");
mkdirSync(dirname(destino), { recursive: true });
writeFileSync(destino, `${JSON.stringify(version, null, 2)}\n`);
console.log(`web/version.json: v${version.version} · ${version.commit_short} · ${version.build_ts_ms}`);
