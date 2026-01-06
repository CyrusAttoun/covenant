//! Covenant CLI - Command line interface for the Covenant compiler

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ariadne::{Color, Label, Report, ReportKind, Source};

use covenant_parser::parse;
use covenant_checker::check;
use covenant_graph::{GraphBuilder, execute_query, parse_query, Table, Query};
use covenant_codegen::compile_pure;

#[derive(Parser)]
#[command(name = "covenant")]
#[command(about = "Covenant programming language compiler", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a file and output the AST as JSON
    Parse {
        /// Input file
        file: PathBuf,
        /// Pretty print the output
        #[arg(short, long)]
        pretty: bool,
    },
    /// Type check a file
    Check {
        /// Input file(s)
        files: Vec<PathBuf>,
    },
    /// Compile a file to WASM
    Compile {
        /// Input file
        file: PathBuf,
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Query the codebase
    Query {
        /// Input file(s) to analyze
        files: Vec<PathBuf>,
        /// Query string (e.g., "select * from functions where is_pure = true")
        #[arg(short, long)]
        query: String,
    },
    /// Show information about a file
    Info {
        /// Input file
        file: PathBuf,
    },
    /// Interactive REPL
    Repl,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, pretty } => cmd_parse(&file, pretty),
        Commands::Check { files } => cmd_check(&files),
        Commands::Compile { file, output } => cmd_compile(&file, output),
        Commands::Query { files, query } => cmd_query(&files, &query),
        Commands::Info { file } => cmd_info(&file),
        Commands::Repl => cmd_repl(),
    }
}

fn cmd_parse(file: &PathBuf, pretty: bool) {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    match parse(&source) {
        Ok(program) => {
            let json = if pretty {
                serde_json::to_string_pretty(&program).unwrap()
            } else {
                serde_json::to_string(&program).unwrap()
            };
            println!("{}", json);
        }
        Err(e) => {
            report_parse_error(&source, file, &e);
            std::process::exit(1);
        }
    }
}

fn cmd_check(files: &[PathBuf]) {
    let mut all_ok = true;

    for file in files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {}: {}", file.display(), e);
                all_ok = false;
                continue;
            }
        };

        match parse(&source) {
            Ok(mut program) => {
                match check(&mut program) {
                    Ok(result) => {
                        let fn_count = result.symbols.functions().count();
                        let pure_count = result.symbols.functions()
                            .filter(|s| result.effects.is_pure(s.id))
                            .count();
                        println!(
                            "✓ {} - {} functions ({} pure)",
                            file.display(),
                            fn_count,
                            pure_count
                        );
                    }
                    Err(errors) => {
                        eprintln!("✗ {} - {} errors:", file.display(), errors.len());
                        for err in errors {
                            eprintln!("  {}", err);
                        }
                        all_ok = false;
                    }
                }
            }
            Err(e) => {
                report_parse_error(&source, file, &e);
                all_ok = false;
            }
        }
    }

    if !all_ok {
        std::process::exit(1);
    }
}

fn cmd_compile(file: &PathBuf, output: Option<PathBuf>) {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let mut program = match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            report_parse_error(&source, file, &e);
            std::process::exit(1);
        }
    };

    let result = match check(&mut program) {
        Ok(r) => r,
        Err(errors) => {
            eprintln!("Type check errors:");
            for err in errors {
                eprintln!("  {}", err);
            }
            std::process::exit(1);
        }
    };

    match compile_pure(&program, &result.symbols) {
        Ok(wasm) => {
            let out_path = output.unwrap_or_else(|| {
                let mut p = file.clone();
                p.set_extension("wasm");
                p
            });
            fs::write(&out_path, &wasm).expect("Failed to write output");
            println!("Compiled to {} ({} bytes)", out_path.display(), wasm.len());
        }
        Err(e) => {
            eprintln!("Compilation error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_query(files: &[PathBuf], query_str: &str) {
    // Parse and check all files
    let mut all_programs = Vec::new();

    for file in files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {}: {}", file.display(), e);
                continue;
            }
        };

        match parse(&source) {
            Ok(mut program) => {
                match check(&mut program) {
                    Ok(result) => {
                        all_programs.push((program, result));
                    }
                    Err(_) => {
                        eprintln!("Skipping {} due to type errors", file.display());
                    }
                }
            }
            Err(_) => {
                eprintln!("Skipping {} due to parse errors", file.display());
            }
        }
    }

    // Parse the query
    let query = match parse_query(query_str) {
        Some(q) => q,
        None => {
            eprintln!("Invalid query syntax");
            std::process::exit(1);
        }
    };

    // Execute query against each program
    for (program, result) in &all_programs {
        let graph_builder = GraphBuilder::new(&result.symbols);
        let graph = graph_builder.build(program);

        let query_result = execute_query(&query, &result.symbols, &graph);

        if !query_result.symbols.is_empty() {
            println!("Results:");
            for sym in &query_result.symbols {
                println!("  {} ({}) - {}", sym.name, sym.kind, sym.type_str);
                if !sym.effects.is_empty() {
                    println!("    effects: {:?}", sym.effects);
                }
                if !sym.calls.is_empty() {
                    println!("    calls: {:?}", sym.calls);
                }
                if !sym.called_by.is_empty() {
                    println!("    called_by: {:?}", sym.called_by);
                }
            }
        } else {
            println!("No results found");
        }
    }
}

fn cmd_info(file: &PathBuf) {
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let mut program = match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            report_parse_error(&source, file, &e);
            std::process::exit(1);
        }
    };

    let result = match check(&mut program) {
        Ok(r) => r,
        Err(errors) => {
            eprintln!("Type check errors:");
            for err in errors {
                eprintln!("  {}", err);
            }
            std::process::exit(1);
        }
    };

    let graph_builder = GraphBuilder::new(&result.symbols);
    let graph = graph_builder.build(&program);

    println!("File: {}", file.display());
    println!();

    // List functions
    println!("Functions:");
    for func in result.symbols.functions() {
        let pure_marker = if result.effects.is_pure(func.id) { "○" } else { "●" };
        println!("  {} {} : {}", pure_marker, func.name, func.ty.display());

        let callees = graph.callees_of(func.id);
        if !callees.is_empty() {
            let names: Vec<_> = callees
                .iter()
                .filter_map(|&id| result.symbols.get(id).map(|s| s.name.as_str()))
                .collect();
            println!("    calls: {}", names.join(", "));
        }
    }

    println!();
    println!("Legend: ○ = pure, ● = effectful");
}

fn cmd_repl() {
    use rustyline::DefaultEditor;

    println!("Covenant REPL v0.1.0");
    println!("Type :help for help, :quit to exit");
    println!();

    let mut rl = DefaultEditor::new().expect("Failed to create REPL");
    let mut loaded_source = String::new();

    loop {
        let readline = rl.readline("covenant> ");
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(&line);
                let trimmed = line.trim();

                if trimmed.starts_with(':') {
                    match trimmed {
                        ":quit" | ":q" => break,
                        ":help" | ":h" => {
                            println!("Commands:");
                            println!("  :load <file>  - Load a file");
                            println!("  :parse        - Show parsed AST");
                            println!("  :check        - Type check loaded code");
                            println!("  :info         - Show info about loaded code");
                            println!("  :clear        - Clear loaded code");
                            println!("  :quit         - Exit REPL");
                        }
                        cmd if cmd.starts_with(":load ") => {
                            let path = &cmd[6..].trim();
                            match fs::read_to_string(path) {
                                Ok(s) => {
                                    loaded_source = s;
                                    println!("Loaded {}", path);
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                }
                            }
                        }
                        ":parse" => {
                            if loaded_source.is_empty() {
                                println!("No code loaded. Use :load <file>");
                            } else {
                                match parse(&loaded_source) {
                                    Ok(program) => {
                                        println!("{}", serde_json::to_string_pretty(&program).unwrap());
                                    }
                                    Err(e) => {
                                        eprintln!("Parse error: {}", e);
                                    }
                                }
                            }
                        }
                        ":check" => {
                            if loaded_source.is_empty() {
                                println!("No code loaded. Use :load <file>");
                            } else {
                                match parse(&loaded_source) {
                                    Ok(mut program) => {
                                        match check(&mut program) {
                                            Ok(result) => {
                                                println!("✓ Type check passed");
                                                let fn_count = result.symbols.functions().count();
                                                let pure_count = result.symbols.functions()
                                                    .filter(|s| result.effects.is_pure(s.id))
                                                    .count();
                                                println!("  {} functions ({} pure)", fn_count, pure_count);
                                            }
                                            Err(errors) => {
                                                eprintln!("✗ Type check failed:");
                                                for err in errors {
                                                    eprintln!("  {}", err);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Parse error: {}", e);
                                    }
                                }
                            }
                        }
                        ":info" => {
                            if loaded_source.is_empty() {
                                println!("No code loaded. Use :load <file>");
                            } else {
                                match parse(&loaded_source) {
                                    Ok(mut program) => {
                                        match check(&mut program) {
                                            Ok(result) => {
                                                println!("Functions:");
                                                for func in result.symbols.functions() {
                                                    let pure = if result.effects.is_pure(func.id) { "pure" } else { "effectful" };
                                                    println!("  {} ({}) : {}", func.name, pure, func.ty.display());
                                                }
                                            }
                                            Err(_) => {
                                                eprintln!("Cannot show info due to type errors");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Parse error: {}", e);
                                    }
                                }
                            }
                        }
                        ":clear" => {
                            loaded_source.clear();
                            println!("Cleared");
                        }
                        _ => {
                            println!("Unknown command. Type :help for help.");
                        }
                    }
                } else if !trimmed.is_empty() {
                    // Try to parse as expression or statement
                    let wrapped = format!("_repl() {{ {} }}", trimmed);
                    match parse(&wrapped) {
                        Ok(program) => {
                            println!("Parsed successfully");
                            // TODO: evaluate
                        }
                        Err(e) => {
                            eprintln!("Parse error: {}", e);
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }

    println!("Goodbye!");
}

fn report_parse_error(source: &str, file: &PathBuf, error: &covenant_parser::ParseError) {
    let span = error.span();
    Report::build(ReportKind::Error, file.to_string_lossy().to_string(), span.start)
        .with_message(error.to_string())
        .with_label(
            Label::new((file.to_string_lossy().to_string(), span.start..span.end))
                .with_message(error.to_string())
                .with_color(Color::Red),
        )
        .finish()
        .eprint((file.to_string_lossy().to_string(), Source::from(source)))
        .unwrap();
}
