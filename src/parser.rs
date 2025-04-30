use crate::ast::*; // Import all AST definitions
use crate::lexer::Lexer;
use crate::token::{Token, TokenType};
use std::iter::Peekable; // Import Peekable

pub struct Parser<'input> {
    // Use Peekable to allow looking ahead one token
    lexer: Peekable<Lexer<'input>>,
    // Store the current token being processed for easier access
    current_token: Option<Token>,
    // Optional: Collect errors instead of panicking on the first one
    // errors: Vec<String>,
}

impl<'input> Parser<'input> {
    // Constructor: Initializes the parser and primes the first token
    pub fn new(lexer: Lexer<'input>) -> Self {
        let mut peekable_lexer = lexer.peekable();
        let current = peekable_lexer.next(); // Load the first token from the lexer
        Parser {
            lexer: peekable_lexer,
            current_token: current,
            // errors: Vec::new(),
        }
    }

    // --- Helper Methods ---

    // Consume the current token and advance to the next one
    fn advance(&mut self) {
        self.current_token = self.lexer.next();
    }

    // Peek at the type of the *next* token without consuming the current one
    fn peek_type(&mut self) -> Option<&TokenType> {
        // Peekable::peek returns Option<&Token), map it to Option<&TokenType>
        self.lexer.peek().map(|token| &token.token_type)
    }

    // Get the type of the *current* token
    fn current_type(&self) -> Option<&TokenType> {
        self.current_token.as_ref().map(|token| &token.token_type)
    }

    // Check if the *current* token matches the expected type
    fn check(&self, expected_type: TokenType) -> bool {
        match self.current_type() {
            Some(t) => *t == expected_type,
            None => false, // No current token (EOF)
        }
    }

    // Check if the *next* token matches the expected type
    fn check_peek(&mut self, expected_type: TokenType) -> bool {
        match self.peek_type() {
            Some(t) => *t == expected_type,
            None => false,
        }
    }

    // Expect the current token to be of a specific type.
    // If it is, consume it (advance). If not, return an error.
    fn expect(&mut self, expected_type: TokenType) -> Result<(), String> {
        if self.check(expected_type) {
            self.advance();
            Ok(())
        } else {
            let current_type_str = format!("{:?}", self.current_type());
            let location = self
                .current_token
                .as_ref()
                .map_or("end of input".to_string(), |t| {
                    format!("line {}, column {}", t.line, t.column)
                });
            Err(format!(
                "Syntax Error: Expected token {:?}, but found {} at {}",
                expected_type, current_type_str, location
            ))
            // Alternative: Push to self.errors and maybe try recovery
        }
    }

    // Expect the current token to be an Identifier.
    // If it is, consume it and return its lexeme (name). If not, return an error.
    fn expect_identifier(&mut self) -> Result<String, String> {
        match &self.current_token {
            Some(token) if token.token_type == TokenType::Identifier => {
                let name = token.lexeme.clone();
                self.advance();
                Ok(name)
            }
            _ => {
                let current_type_str = format!("{:?}", self.current_type());
                let location = self
                    .current_token
                    .as_ref()
                    .map_or("end of input".to_string(), |t| {
                        format!("line {}, column {}", t.line, t.column)
                    });
                Err(format!(
                    "Syntax Error: Expected an Identifier, but found {} at {}",
                    current_type_str, location
                ))
            }
        }
    }
}

impl<'input> Parser<'input> {
    // --- Main Parsing Logic ---

    // Top-level function to parse the entire formula into a Formula AST node.
    pub fn parse_formula(&mut self) -> Result<Formula, String> {
        let mut statements = Vec::new();

        loop {
            // 检查 current_token 是否是 None 来判断 EOF
            if self.current_token.is_none() {
                break; // 输入结束，退出循环
            }

            // 跳过开头的分号
            while self.check(TokenType::Semicolon) {
                self.advance();
                // 在跳过分号后，也需要检查是否到达末尾
                if self.current_token.is_none() {
                    break;
                }
            }
            // 如果因为跳过分号而结束，外层 break 会处理
            if self.current_token.is_none() {
                break;
            }

            let stmt_result = self.parse_statement();

            match stmt_result {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    return Err(e);
                }
            }

            // 检查语句后的分号或 EOF
            if self.check(TokenType::Semicolon) {
                self.advance(); // 消耗分号，current_token 可能变为 None
            } else if self.current_token.is_none() {
                // 语句后是 EOF，合法（最后一条语句）
                // 不需要 break，循环顶部的 is_none() 会处理
            } else {
                // 既不是分号，也不是 None (EOF)
                let current_type_str = format!("{:?}", self.current_type());
                let location = self
                    .current_token
                    .as_ref()
                    .map_or("end of input".to_string(), |t| {
                        format!("line {}, column {}", t.line, t.column)
                    });
                let err_msg = format!(
                    "Syntax Error: 语句后应为 ';' 或文件结束, 但找到 {} 在 {}", // 更新错误信息子串
                    current_type_str, location
                );
                return Err(err_msg);
            }
        } // 循环结束

        Ok(Formula { statements })
    }

    // Parse a single statement (either Assignment or Output)
    fn parse_statement(&mut self) -> Result<Statement, String> {
        // Distinguishing statements:
        // 1. IDENTIFIER `:=` ... -> Assignment
        // 2. IDENTIFIER `:` ...  -> Named Output
        // 3. expression ...     -> Unnamed Output (or potentially just calculation)

        // Use check() for current token and check_peek() for the next token
        if self.check(TokenType::Identifier) && self.check_peek(TokenType::ColonAssign) {
            // It's an assignment statement
            self.parse_assignment_statement()
        } else if self.check(TokenType::Identifier) && self.check_peek(TokenType::Colon) {
            // It's a named output statement
            self.parse_output_statement_with_name()
        } else {
            // Assume it's an expression, treated as an unnamed output
            // This path will handle cases starting with numbers, function calls, '(', etc.
            // Need to ensure the current token is actually the start of an expression
            match self.current_type() {
                 Some(&TokenType::Number) |
                 Some(&TokenType::Identifier) |
                 Some(&TokenType::LParen) |
                 Some(&TokenType::Minus) | // Unary minus
                 Some(&TokenType::Not) |
                 Some(&TokenType::If) |
                 Some(&TokenType::And)|
                 Some(&TokenType::Or)=> { // Unary not
                      self.parse_unnamed_output_statement()
                 }
                 _ => {
                     // If it's not the start of an expression, it's an error
                      let current_type_str = format!("{:?}", self.current_type());
                      let location = self.current_token.as_ref().map_or("end of input".to_string(), |t| format!("line {}, column {}", t.line, t.column));
                      Err(format!("Syntax Error: Unexpected token {:?} ({}) at start of statement at {}",
                                  self.current_type().unwrap(), // Safe to unwrap because we matched Some
                                  self.current_token.as_ref().unwrap().lexeme,
                                  location))
                 }
             }
        }
    }

    // Parses: IDENTIFIER := expression
    fn parse_assignment_statement(&mut self) -> Result<Statement, String> {
        // We already know current token is Identifier (from parse_statement logic)
        let variable_name = self.expect_identifier()?; // Consume identifier and get name

        // Expect and consume :=
        self.expect(TokenType::ColonAssign)?;

        // Parse the expression on the right-hand side
        // We start parse_expression with precedence 0 (lowest)
        let value_expr = self.parse_expression(0)?;

        Ok(Statement::Assignment {
            variable: variable_name,
            value: value_expr,
        })
    }

    // Parses: NAME : expression [, STYLE1, STYLE2...]
    fn parse_output_statement_with_name(&mut self) -> Result<Statement, String> {
        // We know current token is Identifier (the name)
        let output_name = self.expect_identifier()?; // Consume name identifier

        // Expect and consume :
        self.expect(TokenType::Colon)?;

        // Parse the main expression for the output
        let expr = self.parse_expression(0)?;

        // Parse optional plot styles that follow, separated by commas
        let styles = self.parse_plot_styles()?;

        Ok(Statement::Output {
            name: Some(output_name),
            expr,
            styles,
        })
    }

    // Parses: expression [, STYLE1, STYLE2...]
    fn parse_unnamed_output_statement(&mut self) -> Result<Statement, String> {
        // Parse the main expression first
        let expr = self.parse_expression(0)?;

        // Parse optional plot styles that follow, separated by commas
        let styles = self.parse_plot_styles()?;

        Ok(Statement::Output {
            name: None, // No explicit name
            expr,
            styles,
        })
    }

    // Helper to parse plot styles: [, STYLE1, STYLE2...]
    fn parse_plot_styles(&mut self) -> Result<Vec<PlotStyle>, String> {
        let mut styles = Vec::new();
        // Plot styles appear after the main expression, separated by commas
        while self.check(TokenType::Comma) {
            self.advance(); // Consume ','

            // Expect an identifier for the plot style name
            // TDX styles like COLORRED, LINETHICK2 look like identifiers
            let style_name = self.expect_identifier()?;
            styles.push(PlotStyle(style_name));
        }
        Ok(styles)
    }

    // --- Expression Parsing (THE CORE CHALLENGE) ---
    // This is where we handle operator precedence and associativity.
    // We'll use Pratt Parsing / Precedence Climbing.

    // Parses an expression respecting operator precedence.
    // `min_precedence` tells the function to only consume operators
    // with precedence >= `min_precedence`.
    fn parse_expression(&mut self, min_precedence: u8) -> Result<Expr, String> {
        // 1. Parse the initial part of the expression (prefix part).
        // This handles literals, variables, grouped exprs `()`, and unary prefix operators (`-`, `NOT`).
        let mut left = self.parse_prefix()?; // `left` will be updated as we find infix operators

        // 2. Loop as long as the *next* operator (current_token) has sufficient precedence.
        // `infix_binding_power` returns (left_bp, right_bp) for an operator token.
        while let Some((left_bp, right_bp)) = self
            .current_token
            .as_ref()
            .and_then(|t| infix_binding_power(&t.token_type))
        {
            // Should we process this operator? Only if its left binding power
            // is greater than or equal to the minimum precedence we're allowed to handle.
            if left_bp < min_precedence {
                break; // Operator precedence is too low, stop processing here.
            }

            // Consume the operator token.
            let operator_token = self.current_token.clone().unwrap(); // We know it's Some from the loop condition
            self.advance();

            // Convert the token type to our AST BinaryOperator enum.
            let operator = self.token_to_binary_operator(&operator_token.token_type)?;

            // Recursively parse the right-hand side of the operator.
            // The precedence level for the recursive call is the operator's *right* binding power.
            // This handles associativity correctly (e.g., for left-assoc `+`, right_bp > left_bp).
            let right = self.parse_expression(right_bp)?;

            // Combine the `left` expression, the `operator`, and the `right` expression
            // into a new `BinaryOp` node, which becomes the new `left` for the next iteration.
            left = Expr::BinaryOp {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        // When the loop finishes (no more operators or lower precedence), return the accumulated `left` expression.
        Ok(left)
    }

    // Parses the "prefix" part of an expression:
    // - Literals (Numbers)
    // - Variables
    // - Function Calls (IDENTIFIER `(` ... `)`)
    // - Grouped Expressions (`(` expression `)`)
    // - Unary Prefix Operators (`-` negation, `NOT`)
    fn parse_prefix(&mut self) -> Result<Expr, String> {
        // 获取当前 token，如果到文件末尾则报错
        let token = self.current_token.clone().ok_or_else(|| {
            let location = self
                .current_token
                .as_ref()
                .map_or("end of input".to_string(), |t| {
                    format!("line {}, column {}", t.line, t.column)
                });
            format!(
                "Syntax Error: Unexpected end of input while parsing expression at {}",
                location
            )
        })?;

        let location = format!("line {}, column {}", token.line, token.column); // 提前捕获位置信息

        match token.token_type {
            // --- 字面量 (数字) ---
            TokenType::Number => {
                self.advance(); // 消耗数字 token
                match token.lexeme.parse::<f64>() {
                    Ok(value) => Ok(Expr::Literal(LiteralValue::Number(value))),
                    Err(e) => Err(format!(
                        "Syntax Error: Invalid number format '{}' at {}: {}",
                        token.lexeme, location, e
                    )),
                }
            }

            // --- 分组表达式 (...) ---
            TokenType::LParen => {
                self.advance(); // 消耗 '('
                let inner_expr = self.parse_expression(0)?; // 递归解析括号内的表达式
                // 期望并消耗匹配的 ')'。提供更清晰的错误信息。
                self.expect(TokenType::RParen)
                    .map_err(|e| format!("Syntax Error: Mismatched parentheses: {}", e))?;
                // 将内部表达式包装在 Grouped 节点中。
                Ok(Expr::Grouped(Box::new(inner_expr)))
            }

            // --- 一元前缀运算符 (- 和 NOT) ---
            // 注意：TokenType::Not 也是关键字，如果后面没有需要它操作的表达式，会在后面的分支处理。
            TokenType::Minus | TokenType::Not => {
                self.advance(); // 消耗运算符 token ('-' 或 'NOT')
                // 转换为 AST 中的 UnaryOperator 枚举
                let operator = self.token_to_unary_operator(&token.token_type)?;

                // 获取该一元运算符的绑定强度（优先级）。
                // 一元运算符通常与其操作数紧密绑定。
                let (_lbp, right_bp) =
                    prefix_binding_power(&token.token_type).ok_or_else(|| {
                        format!(
                            "Internal Error: Missing prefix binding power for {:?} at {}",
                            token.token_type, location
                        )
                    })?;

                // 递归解析操作数表达式，使用运算符的右绑定强度作为递归调用的最低优先级。
                let operand = self.parse_expression(right_bp)?;

                Ok(Expr::UnaryOp {
                    operator,
                    operand: Box::new(operand),
                })
            }

            // --- 标识符 或 可能作为函数名使用的关键字 (如 IF, AND, OR, NOT) ---
            // 这包括普通标识符 (如 MA, SUM, MYVAR, C, H) 和
            // 也兼作函数名的关键字 (如 IF, AND, OR, NOT，当后面跟着括号时)。
            TokenType::Identifier
            | TokenType::If
            | TokenType::And
            | TokenType::Or
            | TokenType::Not => {
                let name_lexeme = token.lexeme.clone(); // 捕获词素 (如 "MA", "IF", "AND")

                // 检查当前 token 后面是否跟着 '('
                if self.check_peek(TokenType::LParen) {
                    // 是函数调用。首先消耗函数名 token。
                    self.advance(); // 消耗标识符或关键字 token (如 'MA', 'IF')
                    self.expect(TokenType::LParen)?; // 消耗 '('

                    let mut args = Vec::new();
                    // 解析参数直到遇到 ')'
                    if !self.check(TokenType::RParen) {
                        loop {
                            // 解析一个参数表达式 (递归调用，从最低优先级开始)
                            let arg_expr = self.parse_expression(0)?;
                            args.push(arg_expr);

                            // 在一个参数后，期望是 ',' 表示还有下一个参数，或者是 ')' 表示参数结束
                            if self.check(TokenType::Comma) {
                                self.advance(); // 消耗 ','
                                // 防止末尾逗号：检查 ',' 后面是否紧跟着 ')'
                                if self.check(TokenType::RParen) {
                                    let trailing_comma_loc = self
                                        .current_token
                                        .as_ref()
                                        .map_or("end of input".to_string(), |t| {
                                            format!("line {}, column {}", t.line, t.column)
                                        });
                                    return Err(format!(
                                        "Syntax Error: Trailing comma in function arguments not allowed before ')' at {}",
                                        trailing_comma_loc
                                    ));
                                }
                            } else {
                                break; // 不是 ',', 参数结束
                            }
                        }
                    }
                    self.expect(TokenType::RParen)?; // 消耗 ')'

                    // 返回 FunctionCall 表达式。使用捕获的词素作为函数名。
                    // 为了与 Evaluator 中的匹配逻辑一致，这里将函数名转换为大写。
                    Ok(Expr::FunctionCall {
                        name: name_lexeme.to_uppercase(),
                        args,
                    })
                } else {
                    // 后面没有跟着 '('. 将其视为一个简单的变量引用。
                    // 这会处理裸露的标识符 (如 'C', 'H', 'MYVAR')。
                    // 也包括了在表达式中没有作为前缀或函数调用的关键字 (如 IF, AND 等)
                    // 如果裸露的关键字在表达式中不合法，可以在这里添加更具体的错误检查。
                    // 暂时为了简化，先允许它们作为变量。
                    self.advance(); // 消耗标识符或关键字 token
                    Ok(Expr::Variable(name_lexeme))
                }
            }

            // --- 不能开始表达式的关键字 (如 THEN, ELSE) ---
            TokenType::Then | TokenType::Else => Err(format!(
                "Syntax Error: Unexpected keyword {:?} ('{}') found when expecting start of an expression at {}",
                token.token_type, token.lexeme, location
            )),

            // --- 任何其他意外的 token ---
            _ => Err(format!(
                "Syntax Error: Unexpected token {:?} ('{}') found when expecting start of an expression at {}",
                token.token_type, token.lexeme, location
            )),
        }
    }

    // --- Operator Mapping Helpers ---

    // Convert token type to BinaryOperator AST node
    fn token_to_binary_operator(&self, token_type: &TokenType) -> Result<BinaryOperator, String> {
        match token_type {
            TokenType::Plus => Ok(BinaryOperator::Add),
            TokenType::Minus => Ok(BinaryOperator::Sub),
            TokenType::Star => Ok(BinaryOperator::Mul),
            TokenType::Slash => Ok(BinaryOperator::Div),
            TokenType::Gt => Ok(BinaryOperator::Gt),
            TokenType::Lt => Ok(BinaryOperator::Lt),
            TokenType::GtEq => Ok(BinaryOperator::GtEq),
            TokenType::LtEq => Ok(BinaryOperator::LtEq),
            TokenType::Eq => Ok(BinaryOperator::Eq),
            TokenType::NotEq => Ok(BinaryOperator::NotEq),
            TokenType::And => Ok(BinaryOperator::And),
            TokenType::Or => Ok(BinaryOperator::Or),
            _ => {
                let location = self
                    .current_token
                    .as_ref()
                    .map_or("unknown location".to_string(), |t| {
                        format!("line {}, column {}", t.line, t.column)
                    });
                Err(format!(
                    "Internal Error: Token {:?} is not a recognized binary operator at {}",
                    token_type, location
                ))
            } // Should not happen if called correctly after infix_binding_power check
        }
    }

    // Convert token type to UnaryOperator AST node
    fn token_to_unary_operator(&self, token_type: &TokenType) -> Result<UnaryOperator, String> {
        match token_type {
            TokenType::Not => Ok(UnaryOperator::Not),
            TokenType::Minus => Ok(UnaryOperator::Neg), // Unary minus
            _ => {
                let location = self
                    .current_token
                    .as_ref()
                    .map_or("unknown location".to_string(), |t| {
                        format!("line {}, column {}", t.line, t.column)
                    });
                Err(format!(
                    "Internal Error: Token {:?} is not a recognized unary operator at {}",
                    token_type, location
                ))
            } // Should not happen if called correctly after prefix_binding_power check
        }
    }
} // end impl Parser

// --- Operator Precedence Definitions (Pratt Parsing Style) ---
// We define "binding power" (bp) for operators. Higher numbers mean tighter binding (higher precedence).
// For infix operators, we need left (lbp) and right (rbp) binding power to handle associativity.
// - Left Associativity (e.g., a + b + c = (a + b) + c): lbp < rbp
// - Right Associativity (e.g., a = b = c = a = (b = c)): lbp > rbp (less common in TDX expr)

// Returns (left_binding_power, right_binding_power) for infix operators
fn infix_binding_power(token_type: &TokenType) -> Option<(u8, u8)> {
    match token_type {
        TokenType::Or => Some((1, 2)),  // Lowest precedence, left-associative
        TokenType::And => Some((3, 4)), // Left-associative
        TokenType::Eq
        | TokenType::NotEq
        | TokenType::Lt
        | TokenType::Gt
        | TokenType::LtEq
        | TokenType::GtEq => Some((5, 6)), // Comparisons, left-associative (check TDX chaining rules)
        TokenType::Plus | TokenType::Minus => Some((7, 8)), // Addition/Subtraction, left-associative
        TokenType::Star | TokenType::Slash => Some((9, 10)), // Multiplication/Division, left-associative (Highest precedence)
        _ => None,                                           // Not an infix operator
    }
}

// Returns ((), right_binding_power) for prefix unary operators. The left () indicates it's prefix.
// The right_bp determines the precedence of the operand it binds to.
fn prefix_binding_power(token_type: &TokenType) -> Option<((), u8)> {
    match token_type {
        TokenType::Not => Some(((), 5)), // Logical NOT, higher than AND/OR, lower than comparison? Check TDX. Let's put it between AND and comparisons.
        TokenType::Minus => Some(((), 11)), // Unary Negation, binds very tightly (higher than mult/div)
        _ => None,                          // Not a prefix operator
    }
    // Note: Adjust precedence (the u8 values) based on precise TDX rules if known.
    // For example, does `NOT A > B` mean `(NOT A) > B` or `NOT (A > B)`?
    // If `NOT (A > B)`, NOT's right BP should be lower than `>`'s left BP.
    // Let's assume `(NOT A) > B`, meaning NOT binds tighter to A. So NOT's right_bp (e.g., 11) > `>`'s left_bp (e.g., 5). My current numbers reflect this.
    // BUT: TDX `NOT` is often a function `NOT(expression)`. If it's *always* a function, it shouldn't be a prefix operator here.
    // Let's tentatively keep `NOT` as a prefix op for `NOT identifier` cases, but function calls are handled by `parse_function_call`. If `NOT` *only* works like `NOT(expr)`, remove it from prefix ops.
    // Revisit TDX specific examples for unary Minus and NOT. Often `-VAR` is allowed, but `NOT VAR` might require `NOT(VAR>0)`.
    // Let's refine based on testing: If `NOT identifier` doesn't parse correctly, maybe `NOT` should only be recognized as an `Identifier` token initially.
    // Let's adjust NOT's precedence to be lower, maybe around logical ops. `prefix_binding_power` for `NOT` -> `Some(((), 3))` perhaps? This needs verification. Let's stick with 11 for now assuming tight binding.
}

#[cfg(test)]
mod tests {
    use super::*; // Import Parser etc.
    use crate::ast::*; // Import AST nodes
    use crate::lexer::Lexer; // Need Lexer to feed the Parser

    // Helper function to parse input and assert the resulting AST
    fn check_parsing(input: &str, expected_formula: Formula) {
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let result = parser.parse_formula();

        match result {
            Ok(parsed_formula) => {
                assert_eq!(parsed_formula, expected_formula, "Input: \"{}\"", input);
            }
            Err(e) => {
                panic!("Parsing failed for input \"{}\": {}", input, e);
            }
        }
    }

    // Helper function to assert that parsing fails with a specific error message pattern
    fn check_parsing_error(input: &str, expected_error_substring: &str) {
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let result = parser.parse_formula();

        match result {
            Ok(formula) => {
                panic!(
                    "Parsing unexpectedly succeeded for input \"{}\". Got: {:?}",
                    input, formula
                );
            }
            Err(e) => {
                assert!(
                    e.contains(expected_error_substring),
                    "Input: \"{}\"\nExpected error containing: \"{}\"\nActual error: \"{}\"",
                    input,
                    expected_error_substring,
                    e
                );
            }
        }
    }

    // Helper to create Boxed expressions easily
    fn box_expr(expr: Expr) -> Box<Expr> {
        Box::new(expr)
    }

    // Helper for number literals
    fn num(val: f64) -> Expr {
        Expr::Literal(LiteralValue::Number(val))
    }

    // Helper for variable identifiers
    fn var(name: &str) -> Expr {
        Expr::Variable(name.to_string())
    }

    #[test]
    fn test_parse_simple_assignment() {
        let input = "MYVAR := 123.45;";
        let expected = Formula {
            statements: vec![Statement::Assignment {
                variable: "MYVAR".to_string(),
                value: num(123.45),
            }],
        };
        check_parsing(input, expected);
    }

    #[test]
    fn test_parse_named_output() {
        let input = "MA5: MA(C, 5), COLORRED, LINETHICK2;";
        let expected = Formula {
            statements: vec![Statement::Output {
                name: Some("MA5".to_string()),
                expr: Expr::FunctionCall {
                    name: "MA".to_string(),
                    args: vec![var("C"), num(5.0)],
                },
                styles: vec![
                    PlotStyle("COLORRED".to_string()),
                    PlotStyle("LINETHICK2".to_string()),
                ],
            }],
        };
        check_parsing(input, expected);
    }

    #[test]
    fn test_parse_unnamed_output() {
        let input = "CROSS(MA(C,5), MA(C,10));";
        let expected = Formula {
            statements: vec![Statement::Output {
                name: None,
                expr: Expr::FunctionCall {
                    name: "CROSS".to_string(),
                    args: vec![
                        Expr::FunctionCall {
                            name: "MA".to_string(),
                            args: vec![var("C"), num(5.0)],
                        },
                        Expr::FunctionCall {
                            name: "MA".to_string(),
                            args: vec![var("C"), num(10.0)],
                        },
                    ],
                },
                styles: vec![], // No styles specified
            }],
        };
        check_parsing(input, expected);
    }

    #[test]
    fn test_parse_operator_precedence() {
        // Test: A + B * C -> should be A + (B * C)
        let input = "RES := A + B * C;";
        let expected = Formula {
            statements: vec![Statement::Assignment {
                variable: "RES".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(var("A")),
                    operator: BinaryOperator::Add,
                    right: box_expr(Expr::BinaryOp {
                        left: box_expr(var("B")),
                        operator: BinaryOperator::Mul,
                        right: box_expr(var("C")),
                    }),
                },
            }],
        };
        check_parsing(input, expected);

        // Test: A * B + C -> should be (A * B) + C
        let input2 = "RES := A * B + C;";
        let expected2 = Formula {
            statements: vec![Statement::Assignment {
                variable: "RES".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(Expr::BinaryOp {
                        left: box_expr(var("A")),
                        operator: BinaryOperator::Mul,
                        right: box_expr(var("B")),
                    }),
                    operator: BinaryOperator::Add,
                    right: box_expr(var("C")),
                },
            }],
        };
        check_parsing(input2, expected2);
    }

    #[test]
    fn test_parse_parentheses_grouping() {
        let input = "RES := (A + B) * C;";
        let expected = Formula {
            statements: vec![Statement::Assignment {
                variable: "RES".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(Expr::Grouped(box_expr(Expr::BinaryOp {
                        left: box_expr(var("A")),
                        operator: BinaryOperator::Add,
                        right: box_expr(var("B")),
                    }))),
                    operator: BinaryOperator::Mul,
                    right: box_expr(var("C")),
                },
            }],
        };
        check_parsing(input, expected);
    }

    #[test]
    fn test_parse_unary_minus() {
        // Test: -A + B -> (-A) + B
        let input = "RES := -A + B;";
        let expected = Formula {
            statements: vec![Statement::Assignment {
                variable: "RES".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(Expr::UnaryOp {
                        operator: UnaryOperator::Neg, // Negation
                        operand: box_expr(var("A")),
                    }),
                    operator: BinaryOperator::Add,
                    right: box_expr(var("B")),
                },
            }],
        };
        check_parsing(input, expected);

        // Test: A + -B -> A + (-B) (Unary binds tighter)
        let input2 = "RES := A + -B;";
        let expected2 = Formula {
            statements: vec![Statement::Assignment {
                variable: "RES".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(var("A")),
                    operator: BinaryOperator::Add,
                    right: box_expr(Expr::UnaryOp {
                        operator: UnaryOperator::Neg,
                        operand: box_expr(var("B")),
                    }),
                },
            }],
        };
        check_parsing(input2, expected2);
    }

    #[test]
    fn test_parse_logical_operators() {
        // Test: A > B AND C < D
        let input = "COND := A > B AND C < D;";
        let expected = Formula {
            statements: vec![Statement::Assignment {
                variable: "COND".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(Expr::BinaryOp {
                        left: box_expr(var("A")),
                        operator: BinaryOperator::Gt,
                        right: box_expr(var("B")),
                    }),
                    operator: BinaryOperator::And,
                    right: box_expr(Expr::BinaryOp {
                        left: box_expr(var("C")),
                        operator: BinaryOperator::Lt,
                        right: box_expr(var("D")),
                    }),
                },
            }],
        };
        check_parsing(input, expected);

        // Test: A AND B OR C -> (A AND B) OR C (Assuming AND higher precedence or left-assoc)
        // Based on our binding powers (AND: 3,4; OR: 1,2), AND binds tighter.
        let input2 = "COND := A AND B OR C;";
        let expected2 = Formula {
            statements: vec![Statement::Assignment {
                variable: "COND".to_string(),
                value: Expr::BinaryOp {
                    left: box_expr(Expr::BinaryOp {
                        left: box_expr(var("A")),
                        operator: BinaryOperator::And,
                        right: box_expr(var("B")),
                    }),
                    operator: BinaryOperator::Or,
                    right: box_expr(var("C")),
                },
            }],
        };
        check_parsing(input2, expected2);
    }

    #[test]
    fn test_parse_multiple_statements() {
        let input =
            "M5 := MA(C, 5); \n MA10: MA(C, 10), COLORRED; {comment} \n OUTPUT: M5 > MA10; ";
        let expected = Formula {
            statements: vec![
                Statement::Assignment {
                    variable: "M5".to_string(),
                    value: Expr::FunctionCall {
                        name: "MA".to_string(),
                        args: vec![var("C"), num(5.0)],
                    },
                },
                Statement::Output {
                    name: Some("MA10".to_string()),
                    expr: Expr::FunctionCall {
                        name: "MA".to_string(),
                        args: vec![var("C"), num(10.0)],
                    },
                    styles: vec![PlotStyle("COLORRED".to_string())],
                },
                Statement::Output {
                    // Assuming "OUTPUT:" gets parsed as name
                    name: Some("OUTPUT".to_string()),
                    expr: Expr::BinaryOp {
                        left: box_expr(var("M5")),
                        operator: BinaryOperator::Gt,
                        right: box_expr(var("MA10")), // Assumes parser knows MA10 is a variable now
                                                      // This highlights the need for semantic analysis or an environment
                                                      // later, but the parser should produce this structure.
                    },
                    styles: vec![],
                },
            ],
        };
        check_parsing(input, expected);
    }

    // --- Error Handling Tests ---

    #[test]
    fn test_parse_error_missing_semicolon() {
        let input = "X := 5 Y := 10"; // Missing semicolon
        // 更新期望错误子串
        check_parsing_error(input, "语句后应为 ';' 或文件结束");
    }

    #[test]
    fn test_parse_error_mismatched_paren() {
        let input = "X := (A + B;";
        check_parsing_error(input, "Expected token RParen"); // Error occurs when expecting ')'
    }

    #[test]
    fn test_parse_error_unexpected_token() {
        let input = "X := A + * B;"; // '*' cannot follow '+' directly
        // The exact error might depend on where it's caught, likely expecting start of expression after '+'
        check_parsing_error(input, "Unexpected token Star");
    }

    #[test]
    fn test_parse_error_incomplete_assignment() {
        let input = "X :="; // Missing expression
        check_parsing_error(input, "Unexpected end of input while parsing expression"); // Or similar EOF error
    }

    #[test]
    fn test_parse_error_bad_function_args() {
        // 子测试 1: MA(C, );
        let input = "MA(C, );"; // Missing arg after comma
        // 更新期望错误子串
        check_parsing_error(
            input,
            "Trailing comma in function arguments not allowed before ')'",
        );

        // 子测试 2: MA(C, 5 (缺少右括号)
        let input2 = "MA(C, 5"; // Missing closing paren
        // 更新期望错误子串
        check_parsing_error(
            input2,
            "Expected token RParen, but found None at end of input",
        );
    }
}
