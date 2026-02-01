//! Canonical text printer for Covenant AST
//!
//! Serializes AST back to canonical `.cov` text format.
//! Uses two-space indentation and follows the grammar from docs/design/grammar.ebnf.

use crate::{
    BodySection, Branch, CallArg, CallStep, Condition, ConditionKind, ComputeStep,
    ContentSection, CovenantQuery, DeleteStep, DialectQuery, EffectDecl, EffectsSection,
    EnumSignature, FieldAssignment, ForStep, FunctionSignature, HandleBlock, HandleCase,
    IfStep, Input, InputSource, InsertStep, IsolationLevel, MatchCase, MatchPattern,
    MatchStep, MetadataEntry, MetadataSection, Note, Operation, ParallelStep, ParamBinding,
    ParamDecl, Priority, Program, QueryContent, QueryStep, RaceStep, RelationDecl,
    RelationKind, RelationsSection, ReqStatus, Requirement, RequiresSection, ReturnStep,
    ReturnType, ReturnValue, SchemaSection, Section, SignatureKind, SignatureSection,
    Snippet, SnippetFieldDecl, SnippetKind, SnippetOrderDirection, SnippetSelectClause,
    SnippetTableDecl, SnippetVariantDecl, Step, StepKind, StructConstruction,
    StructSignature, TestDecl, TestKind, TestsSection, ToolDecl, ToolsSection,
    TransactionStep, TraverseDepth, TraverseDirection, TraverseStep, TypeDecl, TypesSection,
    UnionMember, UpdateStep, VariantConstruction, BindStep, BindSource,
};
use crate::{Literal, Type, TypeKind, TypePath};

/// Trait for converting AST nodes to canonical Covenant text format.
pub trait ToCov {
    /// Convert to canonical `.cov` format with the given indentation level.
    fn to_cov(&self, indent: usize) -> String;
}

/// Helper to generate indentation string (two spaces per level).
fn indent_str(level: usize) -> String {
    "  ".repeat(level)
}

/// Escape a string for output (double quotes, newlines, etc.)
fn escape_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            _ => result.push(c),
        }
    }
    result
}

// ===== Program =====

impl ToCov for Program {
    fn to_cov(&self, indent: usize) -> String {
        match self {
            Program::Legacy { declarations, .. } => {
                // Legacy mode not supported for canonical output
                format!("{}// Legacy mode not supported for canonical output\n", indent_str(indent))
                    + &declarations
                        .iter()
                        .map(|_| "// declaration\n".to_string())
                        .collect::<Vec<_>>()
                        .join("")
            }
            Program::Snippets { snippets, .. } => snippets
                .iter()
                .map(|s| s.to_cov(indent))
                .collect::<Vec<_>>()
                .join("\n\n"),
        }
    }
}

// ===== Snippet =====

impl ToCov for Snippet {
    fn to_cov(&self, indent: usize) -> String {
        let mut lines = Vec::new();
        let ind = indent_str(indent);

        // Snippet header
        let kind_str = self.kind.to_cov(0);
        lines.push(format!("{}snippet id=\"{}\" kind=\"{}\"", ind, self.id, kind_str));

        // Notes (after header, before sections)
        for note in &self.notes {
            lines.push(note.to_cov(indent));
        }

        // Sections in canonical order: effects, requires, types, tools, signature, body, tests, metadata, relations, content, schema
        let mut effects = Vec::new();
        let mut requires = Vec::new();
        let mut types = Vec::new();
        let mut tools = Vec::new();
        let mut signature = Vec::new();
        let mut body = Vec::new();
        let mut tests = Vec::new();
        let mut metadata = Vec::new();
        let mut relations = Vec::new();
        let mut content = Vec::new();
        let mut schema = Vec::new();

        for section in &self.sections {
            match section {
                Section::Effects(s) => effects.push(s),
                Section::Requires(s) => requires.push(s),
                Section::Types(s) => types.push(s),
                Section::Tools(s) => tools.push(s),
                Section::Signature(s) => signature.push(s),
                Section::Body(s) => body.push(s),
                Section::Tests(s) => tests.push(s),
                Section::Metadata(s) => metadata.push(s),
                Section::Relations(s) => relations.push(s),
                Section::Content(s) => content.push(s),
                Section::Schema(s) => schema.push(s),
            }
        }

        // Add sections in canonical order
        for s in effects {
            lines.push(s.to_cov(indent));
        }
        for s in requires {
            lines.push(s.to_cov(indent));
        }
        for s in types {
            lines.push(s.to_cov(indent));
        }
        for s in tools {
            lines.push(s.to_cov(indent));
        }
        for s in signature {
            lines.push(s.to_cov(indent));
        }
        for s in body {
            lines.push(s.to_cov(indent));
        }
        for s in tests {
            lines.push(s.to_cov(indent));
        }
        for s in metadata {
            lines.push(s.to_cov(indent));
        }
        for s in relations {
            lines.push(s.to_cov(indent));
        }
        for s in content {
            lines.push(s.to_cov(indent));
        }
        for s in schema {
            lines.push(s.to_cov(indent));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for SnippetKind {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            SnippetKind::Function => "fn".to_string(),
            SnippetKind::Struct => "struct".to_string(),
            SnippetKind::Enum => "enum".to_string(),
            SnippetKind::Module => "module".to_string(),
            SnippetKind::Database => "database".to_string(),
            SnippetKind::Extern => "extern".to_string(),
            SnippetKind::ExternAbstract => "extern-abstract".to_string(),
            SnippetKind::ExternImpl => "extern-impl".to_string(),
            SnippetKind::Test => "test".to_string(),
            SnippetKind::Data => "data".to_string(),
        }
    }
}

impl ToCov for Note {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        if let Some(lang) = &self.lang {
            format!("{}note lang=\"{}\" \"{}\"", ind, lang, escape_string(&self.content))
        } else {
            // Check if content has newlines - use triple quotes
            if self.content.contains('\n') {
                format!("{}note \"\"\"\n{}\n{}\"\"\"", ind, self.content, ind)
            } else {
                format!("{}note \"{}\"", ind, escape_string(&self.content))
            }
        }
    }
}

// ===== Effects Section =====

impl ToCov for EffectsSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}effects", ind)];

        for effect in &self.effects {
            lines.push(effect.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for EffectDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        if self.params.is_empty() {
            format!("{}effect {}", ind, self.name)
        } else {
            let params: Vec<String> = self
                .params
                .iter()
                .map(|p| format!("{}={}", p.name, p.value.to_cov(0)))
                .collect();
            format!("{}effect {}({})", ind, self.name, params.join(" "))
        }
    }
}

// ===== Requirements Section =====

impl ToCov for RequiresSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}requires", ind)];

        for req in &self.requirements {
            lines.push(req.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for Requirement {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let ind2 = indent_str(indent + 1);
        let mut lines = vec![format!("{}req id=\"{}\"", ind, self.id)];

        if let Some(text) = &self.text {
            lines.push(format!("{}text \"{}\"", ind2, escape_string(text)));
        }
        if let Some(priority) = &self.priority {
            lines.push(format!("{}priority {}", ind2, priority.to_cov(0)));
        }
        if let Some(status) = &self.status {
            lines.push(format!("{}status {}", ind2, status.to_cov(0)));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for Priority {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            Priority::Critical => "critical".to_string(),
            Priority::High => "high".to_string(),
            Priority::Medium => "medium".to_string(),
            Priority::Low => "low".to_string(),
        }
    }
}

impl ToCov for ReqStatus {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            ReqStatus::Draft => "draft".to_string(),
            ReqStatus::Approved => "approved".to_string(),
            ReqStatus::Implemented => "implemented".to_string(),
            ReqStatus::Tested => "tested".to_string(),
        }
    }
}

// ===== Types Section =====

impl ToCov for TypesSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}types", ind)];

        for ty in &self.types {
            lines.push(ty.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for TypeDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        format!("{}alias name=\"{}\" type=\"{}\"", ind, self.name, self.ty.to_cov(0))
    }
}

// ===== Tools Section =====

impl ToCov for ToolsSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}tools", ind)];

        for tool in &self.tools {
            lines.push(tool.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for ToolDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        format!(
            "{}tool id=\"{}\" contract=\"{}\" end",
            ind, self.name, self.contract
        )
    }
}

// ===== Signature Section =====

impl ToCov for SignatureSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}signature", ind)];

        lines.push(self.kind.to_cov(indent + 1));

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for SignatureKind {
    fn to_cov(&self, indent: usize) -> String {
        match self {
            SignatureKind::Function(f) => f.to_cov(indent),
            SignatureKind::Struct(s) => s.to_cov(indent),
            SignatureKind::Enum(e) => e.to_cov(indent),
        }
    }
}

impl ToCov for FunctionSignature {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let ind2 = indent_str(indent + 1);
        let mut lines = vec![format!("{}fn name=\"{}\"", ind, self.name)];

        for param in &self.params {
            lines.push(param.to_cov(indent + 1));
        }

        if let Some(ret) = &self.returns {
            lines.push(ret.to_cov(indent + 1));
        }

        for generic in &self.generics {
            lines.push(format!("{}generic name=\"{}\"", ind2, generic.name));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for ParamDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        format!("{}param name=\"{}\" type=\"{}\"", ind, self.name, self.ty.to_cov(0))
    }
}

impl ToCov for ReturnType {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        match self {
            ReturnType::Single { ty, optional } => {
                if *optional {
                    format!("{}returns type=\"{}\" optional", ind, ty.to_cov(0))
                } else {
                    format!("{}returns type=\"{}\"", ind, ty.to_cov(0))
                }
            }
            ReturnType::Collection { of } => {
                format!("{}returns collection of=\"{}\"", ind, of.to_cov(0))
            }
            ReturnType::Union { types } => {
                let ind2 = indent_str(indent + 1);
                let mut lines = vec![format!("{}returns union", ind)];
                for member in types {
                    lines.push(member.to_cov(indent + 1));
                }
                lines.push(format!("{}end", ind2));
                lines.join("\n")
            }
        }
    }
}

impl ToCov for UnionMember {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        if self.optional {
            format!("{}type=\"{}\" optional", ind, self.ty.to_cov(0))
        } else {
            format!("{}type=\"{}\"", ind, self.ty.to_cov(0))
        }
    }
}

impl ToCov for StructSignature {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}struct name=\"{}\"", ind, self.name)];

        for field in &self.fields {
            lines.push(field.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for SnippetFieldDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut attrs = Vec::new();

        if self.primary {
            attrs.push("primary_key=true".to_string());
        }
        if self.optional {
            attrs.push("optional".to_string());
        }
        if self.auto {
            attrs.push("auto".to_string());
        }
        if self.unique {
            attrs.push("unique".to_string());
        }

        let attr_str = if attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", attrs.join(" "))
        };

        format!(
            "{}field name=\"{}\" type=\"{}\"{}",
            ind,
            self.name,
            self.ty.to_cov(0),
            attr_str
        )
    }
}

impl ToCov for EnumSignature {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}enum name=\"{}\"", ind, self.name)];

        for variant in &self.variants {
            lines.push(variant.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for SnippetVariantDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);

        if let Some(fields) = &self.fields {
            if fields.is_empty() {
                format!("{}variant name=\"{}\"\n{}end", ind, self.name, ind)
            } else {
                let mut lines = vec![format!("{}variant name=\"{}\"", ind, self.name)];
                for field in fields {
                    lines.push(field.to_cov(indent + 1));
                }
                lines.push(format!("{}end", ind));
                lines.join("\n")
            }
        } else {
            format!("{}variant name=\"{}\"\n{}end", ind, self.name, ind)
        }
    }
}

// ===== Body Section =====

impl ToCov for BodySection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}body", ind)];

        for step in &self.steps {
            lines.push(step.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for Step {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let kind_str = step_kind_name(&self.kind);
        let mut lines = vec![format!("{}step id=\"{}\" kind=\"{}\"", ind, self.id, kind_str)];

        // Add step-specific content
        lines.push(self.kind.to_cov(indent + 1));

        // Add output binding
        lines.push(format!("{}as=\"{}\"", indent_str(indent + 1), self.output_binding));

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

fn step_kind_name(kind: &StepKind) -> &'static str {
    match kind {
        StepKind::Compute(_) => "compute",
        StepKind::Call(_) => "call",
        StepKind::Query(_) => "query",
        StepKind::Bind(_) => "bind",
        StepKind::Return(_) => "return",
        StepKind::If(_) => "if",
        StepKind::Match(_) => "match",
        StepKind::For(_) => "for",
        StepKind::Insert(_) => "insert",
        StepKind::Update(_) => "update",
        StepKind::Delete(_) => "delete",
        StepKind::Transaction(_) => "transaction",
        StepKind::Traverse(_) => "traverse",
        StepKind::Construct(_) => "construct",
        StepKind::Parallel(_) => "parallel",
        StepKind::Race(_) => "race",
    }
}

impl ToCov for StepKind {
    fn to_cov(&self, indent: usize) -> String {
        match self {
            StepKind::Compute(c) => c.to_cov(indent),
            StepKind::Call(c) => c.to_cov(indent),
            StepKind::Query(q) => q.to_cov(indent),
            StepKind::Bind(b) => b.to_cov(indent),
            StepKind::Return(r) => r.to_cov(indent),
            StepKind::If(i) => i.to_cov(indent),
            StepKind::Match(m) => m.to_cov(indent),
            StepKind::For(f) => f.to_cov(indent),
            StepKind::Insert(i) => i.to_cov(indent),
            StepKind::Update(u) => u.to_cov(indent),
            StepKind::Delete(d) => d.to_cov(indent),
            StepKind::Transaction(t) => t.to_cov(indent),
            StepKind::Traverse(t) => t.to_cov(indent),
            StepKind::Construct(c) => c.to_cov(indent),
            StepKind::Parallel(p) => p.to_cov(indent),
            StepKind::Race(r) => r.to_cov(indent),
        }
    }
}

// ===== Step Types =====

impl ToCov for ComputeStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let op_str = self.op.to_cov(0);
        let inputs_str: Vec<String> = self.inputs.iter().map(|i| i.to_cov(0)).collect();
        format!("{}op={} {}", ind, op_str, inputs_str.join(" "))
    }
}

impl ToCov for Operation {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            // Arithmetic
            Operation::Add => "add".to_string(),
            Operation::Sub => "sub".to_string(),
            Operation::Mul => "mul".to_string(),
            Operation::Div => "div".to_string(),
            Operation::Mod => "mod".to_string(),

            // Comparison
            Operation::Equals => "equals".to_string(),
            Operation::NotEquals => "not_equals".to_string(),
            Operation::Less => "less".to_string(),
            Operation::Greater => "greater".to_string(),
            Operation::LessEq => "less_eq".to_string(),
            Operation::GreaterEq => "greater_eq".to_string(),

            // Logical
            Operation::And => "and".to_string(),
            Operation::Or => "or".to_string(),
            Operation::Not => "not".to_string(),
            Operation::Neg => "neg".to_string(),

            // Numeric
            Operation::Abs => "abs".to_string(),
            Operation::Min => "min".to_string(),
            Operation::Max => "max".to_string(),
            Operation::Clamp => "clamp".to_string(),
            Operation::Pow => "pow".to_string(),
            Operation::Sqrt => "sqrt".to_string(),
            Operation::Floor => "floor".to_string(),
            Operation::Ceil => "ceil".to_string(),
            Operation::Round => "round".to_string(),
            Operation::Trunc => "trunc".to_string(),
            Operation::Sign => "sign".to_string(),

            // Bitwise
            Operation::BitAnd => "bit_and".to_string(),
            Operation::BitOr => "bit_or".to_string(),
            Operation::BitXor => "bit_xor".to_string(),
            Operation::BitNot => "bit_not".to_string(),
            Operation::BitShl => "bit_shl".to_string(),
            Operation::BitShr => "bit_shr".to_string(),
            Operation::BitUshr => "bit_ushr".to_string(),

            // Conversion
            Operation::ToInt => "to_int".to_string(),
            Operation::ToFloat => "to_float".to_string(),
            Operation::ToString => "to_string".to_string(),
            Operation::ParseInt => "parse_int".to_string(),
            Operation::ParseFloat => "parse_float".to_string(),

            // Map operations
            Operation::MapLen => "map_len".to_string(),
            Operation::MapHas => "map_has".to_string(),
            Operation::MapInsert => "map_insert".to_string(),
            Operation::MapRemove => "map_remove".to_string(),
            Operation::MapKeys => "map_keys".to_string(),
            Operation::MapValues => "map_values".to_string(),
            Operation::MapEntries => "map_entries".to_string(),
            Operation::MapMerge => "map_merge".to_string(),
            Operation::MapIsEmpty => "map_is_empty".to_string(),

            // Set operations
            Operation::SetLen => "set_len".to_string(),
            Operation::SetHas => "set_has".to_string(),
            Operation::SetAdd => "set_add".to_string(),
            Operation::SetRemove => "set_remove".to_string(),
            Operation::SetUnion => "set_union".to_string(),
            Operation::SetIntersect => "set_intersect".to_string(),
            Operation::SetDiff => "set_diff".to_string(),
            Operation::SetSymmetricDiff => "set_symmetric_diff".to_string(),
            Operation::SetIsSubset => "set_is_subset".to_string(),
            Operation::SetIsSuperset => "set_is_superset".to_string(),
            Operation::SetIsEmpty => "set_is_empty".to_string(),
            Operation::SetToList => "set_to_list".to_string(),

            // DateTime operations
            Operation::DtYear => "dt_year".to_string(),
            Operation::DtMonth => "dt_month".to_string(),
            Operation::DtDay => "dt_day".to_string(),
            Operation::DtHour => "dt_hour".to_string(),
            Operation::DtMinute => "dt_minute".to_string(),
            Operation::DtSecond => "dt_second".to_string(),
            Operation::DtWeekday => "dt_weekday".to_string(),
            Operation::DtUnix => "dt_unix".to_string(),
            Operation::DtAddDays => "dt_add_days".to_string(),
            Operation::DtAddHours => "dt_add_hours".to_string(),
            Operation::DtAddMinutes => "dt_add_minutes".to_string(),
            Operation::DtAddSeconds => "dt_add_seconds".to_string(),
            Operation::DtDiff => "dt_diff".to_string(),
            Operation::DtFormat => "dt_format".to_string(),

            // Bytes operations
            Operation::BytesLen => "bytes_len".to_string(),
            Operation::BytesGet => "bytes_get".to_string(),
            Operation::BytesSlice => "bytes_slice".to_string(),
            Operation::BytesConcat => "bytes_concat".to_string(),
            Operation::BytesToString => "bytes_to_string".to_string(),
            Operation::BytesToBase64 => "bytes_to_base64".to_string(),
            Operation::BytesToHex => "bytes_to_hex".to_string(),
            Operation::BytesIsEmpty => "bytes_is_empty".to_string(),
        }
    }
}

impl ToCov for Input {
    fn to_cov(&self, _indent: usize) -> String {
        self.source.to_cov(0)
    }
}

impl ToCov for InputSource {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            InputSource::Var(name) => format!("input var=\"{}\"", name),
            InputSource::Lit(lit) => format!("input lit={}", lit.to_cov(0)),
            InputSource::Field { of, field } => {
                format!("input field=\"{}\" of=\"{}\"", field, of)
            }
        }
    }
}

impl ToCov for CallStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}fn=\"{}\"", ind, self.fn_name)];

        for arg in &self.args {
            lines.push(arg.to_cov(indent));
        }

        if let Some(handle) = &self.handle {
            lines.push(handle.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for CallArg {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        match &self.source {
            InputSource::Var(v) => format!("{}arg name=\"{}\" from=\"{}\"", ind, self.name, v),
            InputSource::Lit(l) => format!("{}arg name=\"{}\" lit={}", ind, self.name, l.to_cov(0)),
            InputSource::Field { of, field } => {
                format!("{}arg name=\"{}\" from=\"{}.{}\"", ind, self.name, of, field)
            }
        }
    }
}

impl ToCov for HandleBlock {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}handle", ind)];

        for case in &self.cases {
            lines.push(case.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for HandleCase {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}case type=\"{}\"", ind, self.error_type)];

        for step in &self.steps {
            lines.push(step.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for QueryStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        if let Some(dialect) = &self.dialect {
            lines.push(format!("{}dialect=\"{}\"", ind, dialect));
        }

        lines.push(format!("{}target=\"{}\"", ind, self.target));

        match &self.content {
            QueryContent::Covenant(cq) => {
                lines.push(cq.to_cov(indent));
            }
            QueryContent::Dialect(dq) => {
                lines.push(dq.to_cov(indent));
            }
        }

        lines.join("\n")
    }
}

impl ToCov for CovenantQuery {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        // Select clause
        match &self.select {
            SnippetSelectClause::All => lines.push(format!("{}select all", ind)),
            SnippetSelectClause::Field(f) => lines.push(format!("{}select field=\"{}\"", ind, f)),
        }

        // From clause
        lines.push(format!("{}from=\"{}\"", ind, self.from));

        // Where clause
        if let Some(cond) = &self.where_clause {
            lines.push(format!("{}where", ind));
            lines.push(cond.to_cov(indent + 1));
            lines.push(format!("{}end", ind));
        }

        // Order clause
        if let Some(order) = &self.order {
            let dir = match order.direction {
                SnippetOrderDirection::Asc => "asc",
                SnippetOrderDirection::Desc => "desc",
            };
            lines.push(format!("{}order by=\"{}\" dir=\"{}\"", ind, order.field, dir));
        }

        // Limit clause
        if let Some(limit) = self.limit {
            lines.push(format!("{}limit={}", ind, limit));
        }

        lines.join("\n")
    }
}

impl ToCov for Condition {
    fn to_cov(&self, indent: usize) -> String {
        self.kind.to_cov(indent)
    }
}

impl ToCov for ConditionKind {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        match self {
            ConditionKind::Equals { field, value } => {
                let val_str = match value {
                    InputSource::Var(v) => format!("var=\"{}\"", v),
                    InputSource::Lit(l) => format!("lit={}", l.to_cov(0)),
                    InputSource::Field { of, field } => format!("field=\"{}.{}\"", of, field),
                };
                format!("{}equals field=\"{}\" {}", ind, field, val_str)
            }
            ConditionKind::NotEquals { field, value } => {
                let val_str = match value {
                    InputSource::Var(v) => format!("var=\"{}\"", v),
                    InputSource::Lit(l) => format!("lit={}", l.to_cov(0)),
                    InputSource::Field { of, field } => format!("field=\"{}.{}\"", of, field),
                };
                format!("{}not_equals field=\"{}\" {}", ind, field, val_str)
            }
            ConditionKind::Contains { field, value } => {
                let val_str = match value {
                    InputSource::Var(v) => format!("var=\"{}\"", v),
                    InputSource::Lit(l) => format!("lit={}", l.to_cov(0)),
                    InputSource::Field { of, field } => format!("field=\"{}.{}\"", of, field),
                };
                format!("{}contains field=\"{}\" {}", ind, field, val_str)
            }
            ConditionKind::And(left, right) => {
                let mut lines = vec![format!("{}and", ind)];
                lines.push(left.to_cov(indent + 1));
                lines.push(right.to_cov(indent + 1));
                lines.push(format!("{}end", ind));
                lines.join("\n")
            }
            ConditionKind::Or(left, right) => {
                let mut lines = vec![format!("{}or", ind)];
                lines.push(left.to_cov(indent + 1));
                lines.push(right.to_cov(indent + 1));
                lines.push(format!("{}end", ind));
                lines.join("\n")
            }
            ConditionKind::RelTo { target, rel_type } => {
                format!("{}rel_to target=\"{}\" type=\"{}\"", ind, target, rel_type)
            }
            ConditionKind::RelFrom { source, rel_type } => {
                format!("{}rel_from source=\"{}\" type=\"{}\"", ind, source, rel_type)
            }
        }
    }
}

impl ToCov for DialectQuery {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        // Body block with raw SQL
        lines.push(format!("{}body", ind));
        for line in self.body.lines() {
            lines.push(format!("{}{}", indent_str(indent + 1), line));
        }
        lines.push(format!("{}end", ind));

        // Params section
        if !self.params.is_empty() {
            lines.push(format!("{}params", ind));
            for param in &self.params {
                lines.push(param.to_cov(indent + 1));
            }
            lines.push(format!("{}end", ind));
        }

        // Returns type
        lines.push(self.returns.to_cov(indent));

        lines.join("\n")
    }
}

impl ToCov for ParamBinding {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        format!("{}param name=\"{}\" from=\"{}\"", ind, self.name, self.from)
    }
}

impl ToCov for BindStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        match &self.source {
            BindSource::Var(v) => format!("{}from=\"{}\"", ind, v),
            BindSource::Lit(l) => format!("{}lit={}", ind, l.to_cov(0)),
            BindSource::Field { of, field } => format!("{}from=\"{}.{}\"", ind, of, field),
        }
    }
}

impl ToCov for ReturnStep {
    fn to_cov(&self, indent: usize) -> String {
        self.value.to_cov(indent)
    }
}

impl ToCov for ReturnValue {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        match self {
            ReturnValue::Var(v) => format!("{}from=\"{}\"", ind, v),
            ReturnValue::Lit(l) => format!("{}lit={}", ind, l.to_cov(0)),
            ReturnValue::Struct(s) => s.to_cov(indent),
            ReturnValue::Variant(v) => v.to_cov(indent),
        }
    }
}

impl ToCov for StructConstruction {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}type=\"{}\"", ind, self.ty.to_cov(0))];

        for field in &self.fields {
            lines.push(field.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for FieldAssignment {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        match &self.value {
            InputSource::Var(v) => format!("{}field name=\"{}\" from=\"{}\"", ind, self.name, v),
            InputSource::Lit(l) => {
                format!("{}field name=\"{}\" lit={}", ind, self.name, l.to_cov(0))
            }
            InputSource::Field { of, field } => {
                format!("{}field name=\"{}\" from=\"{}.{}\"", ind, self.name, of, field)
            }
        }
    }
}

impl ToCov for VariantConstruction {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}variant type=\"{}\"", ind, self.ty)];

        for field in &self.fields {
            lines.push(field.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for IfStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        // Condition
        let cond_str = match &self.condition {
            InputSource::Var(v) => format!("\"{}\"", v),
            InputSource::Lit(l) => l.to_cov(0),
            InputSource::Field { of, field } => format!("\"{}.{}\"", of, field),
        };
        lines.push(format!("{}condition={}", ind, cond_str));

        // Then branch
        lines.push(format!("{}then", ind));
        for step in &self.then_steps {
            lines.push(step.to_cov(indent + 1));
        }

        // Else branch
        if let Some(else_steps) = &self.else_steps {
            lines.push(format!("{}else", ind));
            for step in else_steps {
                lines.push(step.to_cov(indent + 1));
            }
        }

        lines.join("\n")
    }
}

impl ToCov for MatchStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}on=\"{}\"", ind, self.on)];

        for case in &self.cases {
            lines.push(case.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for MatchCase {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        // Pattern
        let pattern_str = self.pattern.to_cov(0);
        lines.push(format!("{}case {}", ind, pattern_str));

        // Steps
        for step in &self.steps {
            lines.push(step.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for MatchPattern {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            MatchPattern::Wildcard => "wildcard".to_string(),
            MatchPattern::Variant { variant, bindings } => {
                if bindings.is_empty() {
                    format!("variant type=\"{}\"", variant)
                } else {
                    let bindings_str: Vec<String> =
                        bindings.iter().map(|b| format!("\"{}\"", b)).collect();
                    format!(
                        "variant type=\"{}\" bindings=({})",
                        variant,
                        bindings_str.join(",")
                    )
                }
            }
        }
    }
}

impl ToCov for ForStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}var=\"{}\" in=\"{}\"", ind, self.var, self.collection)];

        for step in &self.steps {
            lines.push(step.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for InsertStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}into=\"{}\"", ind, self.target)];

        for assignment in &self.assignments {
            let val_str = match &assignment.value {
                InputSource::Var(v) => format!("from=\"{}\"", v),
                InputSource::Lit(l) => format!("lit={}", l.to_cov(0)),
                InputSource::Field { of, field } => format!("from=\"{}.{}\"", of, field),
            };
            lines.push(format!("{}set field=\"{}\" {}", ind, assignment.name, val_str));
        }

        lines.join("\n")
    }
}

impl ToCov for UpdateStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}target=\"{}\"", ind, self.target)];

        for assignment in &self.assignments {
            let val_str = match &assignment.value {
                InputSource::Var(v) => format!("from=\"{}\"", v),
                InputSource::Lit(l) => format!("lit={}", l.to_cov(0)),
                InputSource::Field { of, field } => format!("from=\"{}.{}\"", of, field),
            };
            lines.push(format!("{}set field=\"{}\" {}", ind, assignment.name, val_str));
        }

        if let Some(cond) = &self.where_clause {
            lines.push(format!("{}where", ind));
            lines.push(cond.to_cov(indent + 1));
            lines.push(format!("{}end", ind));
        }

        lines.join("\n")
    }
}

impl ToCov for DeleteStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}from=\"{}\"", ind, self.target)];

        if let Some(cond) = &self.where_clause {
            lines.push(format!("{}where", ind));
            lines.push(cond.to_cov(indent + 1));
            lines.push(format!("{}end", ind));
        }

        lines.join("\n")
    }
}

impl ToCov for TransactionStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        if let Some(iso) = &self.isolation {
            lines.push(format!("{}isolation=\"{}\"", ind, iso.to_cov(0)));
        }

        for step in &self.steps {
            lines.push(step.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for IsolationLevel {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            IsolationLevel::ReadUncommitted => "read_uncommitted".to_string(),
            IsolationLevel::ReadCommitted => "read_committed".to_string(),
            IsolationLevel::RepeatableRead => "repeatable_read".to_string(),
            IsolationLevel::Serializable => "serializable".to_string(),
        }
    }
}

impl ToCov for TraverseStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![
            format!("{}target=\"{}\"", ind, self.target),
            format!("{}from=\"{}\"", ind, self.from),
            format!("{}follow type=\"{}\"", ind, self.relation_type),
        ];

        match &self.depth {
            TraverseDepth::Bounded(n) => lines.push(format!("{}depth={}", ind, n)),
            TraverseDepth::Unbounded => lines.push(format!("{}depth=unbounded", ind)),
        }

        lines.push(format!("{}direction=\"{}\"", ind, self.direction.to_cov(0)));

        lines.join("\n")
    }
}

impl ToCov for TraverseDirection {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            TraverseDirection::Outgoing => "outgoing".to_string(),
            TraverseDirection::Incoming => "incoming".to_string(),
            TraverseDirection::Both => "both".to_string(),
        }
    }
}

impl ToCov for ParallelStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        if let Some(on_error) = &self.on_error {
            lines.push(format!("{}on_error=\"{}\"", ind, on_error));
        }

        if let Some(timeout) = &self.timeout {
            lines.push(format!("{}timeout={}", ind, timeout));
        }

        for branch in &self.branches {
            lines.push(branch.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for RaceStep {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = Vec::new();

        if let Some(timeout) = &self.timeout {
            lines.push(format!("{}timeout={}", ind, timeout));
        }

        if let Some(on_timeout) = &self.on_timeout {
            lines.push(format!("{}on_timeout=\"{}\"", ind, on_timeout));
        }

        for branch in &self.branches {
            lines.push(branch.to_cov(indent));
        }

        lines.join("\n")
    }
}

impl ToCov for Branch {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}branch id=\"{}\"", ind, self.id)];

        for step in &self.steps {
            lines.push(step.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

// ===== Tests Section =====

impl ToCov for TestsSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}tests", ind)];

        for test in &self.tests {
            lines.push(test.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for TestDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let kind_str = self.kind.to_cov(0);

        let covers_str = if self.covers.is_empty() {
            String::new()
        } else {
            format!(" covers=\"{}\"", self.covers.join(","))
        };

        let mut lines = vec![format!(
            "{}test id=\"{}\" kind=\"{}\"{}",
            ind, self.id, kind_str, covers_str
        )];

        for step in &self.steps {
            lines.push(step.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for TestKind {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            TestKind::Unit => "unit".to_string(),
            TestKind::Integration => "integration".to_string(),
            TestKind::Golden => "golden".to_string(),
            TestKind::Property => "property".to_string(),
        }
    }
}

// ===== Metadata Section =====

impl ToCov for MetadataSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}metadata", ind)];

        for entry in &self.entries {
            lines.push(entry.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for MetadataEntry {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        format!("{}{}=\"{}\"", ind, self.key, escape_string(&self.value))
    }
}

// ===== Relations Section =====

impl ToCov for RelationsSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}relations", ind)];

        for rel in &self.relations {
            lines.push(rel.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for RelationDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let dir = match self.kind {
            RelationKind::To => "to",
            RelationKind::From => "from",
        };

        if let Some(rel_type) = &self.rel_type {
            format!("{}rel {}=\"{}\" type=\"{}\"", ind, dir, self.target, rel_type)
        } else {
            format!("{}rel {}=\"{}\"", ind, dir, self.target)
        }
    }
}

// ===== Content Section =====

impl ToCov for ContentSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);

        // Check if content has newlines - use appropriate quoting
        if self.content.contains('\n') {
            format!("{}content\n\"\"\"\n{}\n\"\"\"\n{}end", ind, self.content, ind)
        } else {
            format!("{}content\n{}  \"{}\"\n{}end", ind, ind, escape_string(&self.content), ind)
        }
    }
}

// ===== Schema Section =====

impl ToCov for SchemaSection {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}schema", ind)];

        for table in &self.tables {
            lines.push(table.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

impl ToCov for SnippetTableDecl {
    fn to_cov(&self, indent: usize) -> String {
        let ind = indent_str(indent);
        let mut lines = vec![format!("{}table name=\"{}\"", ind, self.name)];

        for field in &self.fields {
            lines.push(field.to_cov(indent + 1));
        }

        lines.push(format!("{}end", ind));
        lines.join("\n")
    }
}

// ===== Type and Literal =====

impl ToCov for Type {
    fn to_cov(&self, _indent: usize) -> String {
        self.kind.to_cov(0)
    }
}

impl ToCov for TypeKind {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            TypeKind::Named(path) => path.to_cov(0),
            TypeKind::Optional(inner) => format!("{}?", inner.to_cov(0)),
            TypeKind::List(inner) => format!("{}[]", inner.to_cov(0)),
            TypeKind::Union(types) => types
                .iter()
                .map(|t| t.to_cov(0))
                .collect::<Vec<_>>()
                .join(" | "),
            TypeKind::Tuple(types) => {
                let inner = types
                    .iter()
                    .map(|t| t.to_cov(0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
            TypeKind::Function { params, ret } => {
                let params_str = params
                    .iter()
                    .map(|t| t.to_cov(0))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({}) -> {}", params_str, ret.to_cov(0))
            }
            TypeKind::Struct(fields) => {
                let fields_str = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, f.ty.to_cov(0)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {} }}", fields_str)
            }
        }
    }
}

impl ToCov for TypePath {
    fn to_cov(&self, _indent: usize) -> String {
        let base = self.segments.join("::");
        if self.generics.is_empty() {
            base
        } else {
            let generics_str = self
                .generics
                .iter()
                .map(|t| t.to_cov(0))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{}>", base, generics_str)
        }
    }
}

impl ToCov for Literal {
    fn to_cov(&self, _indent: usize) -> String {
        match self {
            Literal::Int(n) => n.to_string(),
            Literal::Float(n) => {
                // Ensure we output a decimal point for floats
                if n.fract() == 0.0 {
                    format!("{}.0", n)
                } else {
                    n.to_string()
                }
            }
            Literal::String(s) => format!("\"{}\"", escape_string(s)),
            Literal::Bool(b) => b.to_string(),
            Literal::None => "none".to_string(),
        }
    }
}

// ===== Convenience function =====

/// Convert a program to canonical Covenant text format.
pub fn to_cov(program: &Program) -> String {
    program.to_cov(0)
}

/// Convert a single snippet to canonical Covenant text format.
pub fn snippet_to_cov(snippet: &Snippet) -> String {
    snippet.to_cov(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn test_literal_to_cov() {
        assert_eq!(Literal::Int(42).to_cov(0), "42");
        assert_eq!(Literal::Float(3.14).to_cov(0), "3.14");
        assert_eq!(Literal::String("hello".to_string()).to_cov(0), "\"hello\"");
        assert_eq!(Literal::Bool(true).to_cov(0), "true");
        assert_eq!(Literal::None.to_cov(0), "none");
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(escape_string("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_type_to_cov() {
        let int_type = Type {
            kind: TypeKind::Named(TypePath {
                segments: vec!["Int".to_string()],
                generics: vec![],
                span: dummy_span(),
            }),
            span: dummy_span(),
        };
        assert_eq!(int_type.to_cov(0), "Int");

        let optional_type = Type {
            kind: TypeKind::Optional(Box::new(int_type.clone())),
            span: dummy_span(),
        };
        assert_eq!(optional_type.to_cov(0), "Int?");

        let list_type = Type {
            kind: TypeKind::List(Box::new(int_type)),
            span: dummy_span(),
        };
        assert_eq!(list_type.to_cov(0), "Int[]");
    }

    #[test]
    fn test_effect_to_cov() {
        let effect = EffectDecl {
            name: "console".to_string(),
            params: vec![],
            span: dummy_span(),
        };
        assert_eq!(effect.to_cov(0), "effect console");
    }
}
