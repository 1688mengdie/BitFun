//! TUI 监控仪表盘 — 天机盘
//!
//! ratatui 四象限实时监控面板：行情速览 / 策略状态 / 风控指标 / 持仓列表 / 告警日志。

pub(crate) mod dashboard;
pub(crate) mod pane_alerts;
pub(crate) mod pane_market;
pub(crate) mod pane_positions;
pub(crate) mod pane_risk;
pub(crate) mod pane_strategies;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tracing::info;

use crate::AppContext;

/// 运行 TUI 仪表盘
pub(crate) fn run_dashboard(
    ctx: AppContext,
    layout_mode: String,
    refresh_rate: f64,
    symbols: Vec<String>,
) -> Result<()> {
    // 进入 raw mode + alternate screen
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;

    let terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;

    let result = run_ui(terminal, ctx, layout_mode, refresh_rate, symbols);

    // 恢复终端
    terminal::disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}

/// TUI 主循环
fn run_ui(
    mut terminal: Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    _ctx: AppContext,
    _layout_mode: String,
    refresh_rate: f64,
    symbols: Vec<String>,
) -> Result<()> {
    let tick_rate = Duration::from_secs_f64(1.0 / refresh_rate.clamp(0.5, 10.0));

    let mut app = MonitorApp::new(symbols);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| app.render(f))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(50));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Tab => app.focus_next(),
                        KeyCode::Char('r') => app.refresh(),
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }
    }

    info!("TUI 仪表盘退出");
    Ok(())
}

    /// TUI 仪表盘应用状态
struct MonitorApp {
    connection: String,
    #[allow(dead_code)]
    symbols: Vec<String>,
    focus: usize,
    tick_count: u64,

    // 面板数据（简化版）
    market_lines: Vec<String>,
    strategy_lines: Vec<String>,
    risk_lines: Vec<String>,
    position_lines: Vec<String>,
    alert_lines: Vec<String>,
}

impl MonitorApp {
    fn new(symbols: Vec<String>) -> Self {
        let syms = if symbols.is_empty() {
            vec!["BTC-USDT".to_string(), "ETH-USDT".to_string()]
        } else {
            symbols
        };
        let market_lines: Vec<String> = syms
            .iter()
            .map(|s| format!("{}  --.--  --.--  --.--  --.--", s))
            .collect();

        Self {
            connection: "未连接".to_string(),
            symbols: syms,
            focus: 0,
            tick_count: 0,
            market_lines,
            strategy_lines: vec![
                "无活跃策略".to_string(),
            ],
            risk_lines: vec![
                "总敞口    $0.00".to_string(),
                "保证金率  0.0%".to_string(),
                "当日盈亏  $0.00".to_string(),
                "最大回撤  0.00%".to_string(),
            ],
            position_lines: vec![
                "无持仓".to_string(),
            ],
            alert_lines: vec![
                "等待数据...".to_string(),
            ],
        }
    }

    fn focus_next(&mut self) {
        self.focus = (self.focus + 1) % 5;
    }

    fn refresh(&mut self) {
        self.tick_count = 0;
    }

    fn tick(&mut self) {
        self.tick_count += 1;
        self.connection = format!("运行中 ({} ticks)", self.tick_count);
    }

    fn render(&self, f: &mut Frame) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // 标题栏
                Constraint::Min(0),     // 内容区
                Constraint::Length(1),  // 状态栏
            ])
            .split(f.area());

        // 标题栏
        let title = Line::from(Span::styled(
            format!(
                " taiji monitor  [Tab:切换面板] [r:刷新] [q:退出]    连接: {}",
                self.connection
            ),
            Style::default().fg(Color::Cyan),
        ));
        f.render_widget(
            Paragraph::new(title).block(Block::default().borders(Borders::NONE)),
            areas[0],
        );

        // 内容区（四象限）
        let content = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(areas[1]);

        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content[0]);

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content[1]);

        // 左上：行情速览
        let market_focused = self.focus == 0;
        self.render_pane(
            f,
            left[0],
            "行情速览",
            &self.market_lines,
            market_focused,
        );

        // 右上：策略状态
        let strategy_focused = self.focus == 1;
        self.render_pane(
            f,
            right[0],
            "策略状态",
            &self.strategy_lines,
            strategy_focused,
        );

        // 中左：风控指标
        let risk_focused = self.focus == 2;
        self.render_pane(
            f,
            left[1],
            "风控指标",
            &self.risk_lines,
            risk_focused,
        );

        // 中右：持仓列表
        let pos_focused = self.focus == 3;
        self.render_pane(
            f,
            right[1],
            "持仓列表",
            &self.position_lines,
            pos_focused,
        );

        // 底部横条：告警日志
        let alert_focused = self.focus == 4;
        let alert_area = Rect::new(
            areas[1].x,
            areas[1].y + areas[1].height - 5,
            areas[1].width,
            5,
        );
        self.render_pane(
            f,
            alert_area,
            "告警日志",
            &self.alert_lines,
            alert_focused,
        );

        // 状态栏
        let status = Line::from(Span::styled(
            format!(" TUI 仪表盘 v{} | 按 q 退出", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(
            Paragraph::new(status).block(Block::default().borders(Borders::NONE)),
            areas[2],
        );
    }

    fn render_pane(
        &self,
        f: &mut Frame,
        area: Rect,
        title: &str,
        lines: &[String],
        focused: bool,
    ) {
        let border_style = if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        let text: Vec<Line> = lines
            .iter()
            .map(|l| Line::from(Span::raw(l)))
            .collect();

        let para = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(para, area);
    }
}
