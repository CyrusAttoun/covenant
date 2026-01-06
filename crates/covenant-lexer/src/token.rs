//! Token definitions for Covenant

use logos::Logos;

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"[ \t\r\n\f]+")]  // Skip whitespace
#[logos(skip r"//[^\n]*")]      // Skip line comments
pub enum TokenKind {
    // === Keywords ===
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("match")]
    Match,
    #[token("return")]
    Return,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("type")]
    Type,
    #[token("module")]
    Module,
    #[token("import")]
    Import,
    #[token("from")]
    From,
    #[token("extern")]
    Extern,
    #[token("effect")]
    Effect,
    #[token("database")]
    Database,
    #[token("table")]
    Table,
    #[token("ensures")]
    Ensures,
    #[token("handle")]
    Handle,

    // Query keywords
    #[token("query")]
    Query,
    #[token("select")]
    Select,
    #[token("insert")]
    Insert,
    #[token("update")]
    Update,
    #[token("delete")]
    Delete,
    #[token("set")]
    Set,
    #[token("where")]
    Where,
    #[token("order")]
    Order,
    #[token("by")]
    By,
    #[token("limit")]
    Limit,
    #[token("offset")]
    Offset,
    #[token("join")]
    Join,
    #[token("on")]
    On,
    #[token("inner")]
    Inner,
    #[token("left")]
    Left,
    #[token("right")]
    Right,
    #[token("outer")]
    Outer,
    #[token("as")]
    As,
    #[token("asc")]
    Asc,
    #[token("desc")]
    Desc,
    #[token("into")]
    Into,
    #[token("contains")]
    Contains,

    // Database column attributes
    #[token("primary")]
    Primary,
    #[token("unique")]
    Unique,
    #[token("nullable")]
    Nullable,
    #[token("auto")]
    Auto,
    #[token("index")]
    Index,
    #[token("foreign")]
    Foreign,
    #[token("connection")]
    Connection,

    // Literals
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("none")]
    None,

    // === Operators ===
    // IMPORTANT: = is equality, := is assignment
    #[token("=")]
    Eq,
    #[token(":=")]
    ColonEq,
    #[token("!=")]
    Ne,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token(">")]
    Gt,
    #[token(">=")]
    Ge,

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,

    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("::")]
    ColonColon,

    // === Delimiters ===
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("|")]
    Pipe,

    // === Punctuation ===
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token(".")]
    Dot,
    #[token("?")]
    Question,

    // === Literals ===
    #[regex(r"[0-9]+", priority = 2)]
    Int,

    #[regex(r"[0-9]+\.[0-9]+")]
    Float,

    #[regex(r#""([^"\\]|\\.)*""#)]
    String,

    // === Identifiers ===
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    // === Special ===
    Error,
    Eof,
}

impl TokenKind {
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Let
                | TokenKind::Mut
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::For
                | TokenKind::In
                | TokenKind::Match
                | TokenKind::Return
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Type
                | TokenKind::Module
                | TokenKind::Import
                | TokenKind::From
                | TokenKind::Extern
                | TokenKind::Effect
                | TokenKind::Database
                | TokenKind::Table
                | TokenKind::Query
                | TokenKind::Select
                | TokenKind::Insert
                | TokenKind::Update
                | TokenKind::Delete
        )
    }

    pub fn describe(&self) -> &'static str {
        match self {
            TokenKind::Let => "'let'",
            TokenKind::Mut => "'mut'",
            TokenKind::If => "'if'",
            TokenKind::Else => "'else'",
            TokenKind::For => "'for'",
            TokenKind::In => "'in'",
            TokenKind::Match => "'match'",
            TokenKind::Return => "'return'",
            TokenKind::Struct => "'struct'",
            TokenKind::Enum => "'enum'",
            TokenKind::Type => "'type'",
            TokenKind::Module => "'module'",
            TokenKind::Import => "'import'",
            TokenKind::From => "'from'",
            TokenKind::Extern => "'extern'",
            TokenKind::Effect => "'effect'",
            TokenKind::Database => "'database'",
            TokenKind::Table => "'table'",
            TokenKind::Ensures => "'ensures'",
            TokenKind::Handle => "'handle'",
            TokenKind::Query => "'query'",
            TokenKind::Select => "'select'",
            TokenKind::Insert => "'insert'",
            TokenKind::Update => "'update'",
            TokenKind::Delete => "'delete'",
            TokenKind::Set => "'set'",
            TokenKind::Where => "'where'",
            TokenKind::Order => "'order'",
            TokenKind::By => "'by'",
            TokenKind::Limit => "'limit'",
            TokenKind::Offset => "'offset'",
            TokenKind::Join => "'join'",
            TokenKind::On => "'on'",
            TokenKind::Inner => "'inner'",
            TokenKind::Left => "'left'",
            TokenKind::Right => "'right'",
            TokenKind::Outer => "'outer'",
            TokenKind::As => "'as'",
            TokenKind::Asc => "'asc'",
            TokenKind::Desc => "'desc'",
            TokenKind::Into => "'into'",
            TokenKind::Contains => "'contains'",
            TokenKind::Primary => "'primary'",
            TokenKind::Unique => "'unique'",
            TokenKind::Nullable => "'nullable'",
            TokenKind::Auto => "'auto'",
            TokenKind::Index => "'index'",
            TokenKind::Foreign => "'foreign'",
            TokenKind::Connection => "'connection'",
            TokenKind::True => "'true'",
            TokenKind::False => "'false'",
            TokenKind::None => "'none'",
            TokenKind::Eq => "'='",
            TokenKind::ColonEq => "':='",
            TokenKind::Ne => "'!='",
            TokenKind::Lt => "'<'",
            TokenKind::Le => "'<='",
            TokenKind::Gt => "'>'",
            TokenKind::Ge => "'>='",
            TokenKind::Plus => "'+'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Percent => "'%'",
            TokenKind::AndAnd => "'&&'",
            TokenKind::OrOr => "'||'",
            TokenKind::Bang => "'!'",
            TokenKind::Arrow => "'->'",
            TokenKind::FatArrow => "'=>'",
            TokenKind::ColonColon => "'::'",
            TokenKind::LParen => "'('",
            TokenKind::RParen => "')'",
            TokenKind::LBrace => "'{'",
            TokenKind::RBrace => "'}'",
            TokenKind::LBracket => "'['",
            TokenKind::RBracket => "']'",
            TokenKind::Pipe => "'|'",
            TokenKind::Comma => "','",
            TokenKind::Colon => "':'",
            TokenKind::Semicolon => "';'",
            TokenKind::Dot => "'.'",
            TokenKind::Question => "'?'",
            TokenKind::Int => "integer",
            TokenKind::Float => "float",
            TokenKind::String => "string",
            TokenKind::Ident => "identifier",
            TokenKind::Error => "error",
            TokenKind::Eof => "end of file",
        }
    }
}
