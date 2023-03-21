use crossterm::event::Event::Key;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::{error::Error, fmt};
use uuid::Uuid;

const APP_KEYS_DESCRIPTION: &str = r#"
Esc:    Exit insert mode
F:      Switch to folders mode
M:      Switch to models mode
C:      In models mode; compare model
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

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InputMode::Normal => write!(f, "Normal"),
            InputMode::Search => write!(f, "Search"),
            InputMode::Folder => write!(f, "Folder"),
            InputMode::Model => write!(f, "Model"),
            InputMode::Match => write!(f, "Match"),
            InputMode::Help => write!(f, "Help"),
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

struct State {
    mode: InputMode,
    search_text: String,
    folders: Vec<Folder>,
    status_line: String,
}

impl State {
    pub fn new() -> State {
        State {
            mode: InputMode::Normal,
            search_text: String::new(),
            folders: vec![],
            status_line: String::new(),
        }
    }

    pub fn add_folder(&mut self, folder: Folder) {
        self.folders.push(folder);
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn change_mode(&mut self, mode: InputMode) {
        self.mode = mode;
        self.set_status(String::from(format!("Mode: {}", self.mode().to_string())));
    }

    pub fn status(&self) -> String {
        self.status_line.clone()
    }

    pub fn set_status(&mut self, status: String) {
        self.status_line = status.clone();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Prepare the state
    let mut state = State::new();
    state.add_folder(Folder::new(1, String::from("First")));
    state.add_folder(Folder::new(2, String::from("Second")));
    state.set_status(String::from("Press 'h' for help"));

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut state);

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
    state: &mut State,
) -> Result<(), std::io::Error> {
    loop {
        terminal.draw(|f| ui(f, state))?;

        if let Key(key) = event::read()? {
            match state.mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Char('f') => state.change_mode(InputMode::Folder),
                    KeyCode::Char('s') => state.change_mode(InputMode::Search),
                    KeyCode::Char('m') => state.change_mode(InputMode::Model),
                    KeyCode::Char('c') => state.change_mode(InputMode::Match),
                    KeyCode::Char('h') => state.change_mode(InputMode::Help),
                    _ => {}
                },
                InputMode::Search => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    _ => {}
                },
                InputMode::Folder => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    _ => {}
                },
                InputMode::Model => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    _ => {}
                },
                InputMode::Match => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    _ => {}
                },
                InputMode::Help => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    _ => {}
                },
            }
        };
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, state: &mut State) {
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
        .constraints([Constraint::Max(3), Constraint::Min(10), Constraint::Max(1)].as_ref())
        .split(size);

    let search_block = Block::default()
        .title("Search")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(search_block, container_chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(container_chunks[1]);

    let folders_list_section_block = Block::default()
        .title("Folders")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(folders_list_section_block, content_chunks[0]);
    folders(f, state, content_chunks[0]);

    let models_list_section_block = Block::default()
        .title("Models")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(models_list_section_block, content_chunks[1]);

    let status_block = Block::default().borders(Borders::NONE);
    f.render_widget(status_block, container_chunks[2]);
    status(f, state, container_chunks[2]);

    // delete_popup(f, state);
}

fn folders<B: Backend>(f: &mut Frame<B>, state: &mut State, area: Rect) {
    let list_to_show = state.folders.to_owned();

    let items: Vec<ListItem> = list_to_show
        .into_iter()
        .map(|item| ListItem::new(format!("{}: {}", item.id.to_owned(), item.name.to_owned())))
        .collect();

    let list_chunks = Layout::default()
        .margin(2)
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
        .split(area);

    let list = List::new(items)
        .block(Block::default())
        .highlight_symbol("->")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, list_chunks[0]);
}

fn status<B: Backend>(f: &mut Frame<B>, state: &State, area: Rect) {
    let status_line = state.status();
    let status_chunk = Layout::default()
        .horizontal_margin(1)
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(area);
    let status = Paragraph::new(status_line).style(Style::default().fg(Color::Yellow));
    f.render_widget(status, status_chunk[0]);
}
