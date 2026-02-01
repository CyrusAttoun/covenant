//! Snippet AST nodes
//!
//! Snippets are the top-level constructs in Covenant IR. Every piece of code
//! is wrapped in a snippet with explicit sections (effects, requires, signature, body, tests, etc.)

use serde::{Deserialize, Serialize};
use crate::{Literal, Span, Type};

/// A complete snippet (top-level IR construct)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub kind: SnippetKind,
    pub notes: Vec<Note>,
    pub sections: Vec<Section>,
    /// For extern-impl: the abstract snippet ID this implements
    pub implements: Option<String>,
    /// For extern-impl: the target platform
    pub platform: Option<String>,
    pub span: Span,
}

/// Snippet kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnippetKind {
    Function,
    Struct,
    Enum,
    Module,
    Database,
    Extern,
    /// Platform-abstract extern declaration (declares interface + supported platforms)
    ExternAbstract,
    /// Platform-specific extern implementation (provides binding for one platform)
    ExternImpl,
    Test,
    Data,
}

/// A note annotation (can be multilingual)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub lang: Option<String>,
    pub content: String,
    pub span: Span,
}

/// Section types within a snippet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Section {
    Effects(EffectsSection),
    Requires(RequiresSection),
    Signature(SignatureSection),
    Body(BodySection),
    Tests(TestsSection),
    Metadata(MetadataSection),
    Relations(RelationsSection),
    Content(ContentSection),
    Schema(SchemaSection),
    Types(TypesSection),
    Tools(ToolsSection),
}

// ===== Effects Section =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsSection {
    pub effects: Vec<EffectDecl>,
    pub span: Span,
}

/// An effect parameter (key=value pair)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectParam {
    pub name: String,
    pub value: Literal,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDecl {
    pub name: String,
    pub params: Vec<EffectParam>,
    pub span: Span,
}

impl EffectDecl {
    /// Check if this effect has a parameter with the given name
    pub fn get_param(&self, name: &str) -> Option<&EffectParam> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Check if this effect has any parameters
    pub fn has_params(&self) -> bool {
        !self.params.is_empty()
    }
}

// ===== Requirements Section =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiresSection {
    pub requirements: Vec<Requirement>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub text: Option<String>,
    pub priority: Option<Priority>,
    pub status: Option<ReqStatus>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReqStatus {
    Draft,
    Approved,
    Implemented,
    Tested,
}

// ===== Signature Section =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureSection {
    pub kind: SignatureKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignatureKind {
    Function(FunctionSignature),
    Struct(StructSignature),
    Enum(EnumSignature),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<ParamDecl>,
    pub returns: Option<ReturnType>,
    pub generics: Vec<GenericParam>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDecl {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReturnType {
    Single { ty: Type, optional: bool },
    Collection { of: Type },
    Union { types: Vec<UnionMember> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnionMember {
    pub ty: Type,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericParam {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructSignature {
    pub name: String,
    pub fields: Vec<SnippetFieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetFieldDecl {
    pub name: String,
    pub ty: Type,
    pub primary: bool,
    pub auto: bool,
    pub unique: bool,
    pub optional: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumSignature {
    pub name: String,
    pub variants: Vec<SnippetVariantDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetVariantDecl {
    pub name: String,
    pub fields: Option<Vec<SnippetFieldDecl>>,
    pub span: Span,
}

// ===== Body Section =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodySection {
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub kind: StepKind,
    pub output_binding: String, // "as" attribute
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepKind {
    Compute(ComputeStep),
    Call(CallStep),
    Query(QueryStep),
    Bind(BindStep),
    Return(ReturnStep),
    If(IfStep),
    Match(MatchStep),
    For(ForStep),
    Insert(InsertStep),
    Update(UpdateStep),
    Delete(DeleteStep),
    Transaction(TransactionStep),
    Traverse(TraverseStep),
    Construct(StructConstruction),
    Parallel(ParallelStep),
    Race(RaceStep),
}

// ===== Step Types =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeStep {
    pub op: Operation,
    pub inputs: Vec<Input>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Comparison
    Equals,
    NotEquals,
    Less,
    Greater,
    LessEq,
    GreaterEq,

    // Logical
    And,
    Or,
    Not,
    Neg,

    // String operations — now extern-abstract calls (text.concat, text.upper, etc.)
    // Removed: Concat, Contains, Slice, Upper, Lower, Trim, TrimStart, TrimEnd,
    // Replace, Split, Join, Repeat, StrLen, ByteLen, IsEmpty, StartsWith, EndsWith,
    // IndexOf, CharAt, StrReverse, PadStart, PadEnd

    // Numeric operations
    Abs,
    Min,
    Max,
    Clamp,
    Pow,
    Sqrt,
    Floor,
    Ceil,
    Round,
    Trunc,
    Sign,

    // Bitwise operations
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    BitShl,
    BitShr,
    BitUshr,

    // Conversion operations
    ToInt,
    ToFloat,
    ToString,
    ParseInt,
    ParseFloat,

    // List operations — now extern-abstract calls (list.len, list.get, etc.)
    // Removed: ListLen, ListGet, ListFirst, ListLast, ListAppend, ListPrepend,
    // ListConcat, ListSlice, ListReverse, ListTake, ListDrop, ListContains,
    // ListIndexOf, ListIsEmpty, ListSort, ListDedup, ListFlatten

    // Map operations (partially converted — MapGet is now map.get extern-abstract)
    MapLen,
    MapHas,
    MapInsert,
    MapRemove,
    MapKeys,
    MapValues,
    MapEntries,
    MapMerge,
    MapIsEmpty,

    // Set operations
    SetLen,
    SetHas,
    SetAdd,
    SetRemove,
    SetUnion,
    SetIntersect,
    SetDiff,
    SetSymmetricDiff,
    SetIsSubset,
    SetIsSuperset,
    SetIsEmpty,
    SetToList,

    // DateTime operations
    DtYear,
    DtMonth,
    DtDay,
    DtHour,
    DtMinute,
    DtSecond,
    DtWeekday,
    DtUnix,
    DtAddDays,
    DtAddHours,
    DtAddMinutes,
    DtAddSeconds,
    DtDiff,
    DtFormat,

    // Bytes operations
    BytesLen,
    BytesGet,
    BytesSlice,
    BytesConcat,
    BytesToString,
    BytesToBase64,
    BytesToHex,
    BytesIsEmpty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub source: InputSource,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputSource {
    Var(String),
    Lit(Literal),
    Field { of: String, field: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStep {
    pub fn_name: String,
    pub args: Vec<CallArg>,
    pub handle: Option<HandleBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallArg {
    pub name: String,
    pub source: InputSource,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleBlock {
    pub cases: Vec<HandleCase>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleCase {
    pub error_type: String,
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStep {
    pub dialect: Option<String>, // None = Covenant, Some("postgres") = SQL
    pub target: String,
    pub content: QueryContent,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryContent {
    Covenant(CovenantQuery),
    Dialect(DialectQuery),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CovenantQuery {
    pub select: SnippetSelectClause,
    pub from: String,
    pub where_clause: Option<Condition>,
    pub order: Option<OrderClause>,
    pub limit: Option<u64>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnippetSelectClause {
    All,
    Field(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub kind: ConditionKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionKind {
    Equals { field: String, value: InputSource },
    Contains { field: String, value: InputSource },
    NotEquals { field: String, value: InputSource },
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
    RelTo { target: String, rel_type: String },
    RelFrom { source: String, rel_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderClause {
    pub field: String,
    pub direction: SnippetOrderDirection,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnippetOrderDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialectQuery {
    pub body: String, // Raw SQL
    pub params: Vec<ParamBinding>,
    pub returns: ReturnType,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamBinding {
    pub name: String,
    pub from: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindStep {
    pub source: BindSource,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BindSource {
    Var(String),
    Lit(Literal),
    Field { of: String, field: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnStep {
    pub value: ReturnValue,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReturnValue {
    Var(String),
    Lit(Literal),
    Struct(StructConstruction),
    Variant(VariantConstruction),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructConstruction {
    pub ty: Type,
    pub fields: Vec<FieldAssignment>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAssignment {
    pub name: String,
    pub value: InputSource,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantConstruction {
    pub ty: String,
    pub fields: Vec<FieldAssignment>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStep {
    pub condition: InputSource,
    pub then_steps: Vec<Step>,
    pub else_steps: Option<Vec<Step>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchStep {
    pub on: String,
    pub cases: Vec<MatchCase>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchCase {
    pub pattern: MatchPattern,
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchPattern {
    Variant { variant: String, bindings: Vec<String> },
    Wildcard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForStep {
    pub var: String,
    pub collection: String,
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertStep {
    pub target: String,
    pub assignments: Vec<FieldAssignment>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStep {
    pub target: String,
    pub assignments: Vec<FieldAssignment>,
    pub where_clause: Option<Condition>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStep {
    pub target: String,
    pub where_clause: Option<Condition>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStep {
    pub isolation: Option<IsolationLevel>,
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraverseStep {
    pub target: String,
    pub from: String,
    pub relation_type: String,
    pub depth: TraverseDepth,
    pub direction: TraverseDirection,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraverseDepth {
    Bounded(u32),
    Unbounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraverseDirection {
    Outgoing,
    Incoming,
    Both,
}

// ===== Structured Concurrency =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelStep {
    pub branches: Vec<Branch>,
    pub on_error: Option<String>,  // "fail_fast", "collect_all", "ignore_errors"
    pub timeout: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceStep {
    pub branches: Vec<Branch>,
    pub on_timeout: Option<String>,  // "cancel", "return_partial"
    pub timeout: Option<String>,
    pub span: Span,
}

// ===== Other Sections (stubs for now) =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestsSection {
    pub tests: Vec<TestDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDecl {
    pub id: String,
    pub kind: TestKind,
    pub covers: Vec<String>,
    pub steps: Vec<Step>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestKind {
    Unit,
    Integration,
    Golden,
    Property,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSection {
    pub entries: Vec<MetadataEntry>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationsSection {
    pub relations: Vec<RelationDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationDecl {
    pub kind: RelationKind,
    pub target: String,
    pub rel_type: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    To,
    From,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSection {
    pub content: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSection {
    pub tables: Vec<SnippetTableDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetTableDecl {
    pub name: String,
    pub fields: Vec<SnippetFieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypesSection {
    pub types: Vec<TypeDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDecl {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsSection {
    pub tools: Vec<ToolDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDecl {
    pub name: String,
    pub contract: String,
    pub span: Span,
}
