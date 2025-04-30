#[derive(Debug, Clone, PartialEq, Copy)] // PartialEq 用于测试
pub enum TokenType {
    // Single-character tokens
    LParen,
    RParen,
    Comma,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Colon, // :
    Gt,
    Lt,
    Eq, // >, <, =

    // One or two character tokens
    ColonAssign, // :=
    GtEq,
    LtEq,
    NotEq, // >=, <=, <> or !=

    // Literals
    Identifier, // e.g., MA, C, myVar
    Number,     // e.g., 10, 3.14
    String,     // e.g., "MACD Line" (if supported)

    // Keywords (need to verify TDX keywords)
    If,
    Then,
    Else, // Example keywords
    And,
    Or,
    Not, // Logical operators as keywords

    // Special
    Illegal, // Represents an unrecognized token
    Eof,     // End of File/Input
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String, // The actual text segment (e.g., "MA", "12.5", "+")
    pub line: usize,    // Line number where the token starts
    pub column: usize,  // Column number where the token starts
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, line: usize, column: usize) -> Self {
        Token {
            token_type,
            lexeme,
            line,
            column,
        }
    }
}
