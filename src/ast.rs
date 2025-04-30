// --- Expressions ---
// Represents anything that computes a value (a number or a series)

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(LiteralValue),
    Variable(String), // e.g., "C", "H", "L", "O", "V", "MA5", "myVar"
    UnaryOp {
        operator: UnaryOperator,
        operand: Box<Expr>, // Box<T> allows recursive types like Expr containing Expr
    },
    BinaryOp {
        left: Box<Expr>,
        operator: BinaryOperator,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,    // e.g., "MA", "REF", "CROSS"
        args: Vec<Expr>, // Arguments to the function
    },
    // Grouped expression like (A + B). Might not be strictly necessary in AST
    // if precedence is handled correctly, but can be useful for clarity or later steps.
    Grouped(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Number(f64),
    // TDX formulas primarily deal with numbers. Strings might appear in function calls
    // like DRAWTEXT("text", ...), but maybe handle that within FunctionCall args?
    // String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)] // Added Eq, Hash for potential use cases
pub enum UnaryOperator {
    Not, // Logical negation (e.g., NOT(C > O))
    Neg, // Arithmetic negation (unary minus, e.g., -C)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    // Arithmetic
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /

    // Comparison
    Gt,    // >
    Lt,    // <
    GtEq,  // >=
    LtEq,  // <=
    Eq,    // =
    NotEq, // <> or != (Lexer produces NotEq for both)

    // Logical
    And, // AND
    Or,  // OR

         // Note: TDX Assignment (:=) is a Statement, not typically a binary expression operator
}

// --- Statements ---
// Represents a complete action or declaration within a formula

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // Variable assignment: VARNAME := expression;
    Assignment {
        variable: String, // Name of the variable being assigned
        value: Expr,      // The expression calculating the value
    },
    // Output: [NAME:] expression [, plot_style1, plot_style2...];
    Output {
        name: Option<String>,   // Optional explicit name (e.g., "MA5")
        expr: Expr,             // The expression to calculate for output/plotting
        styles: Vec<PlotStyle>, // Plotting attributes like COLORRED, LINETHICK2
    },
    // Some formulas might just contain expressions for calculation without explicit
    // assignment or output, although TDX usually requires output lines.
    // Let's hold off on ExpressionStatement unless proven necessary.
    // ExpressionStatement(Expr),
}

// --- Plotting Styles ---
// Representing styles simply for now. Could be an enum later if needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotStyle(pub String); // e.g., PlotStyle("COLORRED"), PlotStyle("LINETHICK2"), PlotStyle("NODRAW")

// --- Top Level Structure ---
// A complete formula is typically a sequence of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub statements: Vec<Statement>,
}
