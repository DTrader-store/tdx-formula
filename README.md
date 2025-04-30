# TDX 公式解析与求值引擎 (Rust 实现)

这是一个基于 Rust 的库，用于解析通达信（TDX）等股票技术分析软件的公式字符串，并对其进行求值，根据历史行情数据计算出相应的指标序列。

**注意：** 本项目处于开发阶段，目前仅支持部分 TDX 公式语法和内置函数。目标是逐步完善对常见 TDX 公式语法的支持。

## 特性

-   解析 TDX 公式字符串，生成抽象语法树（AST）。
-   基于 AST，对输入行情数据进行求值，计算出指标序列。
-   支持 TDX 的内置变量，如 `C` (收盘价), `O` (开盘价), `H` (最高价), `L` (最低价), `V` (成交量)。
-   支持基本的算术运算符 (`+`, `-`, `*`, `/`)。
-   支持基本的比较运算符 (`>`, `<`, `>=`, `<=`, `=`, `<>`)。
-   支持基本的逻辑运算符 (`AND`, `OR`, `NOT`)。
-   支持一元运算符 (`-` 负号, `NOT`)。
-   支持常见的内置函数实现，包括：
    -   移动平均：`MA(系列, 周期)`
    -   引用先前值：`REF(系列, 偏移量)`
    -   交叉判断：`CROSS(系列1, 系列2)`
    -   周期求和：`SUM(系列, 周期)`
    -   周期平均：`AVERAGE(系列, 周期)`
    -   条件判断：`IF(条件, 值1, 值2)`
    -   周期数：`BARSCOUNT(条件)`
    -   周期最高值：`HHV(系列, 周期)`
    -   周期最低值：`LLV(系列, 周期)`
    -   绝对值：`ABS(系列)`
    -   递归移动平均：`SMA(系列, 周期, 平滑系数)`
    -   指数移动平均：`EMA(系列, 周期)`
    -   周期计数：`COUNT(条件, 周期)`
    -   两系列最大值 (逐点)：`MAX(系列1, 系列2)`
    -   两系列最小值 (逐点)：`MIN(系列1, 系列2)`
    -   区间判断：`BETWEEN(系列, A, B)`
-   支持赋值语句 (`VAR := Expression;`)。
-   支持输出语句 (`OutputName: Expression, Style1, Style2...;` 或 `Expression;`)。
-   支持简单的绘图样式解析 (例如 `COLORRED`, `LINETHICK2`)。
-   支持 `{ ... }` 形式的注释。

## 环境要求

*   Rust 编程语言和 Cargo 包管理器。
    可以通过 [rustup](https://rustup.rs/) 官方网站指引进行安装。

## 构建项目

1.  克隆仓库：
    ```bash
    git clone git@github.com:DTrader-store/tdx-formula.git # 替换为你的仓库实际 URL
    cd TDX-Formula
    ```
2.  构建项目：
    在项目根目录下运行：
    ```bash
    cargo build
    ```

## 运行测试

项目包含单元测试来验证词法分析、语法分析和求值逻辑。在项目根目录下运行：

```bash
cargo test
```

## 项目结构

项目的核心逻辑分布在以下几个模块中：

*   `src/ast.rs`: 定义了抽象语法树（AST）的各种节点，代表公式的结构，如表达式（Expr）和语句（Statement）。
*   `src/token.rs`: 定义了词法分析过程中识别的 Token 类型以及包含 token 信息的结构体。
*   `src/lexer.rs`: 实现了词法分析器（Lexer），负责将输入的公式字符串切割成一系列有意义的 Token。
*   `src/parser.rs`: 实现了语法分析器（Parser），负责接收 Lexer 输出的 Token 序列，并根据语法规则构建 AST。使用了 Pratt Parsing（优先级爬升）算法处理表达式的优先级和结合性。
*   `src/data.rs`: 定义了输入行情数据 (`InputData`) 的结构，以及求值器输出的结果结构 (`FormulaResult`, `OutputLineResult`)。
*   `src/evaluator.rs`: 实现了求值器（Evaluator），负责遍历 AST，结合输入的行情数据和计算过程中产生的变量环境，最终计算出指标序列。包含了各种内置函数的具体实现。
*   `src/lib.rs`: 项目的库入口文件，声明并导出了各个模块。

## 如何使用（作为库）

你可以在你的 Rust 项目中添加此库作为依赖（如果尚未发布到 crates.io，可以使用本地路径依赖）：

```toml
# Cargo.toml
[dependencies]
tdx-formula = { path = "path/to/your/cloned/tdx-formula" } # 替换为实际路径
```

然后在你的代码中使用：

```rust
use tdx_formula::lexer::Lexer;
use tdx_formula::parser::Parser;
use tdx_formula::evaluator::Evaluator;
use tdx_formula::data::InputData;
use tdx_formula::ast::{Formula, Statement, Expr}; // 根据需要导入 AST 类型

fn main() {
    // 示例公式：计算收盘价的5日移动平均
    let formula_string = "MA5: MA(C, 5), COLORRED;";

    // 示例输入数据 (实际数据应从行情源获取)
    // 需要确保所有序列（开高低收量等）长度一致
    let num_bars = 10;
    let input_data = InputData::new(
        vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0], // opens
        vec![10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5], // highs
        vec![9.5, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5],   // lows
        vec![10.3, 11.3, 12.3, 13.3, 14.3, 15.3, 16.3, 17.3, 18.3, 19.3], // closes
        vec![100.0, 110.0, 120.0, 130.0, 140.0, 150.0, 160.0, 170.0, 180.0, 190.0], // volumes
    ).expect("Failed to create InputData");


    // 1. 词法分析
    let lexer = Lexer::new(formula_string);

    // 2. 语法分析
    let mut parser = Parser::new(lexer);
    match parser.parse_formula() {
        Ok(formula) => {
            println!("Parsing successful, AST: {:?}", formula);

            // 3. 求值
            let mut evaluator = Evaluator::new(&input_data);
            match evaluator.evaluate_formula(&formula) {
                Ok(result) => {
                    println!("Evaluation successful, Result: {:?}", result);

                    // 示例：找到 MA5 输出线并打印数据
                    if let Some(ma5_line) = result.output_lines.iter().find(|line| line.name == "MA5") {
                        println!("MA5 Data: {:?}", ma5_line.data);
                        println!("MA5 Styles: {:?}", ma5_line.styles);
                    }
                }
                Err(eval_err) => {
                    eprintln!("Evaluation Error: {}", eval_err);
                }
            }
        }
        Err(parse_err) => {
            eprintln!("Parsing Error: {}", parse_err);
        }
    }
}
```

## 贡献

欢迎贡献！如果你发现 bug、有改进建议或想实现更多的内置函数/语法特性，请随时提交 Issue 或 Pull Request。
