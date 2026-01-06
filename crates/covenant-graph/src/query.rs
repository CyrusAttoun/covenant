//! Query interface for the reference graph

use std::collections::HashSet;
use covenant_ast::SymbolId;
use covenant_checker::{SymbolTable, Symbol, SymbolKind};
use crate::ReferenceGraph;
use serde::{Deserialize, Serialize};

/// A query against the symbol graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub from: Table,
    pub filter: Option<Filter>,
}

/// The table to query
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Table {
    Functions,
    Types,
    Variables,
    All,
}

/// Filter conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter {
    /// Symbol name equals
    NameEquals(String),
    /// Symbol name contains
    NameContains(String),
    /// Calls the given function
    Calls(String),
    /// Called by the given function
    CalledBy(String),
    /// Has the given effect
    HasEffect(String),
    /// Is pure (no effects)
    IsPure,
    /// Is dead code
    IsDeadCode,
    /// Logical AND
    And(Vec<Filter>),
    /// Logical OR
    Or(Vec<Filter>),
    /// Logical NOT
    Not(Box<Filter>),
}

/// Result of a query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub symbols: Vec<SymbolInfo>,
}

/// Information about a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub type_str: String,
    pub effects: Vec<String>,
    pub calls: Vec<String>,
    pub called_by: Vec<String>,
    pub is_pure: bool,
}

/// Execute a query against the symbol table and reference graph
pub fn execute_query(
    query: &Query,
    symbols: &SymbolTable,
    graph: &ReferenceGraph,
) -> QueryResult {
    let mut results = Vec::new();

    for symbol in symbols.iter() {
        // Filter by table
        let matches_table = match query.from {
            Table::Functions => matches!(symbol.kind, SymbolKind::Function { .. }),
            Table::Types => matches!(symbol.kind, SymbolKind::Type),
            Table::Variables => matches!(symbol.kind, SymbolKind::Variable { .. } | SymbolKind::Parameter),
            Table::All => true,
        };

        if !matches_table {
            continue;
        }

        // Apply filter
        let matches_filter = query
            .filter
            .as_ref()
            .map(|f| evaluate_filter(f, symbol, symbols, graph))
            .unwrap_or(true);

        if matches_filter {
            results.push(symbol_to_info(symbol, symbols, graph));
        }
    }

    QueryResult { symbols: results }
}

fn evaluate_filter(
    filter: &Filter,
    symbol: &Symbol,
    symbols: &SymbolTable,
    graph: &ReferenceGraph,
) -> bool {
    match filter {
        Filter::NameEquals(name) => symbol.name == *name,
        Filter::NameContains(substring) => symbol.name.contains(substring),
        Filter::Calls(callee_name) => {
            if let Some(callee) = symbols.lookup(callee_name) {
                graph.callees_of(symbol.id).contains(&callee.id)
            } else {
                false
            }
        }
        Filter::CalledBy(caller_name) => {
            if let Some(caller) = symbols.lookup(caller_name) {
                graph.callers_of(symbol.id).contains(&caller.id)
            } else {
                false
            }
        }
        Filter::HasEffect(effect_name) => {
            if let Some(effects) = graph.effects.get(&symbol.id) {
                // TODO: Look up effect by name
                !effects.is_empty()
            } else {
                false
            }
        }
        Filter::IsPure => {
            graph.effects.get(&symbol.id).map(|e| e.is_empty()).unwrap_or(true)
        }
        Filter::IsDeadCode => {
            let is_exported = false; // TODO: track exports
            graph.is_dead_code(symbol.id, is_exported, false)
        }
        Filter::And(filters) => filters.iter().all(|f| evaluate_filter(f, symbol, symbols, graph)),
        Filter::Or(filters) => filters.iter().any(|f| evaluate_filter(f, symbol, symbols, graph)),
        Filter::Not(inner) => !evaluate_filter(inner, symbol, symbols, graph),
    }
}

fn symbol_to_info(symbol: &Symbol, symbols: &SymbolTable, graph: &ReferenceGraph) -> SymbolInfo {
    let kind = match &symbol.kind {
        SymbolKind::Function { .. } => "function",
        SymbolKind::Type => "type",
        SymbolKind::Variable { .. } => "variable",
        SymbolKind::Parameter => "parameter",
        SymbolKind::Field => "field",
    };

    let effects: Vec<String> = if let SymbolKind::Function { effects, .. } = &symbol.kind {
        effects.clone()
    } else {
        vec![]
    };

    let calls: Vec<String> = graph
        .callees_of(symbol.id)
        .iter()
        .filter_map(|&id| symbols.get(id).map(|s| s.name.clone()))
        .collect();

    let called_by: Vec<String> = graph
        .callers_of(symbol.id)
        .iter()
        .filter_map(|&id| symbols.get(id).map(|s| s.name.clone()))
        .collect();

    let is_pure = graph.effects.get(&symbol.id).map(|e| e.is_empty()).unwrap_or(true);

    SymbolInfo {
        name: symbol.name.clone(),
        kind: kind.to_string(),
        type_str: symbol.ty.display(),
        effects,
        calls,
        called_by,
        is_pure,
    }
}

/// Parse a simple query string like "select * from functions where calls contains 'foo'"
pub fn parse_query(input: &str) -> Option<Query> {
    let input = input.trim().to_lowercase();

    // Very basic parser for demo purposes
    if !input.starts_with("select") {
        return None;
    }

    let from = if input.contains("from functions") {
        Table::Functions
    } else if input.contains("from types") {
        Table::Types
    } else if input.contains("from variables") {
        Table::Variables
    } else {
        Table::All
    };

    let filter = if let Some(where_pos) = input.find("where") {
        let where_clause = &input[where_pos + 5..].trim();
        parse_where_clause(where_clause)
    } else {
        None
    };

    Some(Query { from, filter })
}

fn parse_where_clause(clause: &str) -> Option<Filter> {
    // Handle "calls contains 'name'"
    if clause.contains("calls contains") {
        if let Some(name) = extract_quoted_string(clause) {
            return Some(Filter::Calls(name));
        }
    }

    // Handle "called_by = []" or "calledby = []"
    if clause.contains("called_by = []") || clause.contains("calledby = []") {
        return Some(Filter::IsDeadCode);
    }

    // Handle "is_pure = true"
    if clause.contains("is_pure = true") || clause.contains("ispure = true") {
        return Some(Filter::IsPure);
    }

    // Handle "effects contains 'name'"
    if clause.contains("effects contains") {
        if let Some(name) = extract_quoted_string(clause) {
            return Some(Filter::HasEffect(name));
        }
    }

    // Handle "name = 'value'"
    if clause.contains("name =") {
        if let Some(name) = extract_quoted_string(clause) {
            return Some(Filter::NameEquals(name));
        }
    }

    None
}

fn extract_quoted_string(s: &str) -> Option<String> {
    let start = s.find('\'')?;
    let end = s[start + 1..].find('\'')?;
    Some(s[start + 1..start + 1 + end].to_string())
}
