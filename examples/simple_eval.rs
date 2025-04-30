use tdx_formula::ast::Formula;
use tdx_formula::data::InputData;
use tdx_formula::evaluator::Evaluator;
use tdx_formula::lexer::Lexer;
use tdx_formula::parser::Parser; // 导入 Formula 类型以便打印 AST

fn main() {
    // 1. 定义要解析和求值的 TDX 公式字符串
    // 示例公式：计算收盘价的5日移动平均线，并命名为 MA5，显示为红色。
    let formula_string = "MA5: MA(C, 5), COLORRED;";
    println!("--- TDX Formula Evaluator Example ---");
    println!("Formula: \"{}\"", formula_string);

    // 2. 准备输入行情数据
    // TDX 公式作用于时间序列数据。这里创建一个简单的模拟数据。
    // 实际应用中，这些数据应该从文件、数据库或交易所API获取。
    // 确保所有序列（开、高、低、收、量等）具有相同的长度。
    let num_bars = 15; // 使用较多数据点以便MA有足够的前期数据
    let input_data = InputData::new(
        // opens, highs, lows, closes, volumes
        // 模拟一个简单的上升趋势
        (0..num_bars).map(|i| 10.0 + i as f64).collect(), // opens
        (0..num_bars).map(|i| 10.5 + i as f64 * 1.1).collect(), // highs
        (0..num_bars).map(|i| 9.5 + i as f64 * 0.9).collect(), // lows
        (0..num_bars).map(|i| 10.2 + i as f64).collect(), // closes
        (0..num_bars).map(|i| 100.0 + i as f64 * 10.0).collect(), // volumes
    )
    .expect("Failed to create InputData: series lengths mismatch.");

    println!("\nInput Data (first few bars):");
    println!("O: {:?}", &input_data.opens[0..5]);
    println!("C: {:?}", &input_data.closes[0..5]);
    // ... 打印其他数据或只打印长度等摘要信息

    // 3. 词法分析 (Lexing)
    println!("\n--- Lexing ---");
    let lexer = Lexer::new(formula_string);
    // 可以选择打印 lexer 输出的 token 列表来检查
    // println!("Tokens: {:?}", lexer.collect::<Vec<_>>());

    // 4. 语法分析 (Parsing)
    println!("\n--- Parsing ---");
    // Lexer 是一个 Iterator，Parser::new 接收实现了 Iterator<Item=Token> 的类型
    // 我们创建一个新的 lexer 实例给 parser
    let lexer_for_parser = Lexer::new(formula_string);
    let mut parser = Parser::new(lexer_for_parser);

    match parser.parse_formula() {
        Ok(formula) => {
            println!("Parsing successful.");
            // 打印生成的抽象语法树 (AST)
            println!("Generated AST: {:#?}", formula);

            // 5. 求值 (Evaluation)
            println!("\n--- Evaluation ---");
            // Evaluator 需要输入数据和生成的 AST
            let mut evaluator = Evaluator::new(&input_data);
            match evaluator.evaluate_formula(&formula) {
                Ok(result) => {
                    println!("Evaluation successful.");
                    // 打印求值结果
                    println!("Evaluation Result: {:#?}", result);

                    // 示例：访问并打印特定输出线的数据
                    if let Some(ma5_line) =
                        result.output_lines.iter().find(|line| line.name == "MA5")
                    {
                        println!("\n--- Output Line: {} ---", ma5_line.name);
                        println!(
                            "Data (first 10 values): {:?}",
                            &ma5_line.data[0..10.min(ma5_line.data.len())]
                        );
                        println!("Styles: {:?}", ma5_line.styles);
                    } else {
                        println!("\nOutput line 'MA5' not found in results.");
                    }
                }
                Err(eval_err) => {
                    eprintln!("Evaluation failed: {}", eval_err);
                }
            }
        }
        Err(parse_err) => {
            eprintln!("Parsing failed: {}", parse_err);
        }
    }

    println!("\n--- Example Finished ---");
}
