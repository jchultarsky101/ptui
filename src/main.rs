use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::trace;
use log::{debug, warn};
use std::env;
use std::{cell::RefCell, error::Error, fmt, io::ErrorKind};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        TableState, Wrap,
    },
    Frame, Terminal,
};
use tui_logger::*;
use tui_textarea::{self, Input, TextArea};
use uuid::Uuid;

const NORMAL_MODE_HELP: &str = r#"
Normal Mode:

<q>    Exit the program
<f>    Switch to Folder mode
<m>    Switch to Model mode

Press any key to exit this help
"#;

const SEARCH_MODE_HELP: &str = r#"
Search Mode:

<Esc>          Exit to Normal mode
<Backspace>    Delete the character left of the cursor
<Left Arrow>   Move cursor left
<Right Arrow>  Move cursor right
<Home>         Go to beginning of line
<End>          Go to end of line
<Delete>       Delete character under cursor
<Enter>        Execute search
"#;

const FOLDER_MODE_HELP: &str = r#"
Folder Mode:

<Esc>    Exit to Normal mode
<r>      Reload the list of folders
"#;

const MODEL_MODE_HELP: &str = r#"
Model Mode:

<Esc>    Exit to Normal mode
<r>      Reload the list of models
"#;

const MATCH_MODE_HELP: &str = r#"
Match Mode:

<Esc>    Exit to Normal mode
<r>      Regenerate matches
"#;

#[derive(Debug, Clone, Copy)]
enum InputMode {
    Normal,
    Search,
    Folder,
    Model,
    Match,
    Help,
}

#[derive(Debug, Clone, Copy)]
enum HelpType {
    General,
    Search,
    Folder,
    Model,
    Match,
}

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InputMode::Normal => write!(f, "Normal"),
            InputMode::Search => write!(f, "Search"),
            InputMode::Folder => write!(f, "Folder"),
            InputMode::Model => write!(f, "Model "),
            InputMode::Match => write!(f, "Match "),
            InputMode::Help => write!(f, "Help  "),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ModelState {
    Received,
    Indexing,
    Ready,
}

struct Model {
    id: Uuid,
    name: String,
    state: ModelState,
}

#[derive(Debug, Clone, PartialEq)]
struct Folder {
    id: usize,
    name: String,
}

impl Folder {
    pub fn new(id: usize, name: String) -> Folder {
        Folder { id, name }
    }
}

struct State<'a> {
    mode: InputMode,
    previous_mode: InputMode,
    search_field: TextArea<'a>,
    folder_list: StatefulList<String>,
    models_table: StatefulTable<'a, String>,
    status_line: String,
    help_text: String,
    display_help: bool,
}

impl<'a> State<'a> {
    pub fn new() -> State<'static> {
        State {
            mode: InputMode::Normal,
            previous_mode: InputMode::Normal,
            search_field: TextArea::default(),
            folder_list: StatefulList::default(), //with_items(vec![]),
            models_table: StatefulTable::with_columns(vec!["UUID", "Name", "Status"]),
            status_line: String::new(),
            help_text: String::default(),
            display_help: false,
        }
    }

    pub fn initialize(&mut self) {
        self.add_folder(Folder::new(1, String::from("First")));
        self.add_folder(Folder::new(2, String::from("Second")));
        self.add_folder(Folder::new(2, String::from("Third")));
        self.add_folder(Folder::new(2, String::from("Fourth")));
        self.add_folder(Folder::new(2, String::from("Fifth")));

        self.search_field.set_cursor_line_style(Style::default());

        self.models_table.add_row(vec![
            String::from("UUID_1"),
            String::from("Name_1"),
            String::from("Status_1"),
        ]);
        self.models_table.add_row(vec![
            String::from("UUID_2"),
            String::from("Name_2"),
            String::from("Status_2"),
        ]);
        self.models_table.add_row(vec![
            String::from("UUID_3"),
            String::from("Name_3"),
            String::from("Status_3"),
        ]);
    }

    pub fn add_folder(&mut self, folder: Folder) {
        self.folder_list.items.push(folder.name);
    }

    pub fn change_mode(&mut self, mode: InputMode) {
        self.previous_mode = self.mode;
        self.status_line.clear();
        self.mode = mode;

        debug!("Changed mode from {} to {}", self.previous_mode, self.mode);

        match self.mode {
            InputMode::Normal => {
                self.status_line = String::from("Press <h> for help or <q> to exit");
            }
            InputMode::Search => {
                self.status_line =
                    String::from("Press <Esc> to return to Normal mode or <Ctrl-h> for help");
            }
            InputMode::Folder => {
                self.status_line = String::from(
                    "Press <Esc> to return to Normal mode, <h> for help, or <Tab> for Model mode",
                );
            }
            InputMode::Model => {
                self.status_line = String::from(
                    "Press <Esc> to return to Normal mode, <h> for help, or <Tab> for Folder mode",
                );
            }
            InputMode::Match => {
                self.status_line =
                    String::from("Press <Esc> to return to Normal mode, <h> for help");
            }
            InputMode::Help => {
                self.status_line = String::from("Press any key to exit the help");
            }
        }
    }

    pub fn set_help(&mut self, help_type: HelpType) {
        match help_type {
            HelpType::General => {
                self.help_text = String::from(NORMAL_MODE_HELP);
                self.display_help = true;
            }
            HelpType::Search => {
                self.help_text = String::from(SEARCH_MODE_HELP);
                self.display_help = true;
            }
            HelpType::Folder => {
                self.help_text = String::from(FOLDER_MODE_HELP);
                self.display_help = true;
            }
            HelpType::Model => {
                self.help_text = String::from(MODEL_MODE_HELP);
                self.display_help = true;
            }
            HelpType::Match => {
                self.help_text = String::from(MATCH_MODE_HELP);
                self.display_help = true;
            }
        }
    }

    pub fn hide_help(&mut self) {
        self.display_help = false;
    }

    pub fn show_help(&self) -> bool {
        self.display_help
    }
}

struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn first(&mut self) {
        self.state.select(Some(0));
    }

    fn last(&mut self) {
        if self.items.is_empty() {
            self.first();
        } else {
            self.state.select(Some(self.items.len() - 1));
        }
    }

    fn unselect(&mut self) {
        self.state.select(None);
    }
}

impl<T> Default for StatefulList<T> {
    fn default() -> StatefulList<T> {
        Self::with_items(vec![])
    }
}

struct StatefulTable<'a, T> {
    state: TableState,
    columns: Vec<&'a str>,
    rows: Vec<Vec<T>>,
}

impl<'a, T> StatefulTable<'a, T> {
    fn with_columns(columns: Vec<&'a str>) -> StatefulTable<'a, T> {
        StatefulTable {
            state: TableState::default(),
            columns,
            rows: vec![],
        }
    }

    fn add_row(&mut self, row: Vec<T>) {
        self.rows.push(row);
    }

    fn row(&self, index: usize) -> Option<&Vec<T>> {
        self.rows.get(index)
    }

    fn delete_row(&mut self, index: usize) {
        self.rows.remove(index);
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.rows.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.rows.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let log_level = env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .to_lowercase();

    let level_filter = match log_level.as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => {
            eprintln!("Invalid log level: {}. Defaulting to 'info'", log_level);
            log::LevelFilter::Info
        }
    };

    tui_logger::init_logger(level_filter).unwrap();
    tui_logger::set_default_level(level_filter);

    // Prepare the state
    let state = RefCell::new(State::new());

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, state);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    if let Err(e) = result {
        eprintln!("{}", e.to_string());
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    state: RefCell<State>,
) -> Result<(), std::io::Error> {
    state.borrow_mut().initialize();

    loop {
        terminal.draw(|f| ui(f, &state))?;

        let mut state = state.borrow_mut();
        if let event = event::read()? {
            match state.mode {
                InputMode::Normal => match event {
                    Event::Key(key) => match key {
                        KeyEvent {
                            code: KeyCode::Char('q'),
                            ..
                        } => {
                            return Ok(());
                        }
                        KeyEvent {
                            code: KeyCode::Char('f'),
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Tab, ..
                        } => {
                            state.change_mode(InputMode::Folder);
                        }
                        KeyEvent {
                            code: KeyCode::Char('s'),
                            ..
                        } => {
                            state.change_mode(InputMode::Search);
                        }
                        KeyEvent {
                            code: KeyCode::Char('m'),
                            ..
                        } => state.change_mode(InputMode::Model),
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            ..
                        } => state.change_mode(InputMode::Match),
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            ..
                        } => {
                            state.set_help(HelpType::General);
                            state.change_mode(InputMode::Help);
                        }
                        _ => {
                            warn!("Unsupported key binding. Press <h> for help");
                            state.status_line = String::from("Press <h> for help or <q> to exit");
                        }
                    },
                    _ => {}
                },
                InputMode::Search => match event {
                    Event::Key(key) => match key {
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        } => state.change_mode(InputMode::Normal),
                        KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => {
                            let text = state.search_field.lines()[0].clone();
                            debug!("Search for \"{}\"", text);
                        }
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            state.set_help(HelpType::Search);
                            state.change_mode(InputMode::Help);
                        }
                        input => {
                            let input: Input = Input {
                                ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
                                alt: key.modifiers.contains(KeyModifiers::ALT),
                                key: match key.code {
                                    KeyCode::Char(c) => tui_textarea::Key::Char(c),
                                    KeyCode::Backspace => tui_textarea::Key::Backspace,
                                    KeyCode::Enter => tui_textarea::Key::Enter,
                                    KeyCode::Left => tui_textarea::Key::Left,
                                    KeyCode::Right => tui_textarea::Key::Right,
                                    KeyCode::Up => tui_textarea::Key::Up,
                                    KeyCode::Down => tui_textarea::Key::Down,
                                    KeyCode::Tab => tui_textarea::Key::Tab,
                                    KeyCode::Delete => tui_textarea::Key::Delete,
                                    KeyCode::Home => tui_textarea::Key::Home,
                                    KeyCode::End => tui_textarea::Key::End,
                                    KeyCode::PageUp => tui_textarea::Key::PageUp,
                                    KeyCode::PageDown => tui_textarea::Key::PageDown,
                                    KeyCode::Esc => tui_textarea::Key::Esc,
                                    KeyCode::F(x) => tui_textarea::Key::F(x),
                                    _ => tui_textarea::Key::Null,
                                },
                            };
                            state.search_field.input(input);
                        }
                    },
                    _ => {}
                },
                InputMode::Folder => match event {
                    Event::Key(key) => match key {
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        } => state.change_mode(InputMode::Normal),
                        KeyEvent {
                            code: KeyCode::Tab, ..
                        } => state.change_mode(InputMode::Model),
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            ..
                        } => {
                            state.set_help(HelpType::Folder);
                            state.change_mode(InputMode::Help);
                        }
                        KeyEvent {
                            code: KeyCode::Up, ..
                        } => {
                            state.folder_list.previous();
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            ..
                        } => {
                            state.folder_list.next();
                        }
                        KeyEvent {
                            code: KeyCode::Home,
                            ..
                        } => {
                            state.folder_list.first();
                        }
                        KeyEvent {
                            code: KeyCode::End, ..
                        } => {
                            state.folder_list.last();
                        }
                        KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => {
                            let selected = state.folder_list.state.selected();
                            match selected {
                                Some(index) => {
                                    let selected_item =
                                        state.folder_list.items.get(index).ok_or(Err::<
                                            String,
                                            std::io::Error,
                                        >(
                                            std::io::Error::new(
                                                ErrorKind::Other,
                                                "Incompatible folder list item",
                                            ),
                                        ));
                                    debug!("Selected folder \"{}\"", selected_item.unwrap());
                                }
                                None => warn!("No folder selected"),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                },
                InputMode::Model => match event {
                    Event::Key(key) => match key {
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        } => state.change_mode(InputMode::Normal),
                        KeyEvent {
                            code: KeyCode::Tab, ..
                        } => state.change_mode(InputMode::Folder),
                        KeyEvent {
                            code: KeyCode::Up, ..
                        } => {
                            state.models_table.previous();
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            ..
                        } => {
                            state.models_table.next();
                        }
                        KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => {
                            let selected = state.models_table.state.selected();
                            match selected {
                                Some(index) => {
                                    let selected_row =
                                        state.models_table.rows.get(index).ok_or(Err::<
                                            String,
                                            std::io::Error,
                                        >(
                                            std::io::Error::new(
                                                ErrorKind::Other,
                                                "Incompatible model row item",
                                            ),
                                        ));
                                    debug!("Selected model \"{}\"", selected_row.unwrap()[0]);
                                }
                                None => warn!("No model selected"),
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            ..
                        } => {
                            state.set_help(HelpType::Model);
                            state.change_mode(InputMode::Help);
                        }
                        _ => {}
                    },
                    _ => {}
                },
                InputMode::Match => match event {
                    Event::Key(key) => match key {
                        KeyEvent {
                            code: KeyCode::Esc, ..
                        } => state.change_mode(InputMode::Normal),
                        KeyEvent {
                            code: KeyCode::Char('h'),
                            ..
                        } => {
                            state.set_help(HelpType::Match);
                            state.change_mode(InputMode::Help);
                        }
                        _ => {}
                    },
                    _ => {}
                },
                InputMode::Help => match event {
                    Event::Key(key) => match key {
                        _ => {
                            let previous_mode = state.previous_mode;
                            state.hide_help();
                            state.change_mode(previous_mode);
                        }
                    },
                    _ => {}
                },
            }
        };
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>) {
    let size = f.size();

    // Main container
    let app_container = Block::default()
        .title(Span::styled(
            "Physna TUI",
            Style::default()
                .fg(Color::White)
                //.bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(app_container, size);

    let container_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(10),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(size);

    search_section(f, state, container_chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(container_chunks[1]);

    folders_section(f, state, content_chunks[0]);

    let models_list_section_block = Block::default()
        .title("Models")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(match state.borrow().mode {
            InputMode::Model => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        });
    f.render_widget(models_list_section_block, content_chunks[1]);

    models_section(f, state, content_chunks[1]);

    let tui_w: TuiLoggerWidget = TuiLoggerWidget::default()
        .block(
            Block::default()
                .title("Log")
                .border_style(Style::default().fg(Color::White).bg(Color::Black))
                .borders(Borders::ALL),
        )
        .style_error(Style::default().fg(Color::Red))
        .style_debug(Style::default().fg(Color::Green))
        .style_warn(Style::default().fg(Color::Yellow))
        .style_trace(Style::default().fg(Color::Magenta))
        .style_info(Style::default().fg(Color::Cyan))
        .output_separator('|')
        .output_timestamp(Some("%F %H:%M:%S%.3f".to_string()))
        .output_level(Some(TuiLoggerLevelOutput::Long))
        .output_target(false)
        .output_file(false)
        .output_line(false)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    f.render_widget(tui_w, container_chunks[2]);

    let status_block = Block::default().borders(Borders::NONE);
    f.render_widget(status_block, container_chunks[3]);
    status_section(f, state, container_chunks[3]);

    help_section(f, state);
}

fn folders_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>, area: Rect) {
    let mut state = state.borrow_mut();

    let folder_list_chunk = Layout::default()
        .margin(2)
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(area);

    let folders_list_section_block = Block::default()
        .title("Folders")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(match state.mode {
            InputMode::Folder => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        });
    f.render_widget(folders_list_section_block, area);

    let visible_items: Vec<ListItem> = state
        .folder_list
        .items
        .iter()
        .cloned()
        .map(|i| ListItem::new(i))
        .collect();

    let selection_indicator = format!(" {}", char::from_u32(0x25B6).unwrap());
    let folder_list = List::new(visible_items)
        .highlight_style(
            Style::default().add_modifier(Modifier::REVERSED),
            // .fg(Color::Black)
            // .bg(Color::LightBlue)
            // .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(selection_indicator.as_str());

    f.render_stateful_widget(
        folder_list,
        folder_list_chunk[0],
        &mut state.folder_list.state,
    );
}

fn status_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>, area: Rect) {
    let state = state.borrow();
    let text = vec![Spans::from(vec![
        Span::styled(
            format!(" {} ", char::from_u32(0x25B6).unwrap()),
            Style::default().fg(Color::Blue),
        ),
        Span::styled(
            format!("[{}]", state.mode),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::styled(
            format!(" {}", state.status_line),
            Style::default().fg(Color::Green),
        ),
    ])];
    let status_chunk = Layout::default()
        .horizontal_margin(1)
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(area);
    let status = Paragraph::new(text).style(Style::default().fg(Color::Green));
    f.render_widget(status, status_chunk[0]);
}

fn search_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>, area: Rect) {
    let mut state = state.borrow_mut();

    state.search_field.set_style(Style::default());
    let search_block = Block::default()
        .title("Search")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(match state.mode {
            InputMode::Search => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        });
    f.render_widget(search_block.clone(), area);

    let margin = Margin {
        horizontal: 2,
        vertical: 1,
    };

    let edit_area = area.inner(&margin);
    f.render_widget(state.search_field.widget(), edit_area);
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn help_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>) {
    let state = state.borrow();

    if state.show_help() {
        let block = Block::default().title("Help").borders(Borders::ALL);
        let area = centered_rect(50, 50, f.size());
        f.render_widget(Clear, area); //this clears out the background
        f.render_widget(block, area);

        let text = Paragraph::new(state.help_text.as_str()).wrap(Wrap { trim: true });
        let margin = Margin {
            horizontal: 2,
            vertical: 1,
        };
        f.render_widget(text, area.inner(&margin));
    }
}

fn models_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>, area: Rect) {
    let mut state = state.borrow_mut();

    let rects = Layout::default()
        .constraints([Constraint::Percentage(100)].as_ref())
        .margin(5)
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::White);
    let header_cells = state.models_table.columns.iter().map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(1);

    let rows = state.models_table.rows.iter().map(|item| {
        let height = item
            .iter()
            .map(|content| content.chars().filter(|c| *c == '\n').count())
            .max()
            .unwrap_or(0)
            + 1;
        let cells = item.iter().cloned().map(|c| Cell::from(c));
        Row::new(cells).height(height as u16).bottom_margin(0)
    });

    let selection_indicator = format!(" {}", char::from_u32(0x25B6).unwrap());
    let t = Table::new(rows)
        .header(header)
        .highlight_style(selected_style)
        .highlight_symbol(selection_indicator.as_str())
        .widths(&[
            Constraint::Percentage(50),
            Constraint::Length(30),
            Constraint::Min(10),
        ]);

    let margin = Margin {
        horizontal: 2,
        vertical: 1,
    };
    f.render_stateful_widget(t, area.inner(&margin), &mut state.models_table.state);
}
