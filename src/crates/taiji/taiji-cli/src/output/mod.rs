//! 输出格式化模块 — 传音符/上界文书
//!
//! 支持 text/json/csv/table 四种输出格式。
//! 所有数据查询命令通过 `OutputFormat` 枚举统一分发。

use std::io::Write;

use serde::Serialize;

/// 全局输出格式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    /// 人类可读文本（默认——含颜色、表格、对齐）
    Text,
    /// JSON 序列化（机器消费）
    Json,
    /// CSV 导出（数据工具）
    Csv,
    /// 终端表格（纯文本表格，无颜色）
    Table,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
            Self::Csv => write!(f, "csv"),
            Self::Table => write!(f, "table"),
        }
    }
}

/// 所有数据查询命令的输出契约
pub(crate) trait OutputData: Serialize {
    /// 渲染为人类可读文本（写入 io::Write）
    fn render_text(&self, w: &mut dyn Write) -> std::io::Result<()>;
    /// 渲染为表格（写入 io::Write）
    fn render_table(&self, w: &mut dyn Write) -> std::io::Result<()>;
}

/// 输出分发入口
pub(crate) fn write_output<D: OutputData>(
    data: &D,
    format: OutputFormat,
    w: &mut dyn Write,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Text => data.render_text(w),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(data)
                .map_err(std::io::Error::other)?;
            writeln!(w, "{}", json)
        }
        OutputFormat::Csv => {
            let mut csv_writer = csv::Writer::from_writer(vec![]);
            csv_writer
                .serialize(data)
                .map_err(std::io::Error::other)?;
            let csv_data = csv_writer
                .into_inner()
                .map_err(std::io::Error::other)?;
            write!(w, "{}", String::from_utf8_lossy(&csv_data))
        }
        OutputFormat::Table => data.render_table(w),
    }
}

/// 简单的键值对输出
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub(crate) struct KeyValueOutput {
    pub rows: Vec<KeyValueRow>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub(crate) struct KeyValueRow {
    pub key: String,
    pub value: String,
}

impl OutputData for KeyValueOutput {
    fn render_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        let max_key_len = self.rows.iter().map(|r| r.key.len()).max().unwrap_or(0);
        for row in &self.rows {
            writeln!(w, "  {:width$}  {}", row.key, row.value, width = max_key_len)?;
        }
        Ok(())
    }

    fn render_table(&self, w: &mut dyn Write) -> std::io::Result<()> {
        writeln!(w, "╭──────────────────────┬──────────────────────────────────────────╮")?;
        writeln!(w, "│ Key                  │ Value                                    │")?;
        writeln!(w, "├──────────────────────┼──────────────────────────────────────────┤")?;
        for row in &self.rows {
            writeln!(
                w,
                "│ {:20} │ {:40} │",
                row.key.chars().take(20).collect::<String>(),
                row.value.chars().take(40).collect::<String>(),
            )?;
        }
        writeln!(w, "╰──────────────────────┴──────────────────────────────────────────╯")
    }
}

/// 表格数据输出（通用列式数据）
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub(crate) struct TableOutput {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl OutputData for TableOutput {
    fn render_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        if self.rows.is_empty() {
            return writeln!(w, "(空)");
        }
        let col_count = self.headers.len();
        let mut col_widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
        }
        for cw in col_widths.iter_mut() {
            *cw = (*cw).max(4);
        }

        // 顶部分隔线
        write!(w, "╭")?;
        for (i, cw) in col_widths.iter().enumerate() {
            write!(w, "{}", "─".repeat(cw + 2))?;
            if i < col_count - 1 {
                write!(w, "┬")?;
            }
        }
        writeln!(w, "╮")?;

        // header 行
        write!(w, "│")?;
        for (i, h) in self.headers.iter().enumerate() {
            write!(w, " {:width$} │", h, width = col_widths[i])?;
        }
        writeln!(w)?;

        // 分隔线
        write!(w, "├")?;
        for (i, cw) in col_widths.iter().enumerate() {
            write!(w, "{}", "─".repeat(cw + 2))?;
            if i < col_count - 1 {
                write!(w, "┼")?;
            }
        }
        writeln!(w, "┤")?;

        // 数据行
        for row in &self.rows {
            write!(w, "│")?;
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    write!(w, " {:width$} │", cell, width = col_widths[i])?;
                }
            }
            writeln!(w)?;
        }

        // 底部
        write!(w, "╰")?;
        for (i, cw) in col_widths.iter().enumerate() {
            write!(w, "{}", "─".repeat(cw + 2))?;
            if i < col_count - 1 {
                write!(w, "┴")?;
            }
        }
        writeln!(w, "╯")
    }

    fn render_table(&self, w: &mut dyn Write) -> std::io::Result<()> {
        self.render_text(w)
    }
}

impl OutputData for serde_json::Value {
    fn render_text(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        writeln!(w, "{}", json)
    }

    fn render_table(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        // 对于 JSON Value，table 格式与 text 相同
        self.render_text(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Text.to_string(), "text");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Csv.to_string(), "csv");
        assert_eq!(OutputFormat::Table.to_string(), "table");
    }

    #[test]
    fn test_key_value_output_render_text() {
        let output = KeyValueOutput {
            rows: vec![
                KeyValueRow { key: "key1".into(), value: "value1".into() },
                KeyValueRow { key: "longer_key".into(), value: "val2".into() },
            ],
        };
        let mut buf = Vec::new();
        output.render_text(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("key1"));
        assert!(text.contains("value1"));
        assert!(text.contains("longer_key"));
        assert!(text.contains("val2"));
    }

    #[test]
    fn test_key_value_output_render_table() {
        let output = KeyValueOutput {
            rows: vec![KeyValueRow { key: "a".into(), value: "1".into() }],
        };
        let mut buf = Vec::new();
        output.render_table(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("a"));
        assert!(text.contains("1"));
        assert!(text.contains('╭') || text.contains('╰'));
    }

    #[test]
    fn test_table_output_empty() {
        let output = TableOutput {
            headers: vec!["A".into(), "B".into()],
            rows: vec![],
        };
        let mut buf = Vec::new();
        output.render_text(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.trim(), "(空)");
    }

    #[test]
    fn test_table_output_render_text() {
        let output = TableOutput {
            headers: vec!["Symbol".into(), "Price".into()],
            rows: vec![
                vec!["BTC".into(), "50000".into()],
                vec!["ETH".into(), "3000".into()],
            ],
        };
        let mut buf = Vec::new();
        output.render_text(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("Symbol"));
        assert!(text.contains("Price"));
        assert!(text.contains("BTC"));
        assert!(text.contains("50000"));
    }

    #[test]
    fn test_json_value_render_text() {
        let val = json!({"symbol": "BTC", "price": 50000.0});
        let mut buf = Vec::new();
        val.render_text(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("BTC"));
        assert!(text.contains("50000"));
    }

    #[test]
    fn test_json_value_render_table_falls_back_to_text() {
        let val = json!({"msg": "hello"});
        let mut buf_text = Vec::new();
        let mut buf_table = Vec::new();
        val.render_text(&mut buf_text).unwrap();
        val.render_table(&mut buf_table).unwrap();
        assert_eq!(String::from_utf8(buf_text).unwrap(), String::from_utf8(buf_table).unwrap());
    }

    #[test]
    fn test_write_output_text() {
        let val = json!({"x": 1});
        let mut buf = Vec::new();
        write_output(&val, OutputFormat::Text, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("1"));
    }

    #[test]
    fn test_write_output_json() {
        let val = json!({"x": 1});
        let mut buf = Vec::new();
        write_output(&val, OutputFormat::Json, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["x"], 1);
    }

    #[test]
    fn test_table_output_column_alignment() {
        let output = TableOutput {
            headers: vec!["A".into(), "B".into()],
            rows: vec![vec!["short".into(), "very long value".into()]],
        };
        let mut buf = Vec::new();
        output.render_text(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("very long value"));
        assert!(text.contains("short"));
    }
}
