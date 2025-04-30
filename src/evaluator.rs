use crate::ast::*; // 导入 AST 定义
use crate::data::{FormulaResult, InputData, OutputLineResult}; // 导入数据结构
use std::borrow::Cow;
use std::collections::HashMap; // 用于存储变量环境 // 用于灵活处理借用或拥有数据
use std::f64; // 引入 f64 才能使用 f64::NAN // 导入 ApproxEq trait 和 F64Margin 用于测试

// 变量环境：映射变量名到其对应的时间序列数据
// 时间序列数据用 Vec<f64> 表示
type Environment = HashMap<String, Vec<f64>>;

pub struct Evaluator<'data> {
    // 持有对输入数据的引用
    // 使用 Cow<[f64]> 或直接 &'data [f64] 表示输入序列可能更灵活/高效
    // 但为了与 Vec<f64> 的变量环境保持一致，这里先用 Vec<f64> 的引用数组表示原始输入
    input_data: &'data InputData,

    // 求值过程中生成的变量和内置函数结果都存在这个环境里
    environment: Environment,

    // 收集最终的输出结果
    output_lines: Vec<OutputLineResult>,
}

impl<'data> Evaluator<'data> {
    // 构造函数
    // 需要传入输入数据
    pub fn new(input_data: &'data InputData) -> Self {
        let mut env: Environment = HashMap::new();

        // 初始化内置变量到环境
        // 这里是借用 InputData 中的 Vec<f64>，并克隆到环境中。
        // 如果需要避免克隆，环境可以存储 &'data [f64]，但这会让 evaluate_expr 返回类型复杂
        // (需要处理借用生命周期)，所以先克隆，后续可以优化。
        env.insert("O".to_string(), input_data.opens.clone());
        env.insert("H".to_string(), input_data.highs.clone());
        env.insert("L".to_string(), input_data.lows.clone());
        env.insert("C".to_string(), input_data.closes.clone());
        env.insert("V".to_string(), input_data.volumes.clone());
        // TODO: 添加其他通达信内置变量如 VOL, AMOUNT 等

        Evaluator {
            input_data,
            environment: env,
            output_lines: Vec::new(),
        }
    }

    // 主入口：求值整个公式
    pub fn evaluate_formula(&mut self, formula: &Formula) -> Result<FormulaResult, String> {
        // 遍历公式中的所有语句并执行
        for statement in &formula.statements {
            // execute_statement 会修改环境或产生输出
            if let Some(output_line) = self.execute_statement(statement)? {
                self.output_lines.push(output_line);
            }
        }

        // 返回最终的结果
        Ok(FormulaResult::new(self.output_lines.clone())) // Clone results for return
    }

    // 执行单个语句 (赋值或输出)
    fn execute_statement(
        &mut self,
        statement: &Statement,
    ) -> Result<Option<OutputLineResult>, String> {
        match statement {
            Statement::Assignment { variable, value } => {
                // 求值右侧表达式
                let result_series = self.evaluate_expr(value)?;
                // 将结果存入环境
                self.environment.insert(variable.clone(), result_series);
                Ok(None) // 赋值语句不直接产生输出线
            }
            Statement::Output { name, expr, styles } => {
                // 求值输出表达式
                let result_series = self.evaluate_expr(expr)?;
                // 获取输出线名称 (如果有明确指定则用，否则可以用默认或从变量名推断)
                // 暂时用提供的 name 或者默认 "Unnamed Output"
                let output_name = name
                    .as_ref()
                    .cloned()
                    .unwrap_or("Unnamed Output".to_string());

                // 构建并返回输出线结果
                let output_line = OutputLineResult {
                    name: output_name,
                    data: result_series,
                    styles: styles.clone(), // Clone styles
                };
                Ok(Some(output_line)) // 输出语句产生输出线
            }
        }
    }

    // 求值单个表达式
    // 这是递归的核心函数
    fn evaluate_expr(&self, expr: &Expr) -> Result<Vec<f64>, String> {
        match expr {
            Expr::Literal(LiteralValue::Number(val)) => {
                // 数字字面量需要扩展为与输入数据等长的序列
                let num_bars = self.input_data.num_bars;
                Ok(vec![*val; num_bars]) // 创建一个 num_bars 长度的 Vec，所有元素都是 val
            }
            Expr::Variable(name) => {
                // 在环境中查找变量值
                self.environment
                    .get(name)
                    .cloned() // 克隆 Vec<f64> 返回
                    .ok_or_else(|| format!("Runtime Error: Undefined variable '{}'", name))
            }
            Expr::UnaryOp { operator, operand } => {
                let operand_series = self.evaluate_expr(operand)?;
                let result_series = operand_series
                    .into_iter()
                    .map(|val| match operator {
                        UnaryOperator::Neg => -val,
                        UnaryOperator::Not => {
                            if val != 0.0 {
                                0.0
                            } else {
                                1.0
                            }
                        }
                    })
                    .collect::<Vec<f64>>(); // collect directly produces Vec<f64>

                Ok(result_series) // Wrap the Vec<f64> in Ok()
            }
            Expr::BinaryOp {
                left,
                operator,
                right,
            } => {
                let left_series = self.evaluate_expr(left)?;
                let right_series = self.evaluate_expr(right)?;

                // TODO: 需要处理左右序列长度不一致的情况
                // 简单的处理是取最短长度，或者更复杂的对齐规则
                let len = std::cmp::min(left_series.len(), right_series.len());

                let mut result_series = Vec::with_capacity(len);
                for i in 0..len {
                    let l = left_series[i];
                    let r = right_series[i];
                    let result = match operator {
                        BinaryOperator::Add => l + r,
                        BinaryOperator::Sub => l - r,
                        BinaryOperator::Mul => l * r,
                        BinaryOperator::Div => {
                            // 处理除零错误
                            if r == 0.0 {
                                // TDX 可能返回 NaN 或某个特定值，这里返回 NaN
                                f64::NAN
                            } else {
                                l / r
                            }
                        }
                        // 关系运算符返回 1.0 (真) 或 0.0 (假)
                        BinaryOperator::Gt => {
                            if l > r {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        BinaryOperator::Lt => {
                            if l < r {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        BinaryOperator::GtEq => {
                            if l >= r {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        BinaryOperator::LtEq => {
                            if l <= r {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        BinaryOperator::Eq => {
                            if l == r {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        BinaryOperator::NotEq => {
                            if l != r {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        // 逻辑运算符 (Assuming 0.0 is false, non-zero is true)
                        BinaryOperator::And => {
                            if l != 0.0 && r != 0.0 {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        BinaryOperator::Or => {
                            if l != 0.0 || r != 0.0 {
                                1.0
                            } else {
                                0.0
                            }
                        }
                    };
                    result_series.push(result);
                }
                Ok(result_series)
            }
            Expr::FunctionCall { name, args } => {
                // 求值所有参数
                let mut arg_series: Vec<Vec<f64>> = Vec::with_capacity(args.len());
                for arg_expr in args {
                    arg_series.push(self.evaluate_expr(arg_expr)?);
                }

                // 根据函数名调用对应的实现
                // 需要实现各种内置函数
                match name.to_uppercase().as_str() {
                    // TDX 函数名通常不区分大小写
                    "MA" => self.builtin_ma(&arg_series),
                    "REF" => self.builtin_ref(&arg_series),
                    "CROSS" => self.builtin_cross(&arg_series),

                    // --- 添加对新内置函数的匹配 ---
                    "SUM" => self.builtin_sum(&arg_series),
                    "AVERAGE" => self.builtin_average(&arg_series),
                    "BARSCOUNT" => self.builtin_barscount(&arg_series),
                    "HHV" => self.builtin_hhv(&arg_series),
                    "LLV" => self.builtin_llv(&arg_series),
                    "ABS" => self.builtin_abs(&arg_series),
                    "SMA" => self.builtin_sma(&arg_series), // Simple Moving Average (Recursive)
                    "COUNT" => self.builtin_count(&arg_series), // Count occurrences
                    "MAX" => self.builtin_max_series(&arg_series), // Max of two series (element-wise)
                    "MIN" => self.builtin_min_series(&arg_series), // Min of two series (element-wise)
                    "BETWEEN" => self.builtin_between(&arg_series), // Between bounds
                    "EMA" => self.builtin_ema(&arg_series), // Exponential Moving Average (SMA with M=2)

                    // TODO: 添加其他内置函数
                    other_name => Err(format!(
                        "Runtime Error: Unrecognized function '{}'",
                        other_name
                    )),
                }
            }
            Expr::Grouped(inner_expr) => {
                // 直接求值内部表达式
                self.evaluate_expr(inner_expr)
            }
        }
    }

    // --- 内置函数实现 ---

    // 求和: SUM(系列, 周期)
    // 计算序列在指定周期内的累积和
    fn builtin_sum(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: SUM function requires 2 arguments.".to_string());
        }
        let series = &args[0];
        let period_arg = &args[1];
        // 周期参数通常是标量，取其序列的第一个值。
        // TODO: 验证周期参数的有效性 (整数、非负)
        let period = period_arg.get(0).map_or(0, |&p| p as usize);

        if period == 0 || period > series.len() {
            // SUM(..., 0) 的行为需要确认，TDX 周期参数如果 >len 行为也需要确认
            // 暂定周期为 0 或大于长度时，返回全 NaN 或原始系列或错误
            // 假设周期为0返回原始系列，大于长度返回NaN
            if period == 0 {
                return Ok(series.clone());
            } else {
                return Ok(vec![f64::NAN; series.len()]);
            }
        }

        let mut result = Vec::with_capacity(series.len());
        let mut current_sum: f64 = 0.0;

        for i in 0..series.len() {
            if i < period - 1 {
                // 前 period-1 个值无定义 (或累积和不足周期)
                result.push(f64::NAN); // TDX 累积和通常是这样，或者从第一个值开始累积
            // 如果从第一个值开始累积，需要修改逻辑
            } else if i == period - 1 {
                // 第一个完整的周期和
                current_sum = series[0..period].iter().sum();
                result.push(current_sum);
            } else {
                // 滚动计算和
                current_sum = current_sum + series[i] - series[i - period];
                result.push(current_sum);
            }
        }
        Ok(result)
    }

    // 平均值: AVERAGE(系列, 周期)
    // 通常 Average(X, N) == SUM(X, N) / N
    fn builtin_average(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        // 直接复用 SUM 的逻辑
        let sum_series = self.builtin_sum(args)?;

        if args.len() < 2 {
            // 已在 builtin_sum 中检查，这里冗余一下
            return Err("Runtime Error: AVERAGE function requires 2 arguments.".to_string());
        }
        let period_arg = &args[1];
        let period = period_arg.get(0).map_or(0.0, |&p| p); // 周期作为 f64

        if period <= 0.0 {
            // 周期为 0 或负数，通常返回 NaN 或错误
            // 假设返回 NaN 序列
            return Ok(vec![f64::NAN; sum_series.len()]);
        }

        // 将求和结果除以周期
        let result: Vec<f64> = sum_series
            .into_iter()
            .map(|s| {
                if s.is_nan() { f64::NAN } else { s / period } // 保持 NaN
            })
            .collect();

        Ok(result)
    }

    // 周期数: BARSCOUNT(条件)
    // 计算从第一根 K 线到条件首次成立的周期数
    fn builtin_barscount(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        // 修正 f664 -> f64
        if args.len() != 1 {
            return Err(
                "Runtime Error: BARSCOUNT function requires 1 argument (a condition series)."
                    .to_string(),
            );
        }
        let condition_series = &args[0]; // 条件系列 (非零为真，零为假)

        let mut result = Vec::with_capacity(condition_series.len());

        for i in 0..condition_series.len() {
            // 在索引0到i的范围内查找第一个非零值
            let first_true_idx =
                (0..=i).find(|&j| condition_series[j] != 0.0 && !condition_series[j].is_nan());

            match first_true_idx {
                Some(idx) => {
                    // 如果找到，结果是当前索引减去首次成立索引+1
                    result.push((i - idx + 1) as f64);
                }
                None => {
                    // 如果条件从未成立过，结果是当前索引+1
                    result.push((i + 1) as f64);
                }
            }
        }
        Ok(result)
    }

    // 最高值: HHV(系列, 周期)
    // 计算系列在指定周期内的最高值
    fn builtin_hhv(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: HHV function requires 2 arguments.".to_string());
        }
        let series = &args[0];
        let period_arg = &args[1];
        let period = period_arg.get(0).map_or(0, |&p| p as usize);

        if period == 0 || period > series.len() {
            // 周期为 0 或大于长度，返回 NaN 序列 (待确认 TDX 行为)
            return Ok(vec![f64::NAN; series.len()]);
        }

        let mut result = Vec::with_capacity(series.len());

        for i in 0..series.len() {
            if i < period - 1 {
                // 前 period-1 个值无定义
                result.push(f64::NAN);
            } else {
                // 在当前点 i 及其之前的 period-1 个点中找最高值
                // 窗口是 series[i - (period - 1) ..= i]
                let start_idx = i - (period - 1);
                let max_val = series[start_idx..=i]
                    .iter()
                    .cloned() // 克隆 f64 才能求 max
                    .fold(f64::NAN, f64::max); // 使用 fold + max 处理 NaN

                result.push(max_val);
            }
        }
        Ok(result)
    }

    // 最低值: LLV(系列, 周期)
    // 计算系列在指定周期内的最低值
    fn builtin_llv(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: LLV function requires 2 arguments.".to_string());
        }
        let series = &args[0];
        let period_arg = &args[1];
        let period = period_arg.get(0).map_or(0, |&p| p as usize);

        if period == 0 || period > series.len() {
            return Ok(vec![f64::NAN; series.len()]);
        }

        let mut result = Vec::with_capacity(series.len());

        for i in 0..series.len() {
            if i < period - 1 {
                result.push(f64::NAN);
            } else {
                let start_idx = i - (period - 1);
                let min_val = series[start_idx..=i]
                    .iter()
                    .cloned()
                    .fold(f64::NAN, f64::min); // 使用 fold + min 处理 NaN

                result.push(min_val); // 修正：添加这行
            }
        }
        Ok(result)
    }

    // 绝对值: ABS(系列)
    // 计算系列每个元素的绝对值
    fn builtin_abs(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 1 {
            return Err("Runtime Error: ABS function requires 1 argument.".to_string());
        }
        let series = &args[0];
        // 直接对每个元素调用 abs()
        let result: Vec<f64> = series.iter().map(|&val| val.abs()).collect();
        Ok(result)
    }

    // 移动平均 (递归): SMA(系列, 周期, 平滑系数)
    // TDX SMA(X, N, M): Y = (M*X + (N-M)*Yesterday's Y) / N
    // 通常 M=1: Y = (X + (N-1)*Yesterday's Y) / N
    fn builtin_sma(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 3 {
            return Err("Runtime Error: SMA function requires 3 arguments (series, period, smoothing factor).".to_string());
        }
        let series = &args[0];
        let period_arg = &args[1];
        let smoothing_arg = &args[2];

        let n = period_arg.get(0).map_or(0, |&p| p as usize);
        let m = smoothing_arg.get(0).map_or(0.0, |&m| m);

        if n == 0 || n > series.len() {
            // Period 0 or > len usually results in NaN
            return Ok(vec![f64::NAN; series.len()]);
        }
        if m <= 0.0 {
            // Smoothing factor <= 0 is invalid
            return Err("Runtime Error: SMA smoothing factor must be positive.".to_string());
        }

        let mut result = Vec::with_capacity(series.len());
        let n_f64 = n as f64;

        for i in 0..series.len() {
            if i < n - 1 {
                // 前 N-1 个值是 NaN
                result.push(f64::NAN);
            } else if i == n - 1 {
                // 第 N 个点 (索引 N-1) 通常用简单平均作为初始值 (根据 TDX 行为)
                let sum: f64 = series[0..n].iter().sum();
                result.push(sum / n_f64);
            } else {
                // 递归计算 for i >= N
                // Y[i] = (M*X[i] + (N-M)*Y[i-1]) / N
                let x_i = series[i];
                let y_prev = result[i - 1]; // Yesterday's SMA (already calculated)

                // Handle cases where previous value is NaN
                if y_prev.is_nan() {
                    // 如果前一个值是 NaN，当前值也可能是 NaN
                    result.push(f64::NAN);
                } else {
                    let next_y = (m * x_i + (n_f64 - m) * y_prev) / n_f64;
                    result.push(next_y);
                }
            }
        }
        Ok(result)
    }

    // 指数移动平均: EMA(系列, 周期)
    // TDX EMA(X, N): Y = (2*X + (N-1)*Yesterday's Y) / (N+1)
    // 这等价于 SMA(X, N, 2)
    fn builtin_ema(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: EMA function requires 2 arguments (series, period).".to_string(),
            );
        }
        let series = &args[0];
        let period_arg = &args[1];

        // 调用 SMA 函数，period 参数不变，平滑系数 M 设为 2.0
        // 将 SMA 的参数组织起来
        let sma_args = vec![
            series.clone(),
            period_arg.clone(),
            vec![2.0; period_arg.len()],
        ]; // 克隆参数，最后一个是 M=2.0 的序列

        self.builtin_sma(&sma_args) // 调用 SMA
    }

    // 计数: COUNT(条件, 周期)
    // 计算序列在指定周期内条件成立的次数
    fn builtin_count(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: COUNT function requires 2 arguments (condition, period)."
                    .to_string(),
            );
        }
        let condition_series = &args[0];
        let period_arg = &args[1];

        let n = period_arg.get(0).map_or(0, |&p| p as usize);

        if n == 0 || n > condition_series.len() {
            // Period 0 or > len might result in NaN or 0
            // Let's assume 0 for period 0, and NaN for period > len (like other period fns)
            if n == 0 {
                return Ok(vec![0.0; condition_series.len()]);
            } else {
                return Ok(vec![f64::NAN; condition_series.len()]);
            }
        }

        let mut result = Vec::with_capacity(condition_series.len());

        for i in 0..condition_series.len() {
            if i < n - 1 {
                // 前期不足周期时，窗口不完整，COUNT 通常是 0 (待确认 TDX 行为)
                // 如果是 0，修改这里和对应的测试
                result.push(0.0); // 暂定为 0
            } else {
                // Window is [i - n + 1 ..= i]
                let start_idx = i - n + 1;
                let count = condition_series[start_idx..=i]
                    .iter()
                    .filter(|&&val| val != 0.0 && !val.is_nan()) // Filter for non-zero and non-NaN
                    .count(); // Count the filtered elements

                result.push(count as f64);
            }
        }
        Ok(result)
    }

    // 两个系列的最大值 (元素级别): MAX(系列1, 系列2)
    fn builtin_max_series(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: MAX function requires 2 series arguments.".to_string());
        }
        let series1 = &args[0];
        let series2 = &args[1];

        let len = std::cmp::min(series1.len(), series2.len());
        let result: Vec<f64> = series1
            .iter()
            .zip(series2.iter())
            .map(|(&a, &b)| f64::max(a, b)) // Use f64::max which handles NaN
            .take(len) // Ensure result is only up to min length
            .collect();

        // TDX might extend the shorter series with NaN or last value?
        // Let's resize result to match the longest input series, padding with NaN.
        let max_len = std::cmp::max(series1.len(), series2.len());
        let mut resized_result = result;
        resized_result.resize(max_len, f64::NAN); // Pad with NaN if necessary

        Ok(resized_result)
    }

    // 两个系列的最小值 (元素级别): MIN(系列1, 系列2)
    fn builtin_min_series(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: MIN function requires 2 series arguments.".to_string());
        }
        let series1 = &args[0];
        let series2 = &args[1];

        let len = std::cmp::min(series1.len(), series2.len());
        let result: Vec<f64> = series1
            .iter()
            .zip(series2.iter())
            .map(|(&a, &b)| f64::min(a, b)) // Use f64::min which handles NaN
            .take(len) // Ensure result is only up to min length
            .collect();

        let max_len = std::cmp::max(series1.len(), series2.len());
        let mut resized_result = result;
        resized_result.resize(max_len, f64::NAN); // Pad with NaN if necessary

        Ok(resized_result)
    }

    // 区间判断: BETWEEN(系列, A, B)
    // 返回 1.0 if A <= 系列 <= B, 否则 0.0
    fn builtin_between(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 3 {
            return Err("Runtime Error: BETWEEN function requires 3 arguments (series, lower_bound, upper_bound).".to_string());
        }
        let series = &args[0];
        let lower_bound_series = &args[1]; // 可以是常数序列或变量序列
        let upper_bound_series = &args[2]; // 可以是常数序列或变量序列

        let len = std::cmp::min(
            series.len(),
            std::cmp::min(lower_bound_series.len(), upper_bound_series.len()),
        );

        let result: Vec<f64> = series
            .iter()
            .zip(lower_bound_series.iter())
            .zip(upper_bound_series.iter())
            .map(|((&s, &a), &b)| {
                // 如果任一值为 NaN，结果通常是 NaN (TDX 行为待确认，这里先假设如此)
                if s.is_nan() || a.is_nan() || b.is_nan() {
                    f64::NAN
                } else if s >= a && s <= b {
                    1.0 // 在区间内
                } else {
                    0.0 // 不在区间内
                }
            })
            .take(len)
            .collect();

        let max_len = std::cmp::max(
            series.len(),
            std::cmp::max(lower_bound_series.len(), upper_bound_series.len()),
        );
        let mut resized_result = result;
        resized_result.resize(max_len, f64::NAN); // Pad with NaN

        Ok(resized_result)
    }

    // 移动平均: MA(系列, 周期)
    // 需要参数: 一个系列，一个周期 (通常是数字字面量或变量)
    fn builtin_ma(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: MA function requires 2 arguments.".to_string());
        }
        let series = &args[0];
        // 周期参数通常是一个标量值，取其序列的第一个元素作为周期
        // TODO: 需要处理周期序列长度为 0 或包含非整数/非正数的情况
        let period = args[1].get(0).map_or(0, |&p| p as usize);

        if period == 0 {
            // MA(..., 0) 好像返回原始序列? 待确认 TDX 行为，这里先返回错误或原始系列
            // 假设返回原始系列
            return Ok(series.clone());
        }
        if period > series.len() {
            // 周期大于系列长度，结果应全为 NaN 或特定值
            let mut result = vec![f64::NAN; series.len()];
            // TDX MA(C, >len) 似乎会返回 NAN
            return Ok(result);
        }

        let mut result = Vec::with_capacity(series.len());
        let mut current_sum: f64 = 0.0; // 用于滚动计算和优化性能

        for i in 0..series.len() {
            if i < period - 1 {
                // 前 period-1 个值通常无定义或为 NaN
                result.push(f64::NAN); // TDX 通常是这样
            } else if i == period - 1 {
                // 第一个 MA 值
                current_sum = series[0..period].iter().sum();
                result.push(current_sum / period as f64);
            } else {
                // 滚动计算 MA: 加上新值，减去 period 周期前的值
                // 需要确保 i >= period，所以 i - period 是有效索引
                current_sum = current_sum + series[i] - series[i - period];
                result.push(current_sum / period as f64);
            }
        }
        Ok(result)
    }

    // 引用先前值: REF(系列, N)
    // 需要参数: 一个系列，一个偏移量 N (通常是数字)
    fn builtin_ref(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: REF function requires 2 arguments.".to_string());
        }
        let series = &args[0];
        // 偏移量 N
        let n = args[1].get(0).map_or(0, |&offset| offset as usize);

        let mut result = Vec::with_capacity(series.len());
        for i in 0..series.len() {
            if i >= n {
                result.push(series[i - n]); // 引用 i-n 位置的值
            } else {
                result.push(f64::NAN); // 前 N个位置无定义
            }
        }
        Ok(result)
    }

    // 交叉函数: CROSS(A, B)
    // 当 A 从下方向上穿过 B 时，返回 1，否则返回 0
    // 需要参数: 两个系列
    fn builtin_cross(&self, args: &[Vec<f64>]) -> Result<Vec<f64>, String> {
        if args.len() != 2 {
            return Err("Runtime Error: CROSS function requires 2 arguments.".to_string());
        }
        let series1 = &args[0];
        let series2 = &args[1];

        let len = std::cmp::min(series1.len(), series2.len());
        if len < 2 {
            // CROSS 需要至少两个周期才能判断交叉
            return Ok(vec![0.0; len]); // 或者返回 NaN 序列，待确认 TDX 行为
        }

        let mut result = Vec::with_capacity(len);
        // 第一个点无法判断交叉，通常为 0 或 NaN
        result.push(0.0); // 假设为 0

        for i in 1..len {
            // 检查当前点是否 A > B 且上一个点是否 A <= B
            // 这是一个简化的交叉判断
            let crossed_up = series1[i] > series2[i] && series1[i - 1] <= series2[i - 1];
            // 实际的 TDX CROSS 逻辑可能更复杂，例如处理相等的情况
            // 经典的 CROSS(A, B) 可以定义为 A > B && REF(A, 1) <= REF(B, 1)
            // 让我们尝试用 REF 的概念来写 (尽管直接索引更简单)
            let prev_a = series1[i - 1];
            let prev_b = series2[i - 1];
            let crossed_up_tdx = series1[i] > series2[i] && prev_a <= prev_b;

            result.push(if crossed_up_tdx { 1.0 } else { 0.0 });
        }
        // 如果序列长度大于 2，需要填充剩余部分 (通常是 0)
        result.resize(series1.len(), 0.0); // 用 series1 的长度填充，可能需要更灵活的对齐策略

        Ok(result)
    }

    // TODO: 添加更多内置函数，如 FILTER 等等...
    // 这部分工作量很大，需要逐个实现。
}

// --- 在 Evaluator tests 中添加对这些函数的测试 ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer; // 需要 Lexer 来创建 Parser
    use crate::parser::Parser;
    use float_cmp::{ApproxEq, F64Margin}; // 导入 ApproxEq trait 和 F64Margin
    use std::f64;

    // 自定义函数来比较两个 Vec<f64> 是否近似相等
    fn assert_vec_approx_eq(actual: &[f64], expected: &[f64], epsilon: f64, msg: &str) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "{} - Series lengths differ.",
            msg
        );
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            if a.is_nan() && e.is_nan() {
                continue;
            }
            // 确保非 NaN 时才比较
            if a.is_nan() != e.is_nan() {
                panic!(
                    "{} - Element at index {} differs in NaN status: actual = {}, expected = {}",
                    msg, i, a, e
                );
            }

            assert!(
                (*a).approx_eq(*e, F64Margin::default().epsilon(epsilon)), // <--- 使用 .default().epsilon() 方法
                "{} - Element at index {} differs: actual = {}, expected = {}",
                msg,
                i,
                a,
                e
            );
        }
    }

    // Helper function to create dummy input data
    pub fn create_dummy_input_data(num_bars: usize) -> InputData {
        let mut opens = Vec::with_capacity(num_bars);
        let mut highs = Vec::with_capacity(num_bars);
        let mut lows = Vec::with_capacity(num_bars);
        let mut closes = Vec::with_capacity(num_bars);
        let mut volumes = Vec::with_capacity(num_bars);

        // 生成一些简单的递增数据，方便测试
        for i in 0..num_bars {
            opens.push(10.0 + i as f64);
            highs.push(11.0 + i as f64);
            lows.push(9.0 + i as f64);
            closes.push(10.5 + i as f64); // 让收盘价稍微不同
            volumes.push(100.0 + (i * 10) as f64);
        }

        // 构造函数会检查长度
        InputData::new(opens, highs, lows, closes, volumes).unwrap()
    }

    // Helper to parse, evaluate, and check the result
    pub fn check_evaluation(input: &str, input_data: &InputData, expected_result: FormulaResult) {
        let lexer = crate::lexer::Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let formula = parser.parse_formula().expect("Parsing failed");

        let mut evaluator = Evaluator::new(input_data);
        let actual_result = evaluator
            .evaluate_formula(&formula)
            .expect("Evaluation failed");

        // 比较 FormulaResult 的结构，但不直接比较 Vec<f64> 的相等性
        assert_eq!(
            actual_result.output_lines.len(),
            expected_result.output_lines.len(),
            "Input formula: \"{}\" - Number of output lines differs.",
            input
        );

        for (i, (actual_line, expected_line)) in actual_result
            .output_lines
            .iter()
            .zip(expected_result.output_lines.iter())
            .enumerate()
        {
            assert_eq!(
                actual_line.name, expected_line.name,
                "Input formula: \"{}\" - Output line {} name differs.",
                input, i
            );
            assert_eq!(
                actual_line.styles, expected_line.styles,
                "Input formula: \"{}\" - Output line {} styles differ.",
                input, i
            );

            // 对数据 Vec<f64> 使用近似比较
            assert_vec_approx_eq(
                &actual_line.data,
                &expected_line.data,
                1e-9,
                &format!(
                    "Input formula: \"{}\" - Output line {} data differs",
                    input, i
                ),
            ); // 使用一个小的 epsilon
        }
        // 不需要最后的 assert_eq!(actual_result, expected_result, ...); 了，上面的循环已经完全比较了所有内容
    }

    // 测试简单的常量和变量引用
    #[test]
    fn test_eval_literal_and_variable() {
        let num_bars = 5;
        let input_data = create_dummy_input_data(num_bars);

        // 测试: OUT1: 10;
        let formula_input1 = "OUT1: 10;";
        let expected_data1 = vec![10.0; num_bars]; // 常量应扩展为等长序列
        let expected_result1 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT1".to_string(),
            data: expected_data1,
            styles: vec![],
        }]);
        check_evaluation(formula_input1, &input_data, expected_result1);

        // 测试: OUT2: C;
        let formula_input2 = "OUT2: C;";
        let expected_data2 = input_data.closes.clone(); // C 变量应返回收盘价序列
        let expected_result2 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT2".to_string(),
            data: expected_data2,
            styles: vec![],
        }]);
        check_evaluation(formula_input2, &input_data, expected_result2);
    }

    // 测试简单的二元算术运算符
    #[test]
    fn test_eval_binary_arithmetic() {
        let num_bars = 5;
        let input_data = create_dummy_input_data(num_bars);
        // 示例数据 C: [10.5, 11.5, 12.5, 13.5, 14.5]

        // 测试: OUT: C + 1;
        let formula_input1 = "OUT: C + 1;";
        let expected_data1: Vec<f64> = input_data.closes.iter().map(|c| c + 1.0).collect();
        let expected_result1 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT".to_string(),
            data: expected_data1,
            styles: vec![],
        }]);
        check_evaluation(formula_input1, &input_data, expected_result1);

        // 测试: OUT: H - L;
        let formula_input2 = "OUT: H - L;";
        let expected_data2: Vec<f64> = input_data
            .highs
            .iter()
            .zip(input_data.lows.iter())
            .map(|(h, l)| h - l)
            .collect();
        let expected_result2 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT".to_string(),
            data: expected_data2,
            styles: vec![],
        }]);
        check_evaluation(formula_input2, &input_data, expected_result2);

        // 测试: OUT: H * 2;
        let formula_input3 = "OUT: H * 2;";
        let expected_data3: Vec<f64> = input_data.highs.iter().map(|h| h * 2.0).collect();
        let expected_result3 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT".to_string(),
            data: expected_data3,
            styles: vec![],
        }]);
        check_evaluation(formula_input3, &input_data, expected_result3);

        // 测试: OUT: V / 100;
        let formula_input4 = "OUT: V / 100;";
        let expected_data4: Vec<f64> = input_data.volumes.iter().map(|v| v / 100.0).collect();
        let expected_result4 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT".to_string(),
            data: expected_data4,
            styles: vec![],
        }]);
        check_evaluation(formula_input4, &input_data, expected_result4);

        // 测试除零 (需要包含一个零)
        let mut input_data_with_zero = create_dummy_input_data(5);
        input_data_with_zero.volumes[2] = 0.0; // 在第3个周期设置成交量为0
        let formula_input5 = "OUT: C / V;";
        let mut expected_data5: Vec<f64> = input_data_with_zero
            .closes
            .iter()
            .zip(input_data_with_zero.volumes.iter())
            .map(|(c, v)| if *v == 0.0 { f64::NAN } else { c / v })
            .collect();
        let expected_result5 = FormulaResult::new(vec![OutputLineResult {
            name: "OUT".to_string(),
            data: expected_data5,
            styles: vec![],
        }]);
        check_evaluation(formula_input5, &input_data_with_zero, expected_result5);
    }

    // 测试赋值语句
    #[test]
    fn test_eval_assignment() {
        let num_bars = 5;
        let input_data = create_dummy_input_data(num_bars);
        // 测试: MYVAR := C * 2; OUT: MYVAR;
        let formula_input = "MYVAR := C * 2; OUT: MYVAR;";

        // 预期结果只包含 OUT 线
        let expected_myvar_data: Vec<f64> = input_data.closes.iter().map(|c| c * 2.0).collect();
        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "OUT".to_string(),
            data: expected_myvar_data,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);
    }

    // 测试 MA 函数
    #[test]
    fn test_eval_ma_function() {
        let num_bars = 10; // 至少需要 MA 周期+1 个数据点
        let input_data = create_dummy_input_data(num_bars);
        // 示例数据 C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: MA5: MA(C, 5);
        let formula_input = "MA5: MA(C, 5);";

        // 手动计算 MA(C, 5) 的前几个预期值
        let mut expected_ma5_data = vec![f64::NAN; num_bars];
        for i in 0..num_bars {
            if i >= 4 {
                // MA(C, 5) 从第 5 个点 (索引 4) 开始有值
                let sum: f64 = input_data.closes[(i - 4)..(i + 1)].iter().sum();
                expected_ma5_data[i] = sum / 5.0;
            }
        }
        // 预期 MA5 数据: [NaN, NaN, NaN, NaN, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5] - 这是我们的简单递增数据
        // 确保手动计算的预期值正确
        assert_vec_approx_eq(
            &expected_ma5_data[4..6],
            &[12.5, 13.5],
            1e-9,
            "MA Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "MA5".to_string(),
            data: expected_ma5_data,
            styles: vec![],
        }]);

        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 测试 MA(C, 0) 或周期大于长度的情况
    }

    // 测试 SUM 函数
    #[test]
    fn test_eval_sum_function() {
        let num_bars = 10;
        let input_data = create_dummy_input_data(num_bars);
        // C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: SUMC5: SUM(C, 5);
        let formula_input = "SUMC5: SUM(C, 5);";

        // 手动计算 SUM(C, 5) 的预期值
        let mut expected_sum_data = vec![f64::NAN; num_bars];
        for i in 0..num_bars {
            if i >= 4 {
                // SUM(C, 5) 从第 5 个点 (索引 4) 开始有值
                let sum: f64 = input_data.closes[(i - 4)..(i + 1)].iter().sum();
                expected_sum_data[i] = sum;
            }
        }
        // 预期 SUM5 数据: [NaN, NaN, NaN, NaN, 62.5, 68.5, 74.5, 80.5, 86.5, 92.5]
        assert_vec_approx_eq(
            &expected_sum_data[4..6],
            &[62.5, 67.5],
            1e-9,
            "SUM Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "SUMC5".to_string(),
            data: expected_sum_data,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 测试 SUM(C, 0) 或周期 > len 的情况
    }

    // 测试 AVERAGE 函数
    #[test]
    fn test_eval_average_function() {
        let num_bars = 10;
        let input_data = create_dummy_input_data(num_bars);
        // C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: AVGC5: AVERAGE(C, 5);
        let formula_input = "AVGC5: AVERAGE(C, 5);";

        // 手动计算 AVERAGE(C, 5) 的预期值 (SUM / 5)
        let mut expected_avg_data = vec![f64::NAN; num_bars];
        for i in 0..num_bars {
            if i >= 4 {
                let sum: f64 = input_data.closes[(i - 4)..(i + 1)].iter().sum();
                expected_avg_data[i] = sum / 5.0;
            }
        }
        // 预期 AVG5 数据: [NaN, NaN, NaN, NaN, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5]
        assert_vec_approx_eq(
            &expected_avg_data[4..6],
            &[12.5, 13.5],
            1e-9,
            "AVERAGE Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "AVGC5".to_string(),
            data: expected_avg_data,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 测试 AVERAGE(C, 0) 或周期 > len 的情况
    }

    // 测试 BARSCOUNT 函数
    #[test]
    fn test_eval_barscount_function() {
        let num_bars = 10;
        // Create dummy data first
        let mut input_data = create_dummy_input_data(num_bars);

        // --- 修改这里的数据设置 ---
        // 目标：让 C > O 在前两个周期 (索引 0 和 1) 不成立，从索引 2 开始首次成立
        // 原始 opens:  [10.0, 11.0, 12.0, ...]
        // 原始 closes: [10.5, 11.5, 12.5, ...]
        // C > O 原本是全真。我们需要修改 C[0] 和 C[1]，让 C <= O。

        // 将 C[0] 设置为 <= O[0]
        input_data.closes[0] = 10.0; // C[0]=10.0, O[0]=10.0 => C>O 为 False
        // 将 C[1] 设置为 <= O[1]
        input_data.closes[1] = 11.0; // C[1]=11.0, O[1]=11.0 => C>O 为 False
        // C[2] = 12.5 (默认), O[2] = 12.0 => C[2] > O[2] 为 True (这是首次成立)
        // 后面的 C[i] > O[i] 也默认是 True

        // 现在，条件 C > O 的实际序列大致是 [0.0, 0.0, 1.0, 1.0, 1.0, 1.0, ...]
        // 首次为真的索引是 2。

        // Test: COUNT: BARSCOUNT(C > O);
        let formula_input = "COUNT: BARSCOUNT(C > O);";

        // 根据新的条件序列 [0.0, 0.0, 1.0, 1.0, ...] 计算 BARSCOUNT
        // i=0: 条件为假，从未成立。结果: (0 + 1) = 1
        // i=1: 条件为假，从未成立。结果: (1 + 1) = 2
        // i=2: 条件为真。首次成立索引是 2。结果: (2 - 2 + 1) = 1
        // i=3: 条件为真。首次成立索引是 2。结果: (3 - 2 + 1) = 2
        // i=4: 条件为真。首次成立索引是 2。结果: (4 - 2 + 1) = 3
        // ...
        // i=9: 条件为真。首次成立索引是 2。结果: (9 - 2 + 1) = 8
        let manual_expected = vec![1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]; // 这个硬编码值现在和计算结果一致了。

        // The rest of the test code remains the same.
        // ...
        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "COUNT".to_string(),
            // 直接使用硬编码的 manual_expected 作为预期的结果数据源
            data: manual_expected,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);
    }

    // 测试 HHV 函数
    #[test]
    fn test_eval_hhv_function() {
        let num_bars = 10;
        let input_data = create_dummy_input_data(num_bars);
        // C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: HHVC5: HHV(C, 5);
        let formula_input = "HHVC5: HHV(C, 5);";

        let mut expected_hhv_data = vec![f64::NAN; num_bars];
        for i in 0..num_bars {
            if i >= 4 {
                // HHV(C, 5) 从索引 4 开始有值
                let start_idx = i - 4;
                expected_hhv_data[i] = input_data.closes[start_idx..=i]
                    .iter()
                    .cloned() // 克隆 f64 才能求 max
                    .fold(f64::NAN, f64::max); // 使用 fold + max 处理 NaN
            }
        }
        // 预期 HHV5 数据: [NaN, NaN, NaN, NaN, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]
        assert_vec_approx_eq(
            &expected_hhv_data[4..6],
            &[14.5, 15.5],
            1e-9,
            "HHV Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "HHVC5".to_string(),
            data: expected_hhv_data,
            styles: vec![],
        }]);

        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 测试 HHV(C, 0) 或周期 > len 的情况
    }

    // 测试 LLV 函数
    #[test]
    fn test_eval_llv_function() {
        let num_bars = 10;
        let input_data = create_dummy_input_data(num_bars);
        // C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: LLVC5: LLV(C, 5);
        let formula_input = "LLVC5: LLV(C, 5);";

        let mut expected_llv_data = vec![f64::NAN; num_bars];
        for i in 0..num_bars {
            if i >= 4 {
                // LLV(C, 5) 从索引 4 开始有值
                let start_idx = i - 4;
                expected_llv_data[i] = input_data.closes[start_idx..=i]
                    .iter()
                    .cloned()
                    .fold(f64::NAN, f64::min); // 使用 fold + min 处理 NaN
            }
        }
        // 预期 LLV5 数据: [NaN, NaN, NaN, NaN, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5]
        assert_vec_approx_eq(
            &expected_llv_data[4..6],
            &[10.5, 11.5],
            1e-9,
            "LLV Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "LLVC5".to_string(),
            data: expected_llv_data,
            styles: vec![],
        }]);

        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 测试 LLV(C, 0) 或周期 > len 的情况
    }

    // 测试 ABS 函数
    #[test]
    fn test_eval_abs_function() {
        let num_bars = 5;
        // 创建一个包含负数和 NaN 的序列
        let mut custom_series = vec![-1.0, 5.0, 0.0, f64::NAN, -2.5];
        let mut input_data = create_dummy_input_data(num_bars);
        input_data.closes = custom_series.clone(); // 使用自定义序列作为 C

        // 测试: ABSC: ABS(C);
        let formula_input = "ABSC: ABS(C);";

        // 手动计算 ABS 预期结果
        let expected_abs_data: Vec<f64> = custom_series.iter().map(|&val| val.abs()).collect();
        // 预期 ABS 数据: [1.0, 5.0, 0.0, NaN, 2.5]

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "ABSC".to_string(),
            data: expected_abs_data,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);
    }
    // 测试 SMA 函数 (Recursive Moving Average)
    #[test]
    fn test_eval_sma_function() {
        let num_bars = 10; // 至少需要 SMA 周期+1 个数据点
        let input_data = create_dummy_input_data(num_bars);
        // C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: SMA(C, 5, 1)
        // 预期 SMA(C, 5, 1) 数据: [NaN, NaN, NaN, NaN, SMA[4], SMA[5], ...]
        // SMA[4] = SUM(C[0..4]) / 5 = 12.5 (Simple Average for first point)
        // SMA[i] = (M*X[i] + (N-M)*Y[i-1]) / N for i > 4, M=1
        // SMA[i] = (X[i] + (N-1)*Y[i-1]) / N
        // SMA[5] = (C[5] + 4*SMA[4]) / 5 = (15.5 + 4 * 12.5) / 5 = (15.5 + 50) / 5 = 65.5 / 5 = 13.1
        // SMA[6] = (C[6] + 4*SMA[5]) / 5 = (16.5 + 4 * 13.1) / 5 = (16.5 + 52.4) / 5 = 68.9 / 5 = 13.78
        // SMA[7] = (C[7] + 4*SMA[6]) / 5 = (17.5 + 4 * 13.78) / 5 = (17.5 + 55.12) / 5 = 72.62 / 5 = 14.524

        let formula_input = "SMAC5: SMA(C, 5, 1);";

        let mut expected_sma_data = vec![f64::NAN; num_bars];
        let n = 5;
        let m = 1.0;
        let n_f64 = n as f64;

        for i in 0..num_bars {
            if i >= n - 1 {
                if i == n - 1 {
                    let sum: f64 = input_data.closes[0..n].iter().sum();
                    expected_sma_data[i] = sum / n_f64;
                } else {
                    let x_i = input_data.closes[i];
                    let y_prev = expected_sma_data[i - 1];
                    // Only calculate if previous is not NaN
                    if !y_prev.is_nan() {
                        expected_sma_data[i] = (m * x_i + (n_f64 - m) * y_prev) / n_f64;
                    } else {
                        // If previous is NaN, current is also NaN (already set by initialization)
                    }
                }
            }
        }
        // 预期 SMA5 数据 (M=1): [NaN, NaN, NaN, NaN, 12.5, 13.1, 13.78, 14.524, 15.2192, 15.97536]
        assert_vec_approx_eq(
            &expected_sma_data[4..],
            &[12.5, 13.1, 13.78, 14.524, 15.3192, 16.15536],
            1e-9,
            "SMA Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "SMAC5".to_string(),
            data: expected_sma_data,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 添加 SMA(C, 5, 2) 或其他 M 值的测试
        // TODO: 添加 SMA(C, 0, 1) 或周期 > len 的情况
    }

    // 测试 EMA 函数 (SMA with M=2)
    #[test]
    fn test_eval_ema_function() {
        let num_bars = 10;
        let input_data = create_dummy_input_data(num_bars);
        // C: [10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5]

        // 测试: EMA(C, 5) == SMA(C, 5, 2)
        // 预期 EMA(C, 5) 数据: [NaN, NaN, NaN, NaN, EMA[4], EMA[5], ...]
        // EMA[4] = (2*C[4] + (5-1)*EMA[3]) / (5+1) = (2*14.5 + 4*NaN) / 6 = NaN (前期 NaN)
        // TDX EMA 的第一个值通常是 SMA(X, N, 2) 的第一个有值点 (索引 N-1)
        // SMA(X, N, M) 的第一个有值点 Y[N-1] = (M*X[N-1] + (N-M)*Y[N-2]) / N ... 最终追溯到第一个 X[0]
        // 实际通达信的 EMA(X,N) 第一个有效值 (在索引 N-1) 是 (2*X[N-1] + (N-1)*X[N-2])/(N+1) ? No.
        // 最常见的 TDX EMA(X,N) 实现是： Y = (X*2 + Y.REF(1)*(N-1))/(N+1)
        // 第一个有值点 (索引 N-1) = (2*X[N-1] + (N-1)*X[N-2])/(N+1)? No.
        // 普遍认为 TDX EMA(X,N) 的**首个有效值**是 X[0] ? 或者 SUM(X,N)/(N)?
        // 我之前 SMA 实现的第一个点 (索引 N-1) 是简单平均。如果 EMA 调用 SMA，EMA 的第一个点也是简单平均。
        // SMA(X,N,M) 公式 Y = (M*X + (N-M)*Yesterday's Y) / N
        // 第一个有值 Y[N-1] = (M*X[N-1] + (N-M)*Y[N-2]) / N. 如果 Y[N-2] 是 NaN, Y[N-1] 也是 NaN unless N-M=0 or M=N.
        // 只有当 M=N 时，Y[N-1] = (N*X[N-1] + 0) / N = X[N-1]
        // 只有当 M=N=1 时，Y[0] = (1*X[0] + 0*Y[-1])/1 = X[0]
        // 通达信 EMA(X,N) 的第一个有效值 Y[N-1] 似乎是 X[N-1] 本身。
        // 让我修改 SMA 的第一个点计算：如果 M=N，第一个点是 X[N-1]，否则是 NaN。
        // 或者更简单的，EMA(X,N) 第一个点就是 X[N-1] (对应到 SMA(X,N,N))
        // 根据 https://www.weistock.com/bbs/dispbbs.asp?boardid=11&id=41423
        // 通达信的 EMA 函数算法是：Y=(X*2+前一日Y*(N-1))/(N+1)
        // 初始值：通常取前 N 日简单平均，或者取第一天值。取简单平均更常用。
        // 我们的 SMA(X, N, M) 第一个点 (索引 N-1) = Sum(X[0..N-1]) / N.
        // 如果 EMA(X, N) 调用 SMA(X, N, 2), 且 SMA 的第一个点是简单平均，
        // 那么 EMA(X,N) 的第一个点也是简单平均。
        // EMA(C, 5) => N=5, M=2
        // SMA(C, 5, 2)
        // SMA[4] = SUM(C[0..4]) / 5 = 12.5
        // SMA[5] = (2*C[5] + (5-2)*SMA[4]) / 5 = (2*15.5 + 3*12.5) / 5 = (31.0 + 37.5) / 5 = 68.5 / 5 = 13.7
        // SMA[6] = (2*C[6] + 3*SMA[5]) / 5 = (2*16.5 + 3*13.7) / 5 = (33.0 + 41.1) / 5 = 74.1 / 5 = 14.82

        let formula_input = "EMAC5: EMA(C, 5);";

        let mut expected_ema_data = vec![f64::NAN; num_bars];
        let n = 5;
        let m = 2.0;
        let n_f64 = n as f64;

        for i in 0..num_bars {
            if i >= n - 1 {
                if i == n - 1 {
                    // 第 N 个点 (索引 N-1)，使用简单平均作为 EMA/SMA 递归的起点
                    let sum: f64 = input_data.closes[0..n].iter().sum();
                    expected_ema_data[i] = sum / n_f64;
                } else {
                    // 递归计算
                    let x_i = input_data.closes[i];
                    let y_prev = expected_ema_data[i - 1];
                    // 确保前一个值不是 NaN
                    if !y_prev.is_nan() {
                        expected_ema_data[i] = (m * x_i + (n_f64 - m) * y_prev) / n_f64;
                    } else {
                        // If previous is NaN, current is also NaN (already set by initialization)
                    }
                }
            }
        }
        // 预期 EMA(C, 5) 数据: [NaN, NaN, NaN, NaN, 12.5, 13.7, 14.82, 15.852, 16.8816, 17.89696]
        assert_vec_approx_eq(
            &expected_ema_data[4..6],
            &[12.5, 13.7],
            1e-9,
            "EMA Manual Check",
        );

        let expected_result = FormulaResult::new(vec![OutputLineResult {
            name: "EMAC5".to_string(),
            data: expected_ema_data,
            styles: vec![],
        }]);
        check_evaluation(formula_input, &input_data, expected_result);

        // TODO: 添加 EMA(C, 0) 或周期 > len 的情况
    }

    // TODO: 添加 REF, CROSS 的更多边界测试
    // TODO: 添加关系运算符 (> < >= <= = <>) 的测试 (结果应为 1 或 0)
    // TODO: 添加逻辑运算符 (AND OR NOT) 的测试 (结果应为 1 或 0)
    // TODO: 添加混合表达式和括号的测试
    // TODO: 添加多语句和样式的测试
    // TODO: 添加错误情况的测试 (变量未定义，函数参数数量不对等)
}
