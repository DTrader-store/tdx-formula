use crate::ast::PlotStyle; // 需要 PlotStyle 来表示输出线的样式

#[derive(Debug, Clone)]
pub struct InputData {
    // 为了简化，先只用 Vec<f64> 表示各时间序列
    // 更完整的可能需要日期/时间信息，或者一个 Vec<Bar>
    pub opens: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub closes: Vec<f64>,
    pub volumes: Vec<f64>,
    // 可以根据需要添加其他数据，比如成交额、持仓量等

    // 方便获取数据长度
    pub num_bars: usize,
}

impl InputData {
    // 构造函数
    pub fn new(
        opens: Vec<f64>,
        highs: Vec<f64>,
        lows: Vec<f64>,
        closes: Vec<f64>,
        volumes: Vec<f64>,
    ) -> Result<Self, String> {
        let num_bars = opens.len();
        // 简单检查所有序列长度是否一致
        if highs.len() != num_bars
            || lows.len() != num_bars
            || closes.len() != num_bars
            || volumes.len() != num_bars
        {
            Err("Input data series must all have the same length.".to_string())
        } else {
            Ok(InputData {
                opens,
                highs,
                lows,
                closes,
                volumes,
                num_bars,
            })
        }
    }
}

// 表示单条输出线（指标线或信号线）的求值结果
#[derive(Debug, Clone, PartialEq)]
pub struct OutputLineResult {
    pub name: String,           // 输出线的名称 (AST 中的 name 或默认名称)
    pub data: Vec<f64>,         // 计算出的时间序列数据
    pub styles: Vec<PlotStyle>, // 绘图样式
}

// 表示整个公式的最终求值结果
#[derive(Debug, Clone, PartialEq)]
pub struct FormulaResult {
    pub output_lines: Vec<OutputLineResult>,
    // 可以根据需要添加其他信息，比如选股结果（布尔值序列），交易信号列表等
}

impl FormulaResult {
    pub fn new(output_lines: Vec<OutputLineResult>) -> Self {
        FormulaResult { output_lines }
    }
}
