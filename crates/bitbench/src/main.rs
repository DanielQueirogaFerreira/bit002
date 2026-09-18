//! CLI de benchmark da Arquitetura Higher.
//!
//! Na Etapa E0 a CLI só sabe se descrever: `version` e `models`. A `sweep`
//! de verdade (métricas, corpora, baselines) é a Etapa E4, e até lá o
//! comando existe apenas para falhar de forma explícita — um número
//! inventado aqui viraria "resultado" no relatório, que é exatamente o
//! defeito D9 de `docs/05`.

use std::io::{self, Write};
use std::process::ExitCode;

use higher_core::model::{all_widths, Variant};

const USO: &str = "\
bitbench — CLI de benchmark da Arquitetura Higher

USO:
    bitbench <comando> [opções]

COMANDOS:
    version     imprime a versão do bitbench e do higher-core
    models      lista os 32 modelos da sweep (1..32) com faixa e limiar
    sweep       [E4] roda a sweep completa e grava os runs
    help        esta ajuda
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let comando = args.first().map_or("help", String::as_str);

    match comando {
        "version" | "--version" | "-V" => {
            println!("bitbench {}", env!("CARGO_PKG_VERSION"));
            println!("higher-core {}", higher_core::VERSION);
            ExitCode::SUCCESS
        }
        "models" => {
            listar_modelos();
            ExitCode::SUCCESS
        }
        "sweep" => {
            eprintln!(
                "bitbench: `sweep` ainda não existe — é a Etapa E4 de docs/03-ETAPAS.md.\n\
                 Até lá a CLI não emite número nenhum, para não confundir previsão com medição."
            );
            ExitCode::from(2)
        }
        "help" | "--help" | "-h" => {
            print!("{USO}");
            ExitCode::SUCCESS
        }
        outro => {
            eprintln!("bitbench: comando desconhecido `{outro}`\n");
            eprint!("{USO}");
            ExitCode::from(2)
        }
    }
}

/// Lista as 32 larguras com o que já é derivável da aritmética da tese.
///
/// Nada aqui é medição: os limiares vêm de `docs/00-TESE.md §3`.
///
/// Escreve com `writeln!` e ignora erro de escrita porque a saída é para
/// ser filtrada (`bitbench models | head`), e fechar o pipe não é falha.
fn listar_modelos() {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(
        out,
        "{:<13} {:>2}  {:<6} {:>14} {:>9} {:>7} {:>8}",
        "modelo", "b", "faixa", "capacidade", "contêiner", "grupo", "limiar"
    );
    for w in all_widths() {
        let escrita = writeln!(
            out,
            "{:<13} {:>2}  {:<6} {:>14} {:>8}B {:>3}→{:<3} {:>7.1}%",
            w.run_name(Variant::M2),
            w.bits(),
            format!("{:?}", w.band()),
            w.capacity(),
            w.container_bytes(),
            w.pack_group_symbols(),
            w.pack_group_bytes(),
            w.threshold() * 100.0,
        );
        if escrita.is_err() {
            return;
        }
    }
    let _ = writeln!(
        out,
        "\n[derivação] limiares calculados de docs/00 §3. Não são medição."
    );
}
