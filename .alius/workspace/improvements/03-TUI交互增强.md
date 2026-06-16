# 03 - TUI 交互增强

## 📋 概述

在保持 Alius 现有 Rust + Ratatui 架构的基础上，借鉴竞品（特别是 claude-code 的 React Ink 设计）的交互模式，提升 TUI 用户体验。

**核心原则：保持 Ratatui 风格，增强交互能力，不引入 Node.js/React 依赖**

## 🎯 目标

1. 优化现有 Ratatui 组件的交互性
2. 借鉴 Ink 的组件化思想，重构 TUI 模块
3. 增强键盘导航和快捷键系统
4. 改进视觉反馈和动画效果
5. 提升大数据量场景的性能

## 📊 现状分析

### Alius 当前优势
- ✅ 纯 Rust 实现，性能优秀
- ✅ Ratatui 提供了稳定的 TUI 基础
- ✅ 已有基础的焦点管理和滚动
- ✅ 支持鼠标交互
- ✅ 多面板布局（Conversation、Plans、Agent Team）

### 竞品借鉴点

#### claude-code (React Ink)
- **组件化设计**: 每个 UI 元素都是独立组件
- **声明式布局**: 使用 JSX 描述 UI 结构
- **丰富的交互组件**: Select、Confirm、MultiSelect、PasswordInput
- **动态布局**: Flexbox 布局，自适应调整
- **加载状态**: Spinner、Progress Bar、Skeleton

**关键启示**: 虽然不使用 React，但可以借鉴组件化思想

## 💡 改进方案

### 1. 组件化重构

**目标**: 在 Rust 中实现类似 React 组件的模式

```rust
// entrypoints/cli/src/tui/components/mod.rs
pub mod spinner;
pub mod progress;
pub mod select;
pub mod confirm;
pub mod text_input;
pub mod list;
pub mod table;
pub mod tree;

use ratatui::prelude::*;
use ratatui::widgets::Widget;

/// 组件 trait - 类似 React 组件
pub trait Component {
    /// 组件状态类型
    type State;
    
    /// 组件属性类型
    type Props;
    
    /// 渲染组件
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget;
    
    /// 处理键盘事件
    fn handle_key(&mut self, state: &mut Self::State, key: crossterm::event::KeyEvent) -> ComponentAction {
        ComponentAction::None
    }
    
    /// 处理鼠标事件
    fn handle_mouse(&mut self, state: &mut Self::State, mouse: crossterm::event::MouseEvent) -> ComponentAction {
        ComponentAction::None
    }
    
    /// 组件挂载时调用
    fn on_mount(&mut self, state: &mut Self::State) {}
    
    /// 组件卸载时调用
    fn on_unmount(&mut self, state: &mut Self::State) {}
}

/// 组件动作
#[derive(Debug, Clone)]
pub enum ComponentAction {
    None,
    FocusNext,
    FocusPrev,
    Submit(String),
    Cancel,
    Custom(String),
}
```

### 2. 增强的选择器组件

```rust
// entrypoints/cli/src/tui/components/select.rs
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub struct SelectProps {
    pub title: String,
    pub items: Vec<String>,
    pub help: Option<String>,
}

pub struct SelectState {
    pub selected: usize,
    pub list_state: ListState,
    pub focused: bool,
}

pub struct SelectComponent;

impl Component for SelectComponent {
    type State = SelectState;
    type Props = SelectProps;
    
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget {
        let items: Vec<ListItem> = props.items.iter().enumerate().map(|(i, item)| {
            let symbol = if i == state.selected {
                if state.focused {
                    "▶ "
                } else {
                    "> "
                }
            } else {
                "  "
            };
            
            let style = if i == state.selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            ListItem::new(format!("{}{}", symbol, item)).style(style)
        }).collect();
        
        let list = List::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(props.title.as_str())
                .border_style(if state.focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }));
        
        list
    }
    
    fn handle_key(&mut self, state: &mut Self::State, key: crossterm::event::KeyEvent) -> ComponentAction {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if state.selected > 0 {
                    state.selected -= 1;
                    state.list_state.select(Some(state.selected));
                }
                ComponentAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.selected += 1;
                state.list_state.select(Some(state.selected));
                ComponentAction::None
            }
            KeyCode::Enter => {
                ComponentAction::Submit(state.selected.to_string())
            }
            KeyCode::Esc => {
                ComponentAction::Cancel
            }
            KeyCode::Tab => {
                ComponentAction::FocusNext
            }
            _ => ComponentAction::None
        }
    }
}
```

### 3. 多选组件

```rust
// entrypoints/cli/src/tui/components/multi_select.rs
pub struct MultiSelectState {
    pub cursor: usize,
    pub selected: HashSet<usize>,
    pub focused: bool,
}

impl Component for MultiSelectComponent {
    type State = MultiSelectState;
    type Props = SelectProps;
    
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget {
        let items: Vec<ListItem> = props.items.iter().enumerate().map(|(i, item)| {
            let cursor = if i == state.cursor { "▶ " } else { "  " };
            let checkbox = if state.selected.contains(&i) { "[✓]" } else { "[ ]" };
            
            let style = if i == state.cursor {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            
            ListItem::new(format!("{}{} {}", cursor, checkbox, item)).style(style)
        }).collect();
        
        List::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(format!("{} (Space to toggle, Enter to confirm)", props.title))
                .border_style(if state.focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }))
    }
    
    fn handle_key(&mut self, state: &mut Self::State, key: crossterm::event::KeyEvent) -> ComponentAction {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if state.cursor > 0 {
                    state.cursor -= 1;
                }
                ComponentAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.cursor += 1;
                ComponentAction::None
            }
            KeyCode::Char(' ') => {
                if state.selected.contains(&state.cursor) {
                    state.selected.remove(&state.cursor);
                } else {
                    state.selected.insert(state.cursor);
                }
                ComponentAction::None
            }
            KeyCode::Enter => {
                let selected: Vec<String> = state.selected.iter()
                    .map(|i| i.to_string())
                    .collect();
                ComponentAction::Submit(selected.join(","))
            }
            KeyCode::Esc => {
                ComponentAction::Cancel
            }
            _ => ComponentAction::None
        }
    }
}
```

### 4. 增强的 Spinner 组件

```rust
// entrypoints/cli/src/tui/components/spinner.rs
use std::time::{Duration, Instant};

pub struct SpinnerProps {
    pub message: String,
    pub style: SpinnerStyle,
}

pub enum SpinnerStyle {
    Dots,      // ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏
    Line,      // ⠁⠂⠄⡀⢀⠠⠐⠈
    Arc,       // ◜◠◝◞◡◟
    Arrow,     // ←↖↑↗→↘↓↙
    Clock,     // 🕐🕑🕒🕓🕔🕕
}

pub struct SpinnerState {
    pub started_at: Instant,
    pub frame: usize,
}

impl SpinnerStyle {
    fn frames(&self) -> &[&str] {
        match self {
            SpinnerStyle::Dots => &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            SpinnerStyle::Line => &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"],
            SpinnerStyle::Arc => &["◜", "◠", "◝", "◞", "◡", "◟"],
            SpinnerStyle::Arrow => &["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
            SpinnerStyle::Clock => &["🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚", "🕛"],
        }
    }
    
    fn interval(&self) -> Duration {
        match self {
            SpinnerStyle::Dots | SpinnerStyle::Line => Duration::from_millis(80),
            SpinnerStyle::Arc | SpinnerStyle::Arrow => Duration::from_millis(100),
            SpinnerStyle::Clock => Duration::from_millis(120),
        }
    }
}

impl Component for SpinnerComponent {
    type State = SpinnerState;
    type Props = SpinnerProps;
    
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget {
        let frames = props.style.frames();
        let frame_idx = state.frame % frames.len();
        let symbol = frames[frame_idx];
        
        let elapsed = state.started_at.elapsed();
        let elapsed_str = if elapsed.as_secs() > 60 {
            format!("{}m{}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
        } else {
            format!("{}s", elapsed.as_secs())
        };
        
        Paragraph::new(format!("{} {} ({})", symbol, props.message, elapsed_str))
            .style(Style::default().fg(Color::Cyan))
    }
    
    fn on_mount(&mut self, state: &mut Self::State) {
        state.started_at = Instant::now();
        state.frame = 0;
    }
}

// 使用定时器自动更新 frame
pub fn update_spinner(state: &mut SpinnerState, style: &SpinnerStyle) {
    let elapsed = state.started_at.elapsed();
    let frames = style.frames().len();
    state.frame = (elapsed.as_millis() / style.interval().as_millis()) as usize % frames;
}
```

### 5. 进度条组件

```rust
// entrypoints/cli/src/tui/components/progress.rs
pub struct ProgressProps {
    pub total: u64,
    pub label: String,
    pub show_percentage: bool,
}

pub struct ProgressState {
    pub current: u64,
    pub started_at: Instant,
}

impl Component for ProgressComponent {
    type State = ProgressState;
    type Props = ProgressProps;
    
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget {
        let percentage = if props.total > 0 {
            (state.current as f64 / props.total as f64 * 100.0) as u16
        } else {
            0
        };
        
        let elapsed = state.started_at.elapsed();
        let rate = if elapsed.as_secs() > 0 {
            state.current / elapsed.as_secs()
        } else {
            0
        };
        
        let eta = if rate > 0 && state.current < props.total {
            let remaining = props.total - state.current;
            Duration::from_secs(remaining / rate)
        } else {
            Duration::from_secs(0)
        };
        
        let label = if props.show_percentage {
            format!("{} - {}% ({}/{}) - ETA: {}s", 
                props.label, percentage, state.current, props.total, eta.as_secs())
        } else {
            format!("{} - {}/{}", props.label, state.current, props.total)
        };
        
        LineGauge::default()
            .block(Block::default().borders(Borders::ALL).title(label))
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .ratio(percentage as f64 / 100.0)
    }
}
```

### 6. 树形视图组件

```rust
// entrypoints/cli/src/tui/components/tree.rs
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
}

pub struct TreeProps {
    pub root: TreeNode,
    pub title: String,
}

pub struct TreeState {
    pub selected_path: Vec<usize>,
    pub focused: bool,
}

impl Component for TreeComponent {
    type State = TreeState;
    type Props = TreeProps;
    
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget {
        let mut lines = Vec::new();
        self.render_node(&props.root, &mut lines, 0, &[], state);
        
        List::new(lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(props.title.as_str())
                .border_style(if state.focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }))
    }
    
    fn handle_key(&mut self, state: &mut Self::State, key: crossterm::event::KeyEvent) -> ComponentAction {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                // 移动到上一个节点
                self.move_selection_up(state);
                ComponentAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // 移动到下一个节点
                self.move_selection_down(state);
                ComponentAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                // 展开节点
                self.expand_node(state);
                ComponentAction::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // 折叠节点或移动到父节点
                self.collapse_or_parent(state);
                ComponentAction::None
            }
            KeyCode::Enter => {
                ComponentAction::Submit(self.get_selected_id(state))
            }
            _ => ComponentAction::None
        }
    }
}

impl TreeComponent {
    fn render_node(
        &self,
        node: &TreeNode,
        lines: &mut Vec<ListItem>,
        depth: usize,
        path: &[usize],
        state: &TreeState,
    ) {
        let indent = "  ".repeat(depth);
        let expand_symbol = if node.children.is_empty() {
            "  "
        } else if node.expanded {
            "▼ "
        } else {
            "▶ "
        };
        
        let is_selected = path == state.selected_path.as_slice();
        let style = if is_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        lines.push(ListItem::new(format!("{}{}{}", indent, expand_symbol, node.label)).style(style));
        
        if node.expanded {
            for (i, child) in node.children.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(i);
                self.render_node(child, lines, depth + 1, &child_path, state);
            }
        }
    }
}
```

### 7. 表格组件增强

```rust
// entrypoints/cli/src/tui/components/table.rs
pub struct TableProps {
    pub title: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub column_widths: Vec<Constraint>,
}

pub struct TableState {
    pub selected_row: usize,
    pub selected_col: usize,
    pub sort_column: Option<usize>,
    pub sort_ascending: bool,
    pub focused: bool,
}

impl Component for TableComponent {
    type State = TableState;
    type Props = TableProps;
    
    fn render(&self, props: &Self::Props, state: &Self::State) -> impl Widget {
        // 头部行
        let header_cells = props.headers.iter().enumerate().map(|(i, h)| {
            let sort_indicator = if state.sort_column == Some(i) {
                if state.sort_ascending { " ↑" } else { " ↓" }
            } else {
                ""
            };
            Cell::from(format!("{}{}", h, sort_indicator))
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        });
        let header = Row::new(header_cells).height(1).bottom_margin(1);
        
        // 数据行
        let rows = props.rows.iter().enumerate().map(|(row_idx, row)| {
            let cells = row.iter().enumerate().map(|(col_idx, cell)| {
                let is_selected = row_idx == state.selected_row && col_idx == state.selected_col;
                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else if row_idx == state.selected_row {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                Cell::from(cell.as_str()).style(style)
            });
            Row::new(cells).height(1)
        });
        
        Table::new(rows, props.column_widths.clone())
            .header(header)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(props.title.as_str())
                .border_style(if state.focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }))
    }
    
    fn handle_key(&mut self, state: &mut Self::State, key: crossterm::event::KeyEvent) -> ComponentAction {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if state.selected_row > 0 {
                    state.selected_row -= 1;
                }
                ComponentAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.selected_row += 1;
                ComponentAction::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if state.selected_col > 0 {
                    state.selected_col -= 1;
                }
                ComponentAction::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                state.selected_col += 1;
                ComponentAction::None
            }
            KeyCode::Char('s') => {
                // 按当前列排序
                if state.sort_column == Some(state.selected_col) {
                    state.sort_ascending = !state.sort_ascending;
                } else {
                    state.sort_column = Some(state.selected_col);
                    state.sort_ascending = true;
                }
                ComponentAction::Custom("sort".to_string())
            }
            KeyCode::Enter => {
                ComponentAction::Submit(format!("{},{}", state.selected_row, state.selected_col))
            }
            _ => ComponentAction::None
        }
    }
}
```

### 8. 焦点管理系统增强

```rust
// entrypoints/cli/src/tui/focus.rs
use std::collections::HashMap;

pub struct FocusManager {
    components: Vec<String>,
    current: usize,
    focus_history: Vec<usize>,
    shortcuts: HashMap<String, usize>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            current: 0,
            focus_history: Vec::new(),
            shortcuts: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, id: impl Into<String>) -> usize {
        let id = id.into();
        let index = self.components.len();
        self.components.push(id);
        index
    }
    
    pub fn register_with_shortcut(&mut self, id: impl Into<String>, shortcut: impl Into<String>) -> usize {
        let index = self.register(id);
        self.shortcuts.insert(shortcut.into(), index);
        index
    }
    
    pub fn focus_next(&mut self) {
        self.focus_history.push(self.current);
        self.current = (self.current + 1) % self.components.len();
    }
    
    pub fn focus_prev(&mut self) {
        self.focus_history.push(self.current);
        if self.current == 0 {
            self.current = self.components.len() - 1;
        } else {
            self.current -= 1;
        }
    }
    
    pub fn focus_by_id(&mut self, id: &str) -> bool {
        if let Some(index) = self.components.iter().position(|c| c == id) {
            self.focus_history.push(self.current);
            self.current = index;
            true
        } else {
            false
        }
    }
    
    pub fn focus_by_shortcut(&mut self, shortcut: &str) -> bool {
        if let Some(&index) = self.shortcuts.get(shortcut) {
            self.focus_history.push(self.current);
            self.current = index;
            true
        } else {
            false
        }
    }
    
    pub fn focus_back(&mut self) -> bool {
        if let Some(prev) = self.focus_history.pop() {
            self.current = prev;
            true
        } else {
            false
        }
    }
    
    pub fn is_focused(&self, index: usize) -> bool {
        self.current == index
    }
    
    pub fn current_id(&self) -> Option<&str> {
        self.components.get(self.current).map(|s| s.as_str())
    }
}
```

### 9. 在 Workspace 中集成组件系统

```rust
// entrypoints/cli/src/tui/workspace/mod.rs
use super::components::*;
use super::focus::FocusManager;

struct WorkspaceState {
    // ... 现有字段
    
    // 新增：组件系统
    focus_manager: FocusManager,
    spinner: Option<(SpinnerComponent, SpinnerState)>,
    progress: Option<(ProgressComponent, ProgressState)>,
}

impl WorkspaceState {
    fn new(...) -> Self {
        let mut focus_manager = FocusManager::new();
        
        // 注册各个焦点区域
        focus_manager.register_with_shortcut("input", "i");
        focus_manager.register_with_shortcut("conversation", "c");
        focus_manager.register_with_shortcut("plans", "p");
        focus_manager.register_with_shortcut("agent_team", "a");
        
        Self {
            // ...
            focus_manager,
            spinner: None,
            progress: None,
        }
    }
    
    fn handle_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        // 快捷键切换焦点
        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('i') => {
                    self.focus_manager.focus_by_shortcut("i");
                    return WorkspaceAction::None;
                }
                KeyCode::Char('c') => {
                    self.focus_manager.focus_by_shortcut("c");
                    return WorkspaceAction::None;
                }
                KeyCode::Char('p') => {
                    self.focus_manager.focus_by_shortcut("p");
                    return WorkspaceAction::None;
                }
                _ => {}
            }
        }
        
        // ... 现有键盘处理逻辑
    }
    
    fn show_spinner(&mut self, message: impl Into<String>) {
        let mut spinner = SpinnerComponent;
        let mut state = SpinnerState {
            started_at: Instant::now(),
            frame: 0,
        };
        spinner.on_mount(&mut state);
        
        self.spinner = Some((spinner, state));
    }
    
    fn hide_spinner(&mut self) {
        if let Some((mut spinner, mut state)) = self.spinner.take() {
            spinner.on_unmount(&mut state);
        }
    }
}
```

## 🚀 实施步骤

### 阶段 1：组件基础架构（1周）
1. ✅ 定义 Component trait
2. ✅ 实现 FocusManager
3. ✅ 重构现有组件适配新架构
4. ✅ 单元测试

### 阶段 2：基础交互组件（2周）
1. ✅ 实现 Select 组件
2. ✅ 实现 MultiSelect 组件
3. ✅ 实现 Confirm 组件
4. ✅ 实现增强的 TextInput
5. ✅ 集成测试

### 阶段 3：高级可视组件（2周）
1. ✅ 实现 Spinner 组件
2. ✅ 实现 Progress 组件
3. ✅ 实现 Tree 组件
4. ✅ 增强 Table 组件
5. ✅ 性能优化

### 阶段 4：Workspace 集成（1周）
1. ✅ 集成组件系统到 Workspace
2. ✅ 优化焦点管理
3. ✅ 添加快捷键提示
4. ✅ 用户测试和反馈

## 📈 预期收益

1. **代码复用**: 组件化减少重复代码
2. **易维护性**: 清晰的组件边界
3. **一致性**: 统一的交互模式
4. **可测试性**: 组件独立测试
5. **用户体验**: 更流畅的交互

## ⚠️ 注意事项

1. **保持性能**: 组件抽象不能影响 Ratatui 的渲染性能
2. **向后兼容**: 渐进式重构，不破坏现有功能
3. **内存管理**: 注意组件状态的生命周期
4. **类型安全**: 充分利用 Rust 的类型系统

---

最后更新：2026-06-15
