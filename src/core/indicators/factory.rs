use crate::core::indicators::{
    bollinger::BollingerBands, ma::MovingAverage, macd::Macd, rsi::Rsi, traits::IndicatorCalculator,
};
use crate::models::{
    indicators::{BollingerConfig, IndicatorConfig},
    Candle, IndicatorValue,
};
use std::collections::{HashMap, VecDeque};

/// 指標工廠
///
/// 接收動態指標請求，透過 DAG 拓撲排序決定計算順序，
/// 確保有依賴關係的指標在其依賴項計算完成後才執行。
/// 循環依賴時回傳錯誤，不允許執行。
pub struct IndicatorFactory {
    /// 已註冊的指標計算器，key 為指標 ID
    calculators: HashMap<String, Box<dyn IndicatorCalculator>>,
}

impl IndicatorFactory {
    /// 建立新的 IndicatorFactory 並註冊所有已知指標
    pub fn new() -> Self {
        Self {
            calculators: HashMap::new(),
        }
    }

    /// 依據請求的指標設定，建立對應的計算器並回傳工廠實例
    ///
    /// 每次請求建立新的計算器組合，不重用跨請求的狀態。
    pub fn build_from_request(
        indicators: &HashMap<String, IndicatorConfig>,
    ) -> anyhow::Result<Self> {
        let mut factory = Self::new();

        for (name, config) in indicators {
            match name.as_str() {
                "ma" => {
                    if let IndicatorConfig::Periods(periods) = config {
                        for &period in periods {
                            let calculator = MovingAverage::new(period as usize)?;
                            factory.register(Box::new(calculator));
                        }
                    }
                }
                "rsi" => {
                    if let IndicatorConfig::Periods(periods) = config {
                        for &period in periods {
                            let calculator = Rsi::new(period as usize)?;
                            factory.register(Box::new(calculator));
                        }
                    }
                }
                "macd" => {
                    if let IndicatorConfig::Periods(periods) = config {
                        if periods.len() != 3 {
                            anyhow::bail!(
                                "MACD requires exactly 3 periods [fast, slow, signal], got {}",
                                periods.len()
                            );
                        }
                        let calculator = Macd::new(
                            periods[0] as usize,
                            periods[1] as usize,
                            periods[2] as usize,
                        )?;
                        factory.register(Box::new(calculator));
                    }
                }
                "bollinger" => {
                    if let IndicatorConfig::Bollinger(BollingerConfig {
                        period,
                        std_dev_multiplier,
                    }) = config
                    {
                        let calculator =
                            BollingerBands::new(*period as usize, *std_dev_multiplier)?;
                        factory.register(Box::new(calculator));
                    }
                }
                unknown => {
                    anyhow::bail!("Unknown indicator type: {unknown}");
                }
            }
        }

        Ok(factory)
    }

    /// 註冊指標計算器
    fn register(&mut self, calculator: Box<dyn IndicatorCalculator>) {
        self.calculators
            .insert(calculator.id().to_string(), calculator);
    }

    /// 依 DAG 依賴關係進行拓撲排序，回傳正確的計算順序
    ///
    /// 使用 Kahn's algorithm（BFS 拓撲排序）：
    /// 1. 計算每個節點的入度（被依賴次數）
    /// 2. 將入度為 0 的節點加入佇列
    /// 3. 依序取出節點，更新鄰居的入度
    /// 4. 若最終排序數量不等於節點總數，表示存在循環依賴
    ///
    /// 循環依賴時回傳 INVALID_INDICATOR_CONFIG 錯誤。
    pub fn resolve_execution_order(&self) -> anyhow::Result<Vec<String>> {
        let ids: Vec<&str> = self.calculators.keys().map(|s| s.as_str()).collect();

        // 建立入度表
        let mut in_degree: HashMap<&str, usize> = ids.iter().map(|&id| (id, 0)).collect();

        // 建立依賴圖：dependency -> dependents
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();

        for (id, calculator) in self
            .calculators
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_ref()))
        {
            for dep in calculator.dependencies() {
                if !self.calculators.contains_key(dep) {
                    anyhow::bail!(
                        "Indicator '{id}' depends on '{dep}' which is not in the request"
                    );
                }
                graph.entry(dep).or_default().push(id);
                *in_degree.entry(id).or_insert(0) += 1;
            }
        }

        // BFS 拓撲排序（Kahn's algorithm）
        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut order: Vec<String> = Vec::with_capacity(self.calculators.len());

        while let Some(current) = queue.pop_front() {
            order.push(current.to_string());

            if let Some(dependents) = graph.get(current) {
                for &dependent in dependents {
                    let degree = in_degree.entry(dependent).or_insert(0);
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }

        if order.len() != self.calculators.len() {
            anyhow::bail!(
                "Circular dependency detected among indicators: {:?}",
                self.calculators.keys().collect::<Vec<_>>()
            );
        }

        Ok(order)
    }

    /// 依拓撲排序順序計算所有指標
    ///
    /// 前一個指標的結果存入 computed map，供有依賴關係的指標使用。
    /// 回傳 (計算結果 map, 執行順序)，執行順序供 response 的 dag_execution_order 欄位使用。
    pub fn compute_all(
        &self,
        candles: &[Candle],
    ) -> anyhow::Result<(HashMap<String, Vec<IndicatorValue>>, Vec<String>)> {
        let execution_order = self.resolve_execution_order()?;
        let mut computed: HashMap<String, Vec<IndicatorValue>> = HashMap::new();

        for id in &execution_order {
            let calculator = self
                .calculators
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("Calculator '{id}' not found in registry"))?;

            let result = calculator.compute(candles, &computed)?;
            computed.insert(id.clone(), result);
        }

        Ok((computed, execution_order))
    }
}

impl Default for IndicatorFactory {
    fn default() -> Self {
        Self::new()
    }
}

// ── 單元測試 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::indicators::IndicatorConfig;
    use crate::models::Interval;

    fn make_candles(count: usize) -> Vec<Candle> {
        (0..count)
            .map(|i| Candle {
                symbol: "2330".to_string(),
                interval: Interval::OneHour,
                timestamp_ms: 1704067200000 + (i as i64 * 3_600_000),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.0 + i as f64,
                volume: 1_000_000,
                indicators: Default::default(),
            })
            .collect()
    }

    #[test]
    fn test_factory_build_ma_from_request() {
        let mut request = HashMap::new();
        request.insert("ma".to_string(), IndicatorConfig::Periods(vec![5, 20]));

        let factory = IndicatorFactory::build_from_request(&request).unwrap();
        assert!(factory.calculators.contains_key("ma5"));
        assert!(factory.calculators.contains_key("ma20"));
    }

    #[test]
    fn test_factory_build_unknown_indicator_returns_error() {
        let mut request = HashMap::new();
        request.insert(
            "unknown_indicator".to_string(),
            IndicatorConfig::Periods(vec![14]),
        );

        assert!(IndicatorFactory::build_from_request(&request).is_err());
    }

    #[test]
    fn test_factory_macd_wrong_period_count_returns_error() {
        let mut request = HashMap::new();
        request.insert("macd".to_string(), IndicatorConfig::Periods(vec![12, 26])); // 缺 signal

        assert!(IndicatorFactory::build_from_request(&request).is_err());
    }

    #[test]
    fn test_resolve_execution_order_no_dependencies() {
        let mut request = HashMap::new();
        request.insert("ma".to_string(), IndicatorConfig::Periods(vec![5]));
        request.insert("rsi".to_string(), IndicatorConfig::Periods(vec![14]));

        let factory = IndicatorFactory::build_from_request(&request).unwrap();
        let order = factory.resolve_execution_order().unwrap();

        assert_eq!(order.len(), 2);
        assert!(order.contains(&"ma5".to_string()));
        assert!(order.contains(&"rsi14".to_string()));
    }

    #[test]
    fn test_compute_all_returns_correct_length() {
        let mut request = HashMap::new();
        request.insert("ma".to_string(), IndicatorConfig::Periods(vec![5]));

        let factory = IndicatorFactory::build_from_request(&request).unwrap();
        let candles = make_candles(30);
        let (computed, order) = factory.compute_all(&candles).unwrap();

        assert!(computed.contains_key("ma5"));
        assert_eq!(computed["ma5"].len(), 30);
        assert!(order.contains(&"ma5".to_string()));
    }

    #[test]
    fn test_compute_all_multiple_indicators() {
        let mut request = HashMap::new();
        request.insert("ma".to_string(), IndicatorConfig::Periods(vec![5, 20]));
        request.insert("rsi".to_string(), IndicatorConfig::Periods(vec![14]));
        request.insert(
            "macd".to_string(),
            IndicatorConfig::Periods(vec![12, 26, 9]),
        );

        let factory = IndicatorFactory::build_from_request(&request).unwrap();
        let candles = make_candles(50);
        let (computed, _) = factory.compute_all(&candles).unwrap();

        assert!(computed.contains_key("ma5"));
        assert!(computed.contains_key("ma20"));
        assert!(computed.contains_key("rsi14"));
        assert!(computed.contains_key("macd"));
    }
}
