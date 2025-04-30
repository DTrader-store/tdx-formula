use crate::token::{Token, TokenType};

pub struct Lexer<'input> {
    source: &'input str,                                       // 输入的源代码字符串
    chars: std::iter::Peekable<std::str::CharIndices<'input>>, // 字符迭代器，带有一个字符的预读能力
    position: usize,                                           // 当前字符的字节索引
    read_position: usize,                                      // 下一个要读取的字符的字节索引
    current_char: Option<char>,                                // 当前正在查看的字符
    line: usize,                                               // 当前行号
    column: usize,                                             // 当前列号 (相对于当前行的起始)
    line_start_pos: usize, // 当前行起始字符在 source 中的字节索引
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        let mut lexer = Lexer {
            source: input,
            chars: input.char_indices().peekable(),
            position: 0,
            read_position: 0,
            current_char: None,
            line: 1,
            column: 0, // 列号从 0 或 1 开始，保持一致即可
            line_start_pos: 0,
        };
        lexer.read_char(); // 初始化第一个字符
        lexer
    }

    // 读取下一个字符并更新位置
    fn read_char(&mut self) {
        if let Some((pos, ch)) = self.chars.next() {
            self.position = pos;
            self.current_char = Some(ch);
            // 计算列号：当前位置减去行起始位置
            // 注意：这对于多字节UTF-8字符可能不精确地代表“视觉列”数，
            // 但对于错误报告来说，字节偏移量通常足够，或者需要更复杂的列计算。
            // 简单的做法是假设每个char占一列，除非是换行符。
            if ch == '\n' {
                self.line += 1;
                self.line_start_pos = self.position + 1; // 下一行从下一个字符开始
                self.column = 0;
            } else {
                // 简单的列计算，可能需要根据实际需求调整
                self.column = self.position - self.line_start_pos + 1;
            }
        } else {
            self.position = self.source.len();
            self.current_char = None; // Reached EOF
            self.column = self.position - self.line_start_pos + 1;
        }
        // 更新 read_position (指向下一个字符的开始位置)
        self.read_position = if let Some(&(pos, _)) = self.chars.peek() {
            pos
        } else {
            self.source.len()
        };
    }

    // "偷看"下一个字符，但不消耗它
    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, ch)| ch)
    }

    // 跳过空白字符
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() {
                self.read_char();
            } else {
                break;
            }
        }
    }

    // 跳过注释 { ... }
    fn skip_comment(&mut self) {
        // 假设注释不能嵌套
        while let Some(ch) = self.current_char {
            if ch == '}' {
                self.read_char(); // Consume the '}'
                break;
            }
            self.read_char(); // Consume character inside comment
            // 可以在这里添加对 EOF 的检查，防止未闭合的注释导致死循环
            if self.current_char.is_none() {
                // Handle unclosed comment error if necessary
                break;
            }
        }
    }

    // 识别标识符或关键字
    fn read_identifier(&mut self) -> String {
        let start_pos = self.position;
        // 假设标识符由字母、数字、下划线组成，且以字母或下划线开头
        while let Some(ch) = self.current_char {
            if is_identifier_char(ch) {
                self.read_char();
            } else {
                break;
            }
        }
        self.source[start_pos..self.position].to_string()
    }

    // 识别数字 (处理 .5 的情况)
    fn read_number(&mut self) -> String {
        let start_pos = self.position;
        let mut has_dot = false;
        let mut read_something = false; // 标记是否读取到了数字或小数点

        // 处理以小数点开头的情况
        if self.current_char == Some('.') {
            // 检查小数点后面是否跟数字
            if self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
                has_dot = true;
                self.read_char(); // 消耗小数点
                read_something = true;
            } else {
                // 小数点后面不是数字，这个小数点不构成数字开头，
                // read_number 不处理它，交给 next_token 处理为 Illegal
                return String::new();
            }
        } else if self.current_char.map_or(false, |c| c.is_ascii_digit()) {
            // 以数字开头的情况
            read_something = true;
            self.read_char(); // 消耗第一个数字
        } else {
            // 既不是 '.' 开头也不是数字开头，不是数字字面量，直接返回空
            return String::new();
        }

        // 继续读取数字和小数点
        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                self.read_char();
            } else if ch == '.' && !has_dot {
                // 遇到第二个 '.' 或者已经处理过 '.' 开头，但当前不是 '.'
                if self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
                    has_dot = true;
                    self.read_char();
                } else {
                    break; // 点后面不是数字，停止读取 (处理 5. 的情况，'.' 留给 next_token)
                }
            } else {
                break; // 不是数字也不是有效的 '.'
            }
        }

        self.source[start_pos..self.position].to_string()
    }

    // 主函数：获取下一个 Token
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace(); // 跳过开头的空白

        let start_line = self.line;
        let start_col = self.column;
        let start_pos = self.position; // 记录 token 开始的精确位置

        let current_char = self.current_char; // 复制一份当前字符，避免在 match 中多次借用可变 self

        let token: Token = match current_char {
            Some('{') => {
                self.read_char(); // Consume '{'
                self.skip_comment();
                // 跳过注释后，需要递归调用 next_token 来获取真正的下一个 token
                // 或者在一个循环中处理，直到获得一个非空白/非注释的 token
                return self.next_token(); // 简单递归处理
            }
            Some('=') => {
                self.read_char();
                Token::new(TokenType::Eq, "=".to_string(), start_line, start_col)
            }
            Some('+') => {
                self.read_char();
                Token::new(TokenType::Plus, "+".to_string(), start_line, start_col)
            }
            Some('-') => {
                self.read_char();
                Token::new(TokenType::Minus, "-".to_string(), start_line, start_col)
            }
            Some('*') => {
                self.read_char();
                Token::new(TokenType::Star, "*".to_string(), start_line, start_col)
            }
            Some('/') => {
                self.read_char();
                Token::new(TokenType::Slash, "/".to_string(), start_line, start_col)
            }
            Some('(') => {
                self.read_char();
                Token::new(TokenType::LParen, "(".to_string(), start_line, start_col)
            }
            Some(')') => {
                self.read_char();
                Token::new(TokenType::RParen, ")".to_string(), start_line, start_col)
            }
            Some(',') => {
                self.read_char();
                Token::new(TokenType::Comma, ",".to_string(), start_line, start_col)
            }
            Some(';') => {
                self.read_char();
                Token::new(TokenType::Semicolon, ";".to_string(), start_line, start_col)
            }
            Some(':') => {
                if self.peek_char() == Some('=') {
                    self.read_char(); // Consume ':'
                    self.read_char(); // Consume '='
                    Token::new(
                        TokenType::ColonAssign,
                        ":=".to_string(),
                        start_line,
                        start_col,
                    )
                } else {
                    self.read_char(); // Consume ':'
                    Token::new(TokenType::Colon, ":".to_string(), start_line, start_col)
                }
            }
            Some('>') => {
                if self.peek_char() == Some('=') {
                    self.read_char();
                    self.read_char();
                    Token::new(TokenType::GtEq, ">=".to_string(), start_line, start_col)
                } else {
                    self.read_char();
                    Token::new(TokenType::Gt, ">".to_string(), start_line, start_col)
                }
            }
            Some('<') => {
                if self.peek_char() == Some('=') {
                    self.read_char();
                    self.read_char();
                    Token::new(TokenType::LtEq, "<=".to_string(), start_line, start_col)
                } else if self.peek_char() == Some('>') {
                    self.read_char();
                    self.read_char();
                    Token::new(TokenType::NotEq, "<>".to_string(), start_line, start_col)
                } else {
                    self.read_char();
                    Token::new(TokenType::Lt, "<".to_string(), start_line, start_col)
                }
            }
            Some(ch) => {
                if is_identifier_start(ch) {
                    let ident = self.read_identifier();
                    let token_type = lookup_ident(&ident);
                    Token::new(token_type, ident, start_line, start_col)
                } else if ch.is_ascii_digit()
                    || (ch == '.' && self.peek_char().map_or(false, |c| c.is_ascii_digit()))
                {
                    // <--- 优化这里的判断，只在 '.' 后面是数字时才尝试 read_number
                    let number_lexeme = self.read_number();
                    // read_number 只有在成功读到数字部分时才返回非空
                    Token::new(TokenType::Number, number_lexeme, start_line, start_col)
                }
                // else if ch.is_ascii_digit() || ch == '.' { // 这是之前的逻辑，现在简化判断
                //     let number_lexeme = self.read_number();
                //     if !number_lexeme.is_empty() {
                //         Token::new(TokenType::Number, number_lexeme, start_line, start_col)
                //     } else {
                //         let lexeme = ch.to_string();
                //         self.read_char();
                //         Token::new(TokenType::Illegal, lexeme, start_line, start_col)
                //     }
                // }
                else {
                    // 处理其他无法识别的字符
                    let lexeme = ch.to_string();
                    self.read_char();
                    Token::new(TokenType::Illegal, lexeme, start_line, start_col)
                }
            }
            None => {
                // 文件结束
                Token::new(TokenType::Eof, "".to_string(), self.line, self.column)
            }
        };

        token
    }
}

// 辅助函数：判断字符是否可以作为标识符的开头
fn is_identifier_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

// 辅助函数：判断字符是否可以作为标识符的一部分（除开头外）
fn is_identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

// 辅助函数：将标识符映射到关键字 TokenType，如果不是关键字则返回 Identifier
fn lookup_ident(ident: &str) -> TokenType {
    // 通达信关键字通常不区分大小写，所以先转为大写或小写比较
    let uppercase_ident = ident.to_uppercase();
    match uppercase_ident.as_str() {
        "IF" => TokenType::If,
        "THEN" => TokenType::Then,
        "ELSE" => TokenType::Else,
        "AND" => TokenType::And,
        "OR" => TokenType::Or,
        "NOT" => TokenType::Not,
        // 添加其他通达信特有的关键字，如 DRAWTEXT, MA, REF 等，
        // 但注意：MA, REF 等更像是函数名，通常应被识别为 Identifier，
        // 除非它们有特殊的语法含义，否则不一定需要作为单独的 Keyword Token。
        // 这个需要根据后续 Parser 的设计来决定。暂时只列出逻辑和流程控制关键字。
        _ => TokenType::Identifier,
    }
}

// 可以为 Lexer 实现 Iterator trait，使其更容易使用
impl<'input> Iterator for Lexer<'input> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        if token.token_type == TokenType::Eof {
            None // Iterator结束
        } else {
            Some(token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*; // 导入 Lexer, Token, TokenType 等
    use crate::token::TokenType; // 显式导入 TokenType

    // Helper function to easily create expected tokens (ignoring line/col for simplicity in basic tests)
    fn check_lexing(input: &str, expected_tokens: Vec<TokenType>) {
        let lexer = Lexer::new(input);
        // Collect all tokens EXCEPT the final Eof token for easier comparison
        let tokens: Vec<TokenType> = lexer.map(|token| token.token_type).collect();

        assert_eq!(tokens, expected_tokens, "Input: \"{}\"", input);
    }

    // Helper function to compare tokens including lexemes (still ignoring location)
    fn check_lexing_with_lexemes(input: &str, expected_tokens_data: Vec<(TokenType, &str)>) {
        let lexer = Lexer::new(input);
        let tokens: Vec<(TokenType, String)> = lexer.map(|t| (t.token_type, t.lexeme)).collect();
        // Convert expected &str lexemes to String for comparison
        let expected: Vec<(TokenType, String)> = expected_tokens_data
            .into_iter()
            .map(|(tt, s)| (tt, s.to_string()))
            .collect();

        assert_eq!(tokens, expected, "Input: \"{}\"", input);
    }

    #[test]
    fn test_simple_operators_and_punctuation() {
        let input = "+-*/=(),:;";
        let expected = vec![
            TokenType::Plus,
            TokenType::Minus,
            TokenType::Star,
            TokenType::Slash,
            TokenType::Eq,
            TokenType::LParen,
            TokenType::RParen,
            TokenType::Comma,
            TokenType::Colon,
            TokenType::Semicolon,
        ];
        check_lexing(input, expected);
    }

    #[test]
    fn test_multi_char_operators() {
        let input = ":= >= <= <>"; // Using <> for NotEq
        let expected = vec![
            TokenType::ColonAssign,
            TokenType::GtEq,
            TokenType::LtEq,
            TokenType::NotEq,
        ];
        check_lexing(input, expected);

        // Test potential ambiguity with single char ops
        let input = "> = < = : =";
        let expected_ambig = vec![
            TokenType::Gt,
            TokenType::Eq,
            TokenType::Lt,
            TokenType::Eq,
            TokenType::Colon,
            TokenType::Eq,
        ];
        check_lexing(input, expected_ambig);
    }

    #[test]
    fn test_identifiers_and_numbers() {
        let input = "VAR1 C ma5 123 45.67";
        let expected = vec![
            (TokenType::Identifier, "VAR1"),
            (TokenType::Identifier, "C"),
            (TokenType::Identifier, "ma5"), // Assuming case-sensitive identifiers unless lookup_ident handles it
            (TokenType::Number, "123"),
            (TokenType::Number, "45.67"),
        ];
        check_lexing_with_lexemes(input, expected);
    }

    #[test]
    fn test_keywords() {
        // Assumes lookup_ident converts to uppercase for comparison
        let input = "IF Then ELSE and OR not";
        let expected = vec![
            TokenType::If,
            TokenType::Then,
            TokenType::Else,
            TokenType::And,
            TokenType::Or,
            TokenType::Not,
        ];
        check_lexing(input, expected.clone());

        // Test mixed case keywords
        let input_mixed = "If tHeN elSE AnD oR NOt";
        check_lexing(input_mixed, expected); // Should still work if lookup_ident uses to_uppercase()

        // Test identifier that looks like keyword start/end
        let input_ident = "ORDER IFTHEN";
        let expected_ident = vec![
            (TokenType::Identifier, "ORDER"),
            (TokenType::Identifier, "IFTHEN"), // Should not be split or confused with keywords
        ];
        check_lexing_with_lexemes(input_ident, expected_ident);
    }

    #[test]
    fn test_comments_and_whitespace() {
        let input = " { This is a comment } C { another } := { nested? No. } 5.0 ; ";
        let expected = vec![
            (TokenType::Identifier, "C"),
            (TokenType::ColonAssign, ":="),
            (TokenType::Number, "5.0"),
            (TokenType::Semicolon, ";"),
        ];
        check_lexing_with_lexemes(input, expected);
    }

    #[test]
    fn test_unclosed_comment() {
        // The current lexer's skip_comment might loop indefinitely or stop at EOF.
        // Let's test it consumes until EOF without panic.
        // A robust lexer might return an error or a special token.
        // Our current one likely just stops.
        let input = "VAR { unclosed comment";
        let expected = vec![
            (TokenType::Identifier, "VAR"),
            // Lexer stops here after consuming the rest
        ];
        check_lexing_with_lexemes(input, expected);
        // Consider adding specific error handling/token for unclosed comments later.
    }

    #[test]
    fn test_simple_assignment() {
        let input = "MYVAR := C + 10;";
        let expected = vec![
            (TokenType::Identifier, "MYVAR"),
            (TokenType::ColonAssign, ":="),
            (TokenType::Identifier, "C"),
            (TokenType::Plus, "+"),
            (TokenType::Number, "10"),
            (TokenType::Semicolon, ";"),
        ];
        check_lexing_with_lexemes(input, expected);
    }

    #[test]
    fn test_function_call_lexing() {
        let input = "MA5: MA(CLOSE, 5);";
        let expected = vec![
            (TokenType::Identifier, "MA5"),
            (TokenType::Colon, ":"),
            (TokenType::Identifier, "MA"),
            (TokenType::LParen, "("),
            (TokenType::Identifier, "CLOSE"),
            (TokenType::Comma, ","),
            (TokenType::Number, "5"),
            (TokenType::RParen, ")"),
            (TokenType::Semicolon, ";"),
        ];
        check_lexing_with_lexemes(input, expected);
    }

    #[test]
    fn test_illegal_character() {
        // Assumes Lexer produces Illegal token for unrecognized chars
        let input = "VAR @ 10";
        let expected = vec![
            (TokenType::Identifier, "VAR"),
            (TokenType::Illegal, "@"), // Expecting '@' to be illegal
            (TokenType::Number, "10"),
        ];
        check_lexing_with_lexemes(input, expected);
    }

    #[test]
    fn test_number_starting_with_dot() {
        // 现在预期 .5 被解析为 Number(".5")
        let input = ".5";
        let expected = vec![(TokenType::Number, ".5")]; // 修改预期
        check_lexing_with_lexemes(input, expected);

        // 测试 5. - 预期不变: Number("5"), Illegal(".")
        let input_dot_end = "5.";
        let expected_dot_end = vec![(TokenType::Number, "5"), (TokenType::Illegal, ".")];
        check_lexing_with_lexemes(input_dot_end, expected_dot_end);

        // 新增测试：单独的 '.' 应该还是 Illegal
        let input_dot_only = ".";
        let expected_dot_only = vec![(TokenType::Illegal, ".")];
        check_lexing_with_lexemes(input_dot_only, expected_dot_only);

        // 新增测试：'.' 开头但后面不是数字
        let input_dot_alpha = ".A";
        let expected_dot_alpha = vec![
            (TokenType::Illegal, "."),
            (TokenType::Identifier, "A"), // '.' 被识别为 Illegal，然后 'A' 被识别为 Identifier
        ];
        check_lexing_with_lexemes(input_dot_alpha, expected_dot_alpha);
    }
}
