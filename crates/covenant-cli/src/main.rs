//! Covenant CLI - Command line interface for the Covenant compiler

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ariadne::{Color, Label, Report, ReportKind, Source};

use covenant_parser::parse;
use covenant_symbols::build_symbol_graph;
use covenant_checker::{check, check_effects, EffectError};
use covenant_graph::{GraphBuilder, execute_query, parse_query};
use covenant_codegen::compile_pure;
use covenant_llm::{
    ExplainGenerator, ExplanationCache, LlmClient,
    Verbosity, ExplainFormat, format_explanation,
};
use covenant_requirements::{validate_program, format_report, ReportFormat, filter_uncovered, has_coverage_errors};
use covenant_optimizer::{optimize, OptSettings, OptLevel};

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
        /// Also validate requirement coverage
        #[arg(long)]
        requirements: bool,
    },
    /// Compile a file to WASM
    Compile {
        /// Input file
        file: PathBuf,
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Target platform (browser, node, wasi). Defaults to node.
        #[arg(long, default_value = "node")]
        target: String,
        /// Optimization level (0=none, 1=basic, 2=standard, 3=aggressive)
        #[arg(long, default_value = "0")]
        optimize: u8,
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
    /// Generate AI explanation for code
    Explain {
        /// Input file
        file: PathBuf,
        /// Output format (json, text, markdown, compact)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Verbosity level (minimal, standard, detailed)
        #[arg(short, long, default_value = "standard")]
        verbosity: String,
        /// Disable caching
        #[arg(long)]
        no_cache: bool,
    },
    /// Analyze effect declarations and compute transitive closures
    Effects {
        /// Input file(s) to analyze
        files: Vec<PathBuf>,
        /// Show only violations (errors)
        #[arg(long)]
        violations_only: bool,
    },
    /// Analyze requirement coverage
    Requirements {
        /// Input file(s) to analyze
        files: Vec<PathBuf>,
        /// Output format (text, json, markdown)
        #[arg(long, default_value = "text")]
        report: String,
        /// Show only uncovered requirements
        #[arg(long)]
        uncovered_only: bool,
        /// Exit with error if coverage is below threshold (0-100)
        #[arg(long)]
        min_coverage: Option<f64>,
        /// Treat all uncovered requirements as errors (regardless of priority)
        #[arg(long)]
        strict: bool,
    },
    /// Interactive REPL
    Repl,
    /// Compile and run a file
    Run {
        /// Input file
        file: PathBuf,
        /// Optimization level (0=none, 1=basic, 2=standard, 3=aggressive)
        #[arg(long, default_value = "0")]
        optimize: u8,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, pretty } => cmd_parse(&file, pretty),
        Commands::Check { files, requirements } => cmd_check(&files, requirements),
        Commands::Compile { file, output, target, optimize: opt_level } => cmd_compile(&file, output, &target, opt_level),
        Commands::Query { files, query } => cmd_query(&files, &query),
        Commands::Info { file } => cmd_info(&file),
        Commands::Explain { file, format, verbosity, no_cache } => {
            cmd_explain(&file, &format, &verbosity, no_cache).await;
        }
        Commands::Effects { files, violations_only } => cmd_effects(&files, violations_only),
        Commands::Requirements { files, report, uncovered_only, min_coverage, strict } => {
            cmd_requirements(&files, &report, uncovered_only, min_coverage, strict);
        }
        Commands::Repl => cmd_repl(),
        Commands::Run { file, optimize: opt_level } => cmd_run(&file, opt_level),
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

fn cmd_check(files: &[PathBuf], validate_requirements: bool) {
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
            Ok(program) => {
                // Phase 2: Symbol graph building
                let symbol_result = match build_symbol_graph(&program) {
                    Ok(result) => {
                        // Report deferred errors (undefined references) as warnings
                        for err in &result.deferred_errors {
                            eprintln!("  warning: {}", err);
                        }
                        result
                    }
                    Err(errors) => {
                        eprintln!("✗ {} - {} symbol errors:", file.display(), errors.len());
                        for err in &errors {
                            eprintln!("  {}: {}", err.code(), err);
                        }
                        all_ok = false;
                        continue;
                    }
                };

                // Phase 3-4: Type checking
                match check(&program) {
                    Ok(result) => {
                        let fn_count = result.symbols.functions().count();
                        let pure_count = result.symbols.functions()
                            .filter(|s| result.effects.is_pure(s.id))
                            .count();
                        let symbol_count = symbol_result.graph.len();

                        // Phase 5: Requirement validation (optional)
                        let req_info = if validate_requirements {
                            let req_report = validate_program(&program, None);
                            let has_errors = has_coverage_errors(&req_report);
                            if has_errors {
                                all_ok = false;
                            }
                            Some((req_report.summary.coverage_percent, has_errors))
                        } else {
                            None
                        };

                        // Print status line
                        if let Some((coverage, has_errors)) = req_info {
                            if has_errors {
                                eprintln!(
                                    "✗ {} - {} symbols, {} functions ({} pure), requirements: {:.0}% coverage (errors)",
                                    file.display(),
                                    symbol_count,
                                    fn_count,
                                    pure_count,
                                    coverage
                                );
                            } else {
                                println!(
                                    "✓ {} - {} symbols, {} functions ({} pure), requirements: {:.0}% coverage",
                                    file.display(),
                                    symbol_count,
                                    fn_count,
                                    pure_count,
                                    coverage
                                );
                            }
                        } else {
                            println!(
                                "✓ {} - {} symbols, {} functions ({} pure)",
                                file.display(),
                                symbol_count,
                                fn_count,
                                pure_count
                            );
                        }
                    }
                    Err(errors) => {
                        eprintln!("✗ {} - {} type errors:", file.display(), errors.len());
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

fn cmd_compile(file: &PathBuf, output: Option<PathBuf>, target: &str, opt_level: u8) {
    // Validate target platform
    let valid_targets = ["browser", "node", "wasi"];
    if !valid_targets.contains(&target) {
        eprintln!("Invalid target '{}'. Valid targets: browser, node, wasi", target);
        std::process::exit(1);
    }

    // Map optimization level
    let opt_level = match opt_level {
        0 => OptLevel::O0,
        1 => OptLevel::O1,
        2 => OptLevel::O2,
        _ => OptLevel::O3,
    };

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

    let result = match check(&program) {
        Ok(r) => r,
        Err(errors) => {
            eprintln!("Type check errors:");
            for err in errors {
                eprintln!("  {}", err);
            }
            std::process::exit(1);
        }
    };

    // Run optimizer if level > 0
    if opt_level != OptLevel::O0 {
        let settings = OptSettings {
            level: opt_level,
            emit_warnings: true,
        };

        // Optimize each snippet's body
        if let covenant_ast::Program::Snippets { ref mut snippets, .. } = program {
            for snippet in snippets.iter_mut() {
                // Find the body section and optimize its steps
                for section in snippet.sections.iter_mut() {
                    if let covenant_ast::Section::Body(ref mut body) = section {
                        let opt_result = optimize(&mut body.steps, &settings);

                        // Report warnings
                        for warning in &opt_result.warnings {
                            eprintln!("{}: {}", warning.code, warning.message);
                        }
                    }
                }
            }
        }
    }

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
            Ok(program) => {
                match check(&program) {
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

    let program = match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            report_parse_error(&source, file, &e);
            std::process::exit(1);
        }
    };

    let result = match check(&program) {
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

fn cmd_effects(files: &[PathBuf], violations_only: bool) {
    let mut all_ok = true;
    let mut total_violations = 0;

    for file in files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {}: {}", file.display(), e);
                all_ok = false;
                continue;
            }
        };

        let program = match parse(&source) {
            Ok(p) => p,
            Err(e) => {
                report_parse_error(&source, file, &e);
                all_ok = false;
                continue;
            }
        };

        // Build symbol graph (Phase 2)
        let symbol_result = match build_symbol_graph(&program) {
            Ok(result) => result,
            Err(errors) => {
                eprintln!("✗ {} - {} symbol errors:", file.display(), errors.len());
                for err in &errors {
                    eprintln!("  {}: {}", err.code(), err);
                }
                all_ok = false;
                continue;
            }
        };

        // Run effect checking (Phase 3)
        let result = check_effects(&symbol_result.graph);
        total_violations += result.violations.len();

        if !violations_only {
            println!("File: {}", file.display());
            println!();

            // Group by pure/effectful
            let mut pure_fns = Vec::new();
            let mut effectful_fns = Vec::new();

            for (name, closure) in &result.closures {
                if closure.is_pure {
                    pure_fns.push((name, closure));
                } else {
                    effectful_fns.push((name, closure));
                }
            }

            if !pure_fns.is_empty() {
                println!("Pure functions:");
                for (name, closure) in &pure_fns {
                    let status = if closure.computed.is_empty() { "✓" } else { "✗" };
                    println!("  {} {}", status, name);
                }
                println!();
            }

            if !effectful_fns.is_empty() {
                println!("Effectful functions:");
                for (name, closure) in &effectful_fns {
                    let declared: Vec<_> = closure.declared.iter().collect();
                    let computed: Vec<_> = closure.computed.iter().collect();
                    let status = if closure.declared == closure.computed { "✓" } else { "✗" };
                    println!("  {} {} [declared: {:?}, computed: {:?}]", status, name, declared, computed);
                }
                println!();
            }
        }

        // Report violations
        if !result.violations.is_empty() {
            all_ok = false;
            eprintln!("Effect violations in {}:", file.display());
            for error in &result.violations {
                match error {
                    EffectError::PureCallsEffectful { function, callee, effects, span } => {
                        eprintln!(
                            "  E-EFFECT-001 [{}:{}]: pure function `{}` calls effectful `{}` (effects: {:?})",
                            span.start, span.end, function, callee, effects
                        );
                    }
                    EffectError::MissingEffect { function, missing, source_callee, span } => {
                        eprintln!(
                            "  E-EFFECT-002 [{}:{}]: function `{}` missing effect declarations {:?} (from `{}`)",
                            span.start, span.end, function, missing, source_callee
                        );
                    }
                }
            }
            eprintln!();
        }
    }

    // Summary
    if violations_only {
        if total_violations == 0 {
            println!("No effect violations found");
        } else {
            eprintln!("{} effect violation(s) found", total_violations);
        }
    }

    if !all_ok {
        std::process::exit(1);
    }
}

fn cmd_requirements(
    files: &[PathBuf],
    format_str: &str,
    uncovered_only: bool,
    min_coverage: Option<f64>,
    strict: bool,
) {
    use covenant_requirements::ValidatorConfig;

    let format: ReportFormat = format_str.parse().unwrap_or(ReportFormat::Text);
    let mut all_ok = true;

    // Use strict config if requested (all uncovered = error)
    let config = if strict {
        ValidatorConfig::strict()
    } else {
        ValidatorConfig::default_config()
    };

    for file in files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {}: {}", file.display(), e);
                all_ok = false;
                continue;
            }
        };

        let program = match parse(&source) {
            Ok(p) => p,
            Err(e) => {
                report_parse_error(&source, file, &e);
                all_ok = false;
                continue;
            }
        };

        let report = validate_program(&program, Some(config.clone()));

        // Apply uncovered filter if requested
        let report = if uncovered_only {
            filter_uncovered(&report)
        } else {
            report
        };

        // Output the report
        println!("{}", format_report(&report, format));

        // Check for errors
        if has_coverage_errors(&report) {
            all_ok = false;
        }

        // Check coverage threshold
        if let Some(threshold) = min_coverage {
            if report.summary.coverage_percent < threshold {
                eprintln!(
                    "Coverage {:.1}% is below threshold {:.1}%",
                    report.summary.coverage_percent,
                    threshold
                );
                all_ok = false;
            }
        }
    }

    if !all_ok {
        std::process::exit(1);
    }
}

async fn cmd_explain(file: &PathBuf, format: &str, verbosity: &str, no_cache: bool) {
    // Read and parse the file
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let program = match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            report_parse_error(&source, file, &e);
            std::process::exit(1);
        }
    };

    // Parse verbosity and format
    let verbosity: Verbosity = verbosity.parse().unwrap_or_default();
    let format: ExplainFormat = format.parse().unwrap_or_default();

    // Create LLM client
    let llm = match LlmClient::new() {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Set ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable");
            std::process::exit(1);
        }
    };

    // Create generator with optional cache
    let generator = if no_cache {
        ExplainGenerator::new(llm)
    } else {
        let cache = ExplanationCache::new();
        ExplainGenerator::with_cache(llm, cache)
    };

    // Generate explanations for all snippets in the file
    let snippets = match &program {
        covenant_ast::Program::Snippets { snippets, .. } => snippets,
        covenant_ast::Program::Legacy { .. } => {
            eprintln!("Error: explain command only works with snippet-based files");
            eprintln!("The file uses legacy declaration syntax");
            std::process::exit(1);
        }
    };

    for snippet in snippets {
        match generator.explain(snippet, &source, verbosity).await {
            Ok(explanation) => {
                let output = format_explanation(&explanation, format);
                println!("{}", output);
            }
            Err(e) => {
                eprintln!("Error generating explanation for {}: {}", snippet.id, e);
            }
        }
    }
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
                                    Ok(program) => {
                                        match check(&program) {
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
                                    Ok(program) => {
                                        match check(&program) {
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
                        Ok(_program) => {
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

fn cmd_run(file: &PathBuf, opt_level: u8) {
    use std::process::Command;

    // Map optimization level
    let opt_level = match opt_level {
        0 => OptLevel::O0,
        1 => OptLevel::O1,
        2 => OptLevel::O2,
        _ => OptLevel::O3,
    };

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

    let result = match check(&program) {
        Ok(r) => r,
        Err(errors) => {
            eprintln!("Type check errors:");
            for err in errors {
                eprintln!("  {}", err);
            }
            std::process::exit(1);
        }
    };

    // Run optimizer if level > 0
    if opt_level != OptLevel::O0 {
        let settings = OptSettings {
            level: opt_level,
            emit_warnings: true,
        };

        if let covenant_ast::Program::Snippets { ref mut snippets, .. } = program {
            for snippet in snippets.iter_mut() {
                for section in snippet.sections.iter_mut() {
                    if let covenant_ast::Section::Body(ref mut body) = section {
                        let opt_result = optimize(&mut body.steps, &settings);
                        for warning in &opt_result.warnings {
                            eprintln!("{}: {}", warning.code, warning.message);
                        }
                    }
                }
            }
        }
    }

    // Compile to WASM
    let wasm = match compile_pure(&program, &result.symbols) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Compilation error: {}", e);
            std::process::exit(1);
        }
    };

    // Write to temp file
    let temp_wasm = std::env::temp_dir().join("covenant_run.wasm");
    if let Err(e) = fs::write(&temp_wasm, &wasm) {
        eprintln!("Error writing temp file: {}", e);
        std::process::exit(1);
    }

    // Find the runner script
    // First try relative to executable, then relative to current directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let runner_paths = [
        exe_dir.as_ref().map(|d| d.join("../../host/run.mjs")).unwrap_or_default(),
        PathBuf::from("host/run.mjs"),
        PathBuf::from("../host/run.mjs"),
    ];

    let runner = runner_paths.iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| {
            eprintln!("Error: Could not find host/run.mjs");
            eprintln!("Make sure you're running from the covenant project directory");
            std::process::exit(1);
        });

    // Run with Node.js
    let status = Command::new("node")
        .arg(&runner)
        .arg(&temp_wasm)
        .status();

    // Clean up temp file
    let _ = fs::remove_file(&temp_wasm);

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            std::process::exit(s.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Error running node: {}", e);
            eprintln!("Make sure Node.js is installed and in your PATH");
            std::process::exit(1);
        }
    }
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
