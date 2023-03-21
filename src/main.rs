use crossterm::event::Event::Key;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::error::Error;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame, Terminal,
};
use uuid::Uuid;

const APP_KEYS_DESCRIPTION: &str = r#"
Esc:    Exit insert mode
F:      Switch to folders mode
M:      Switch to models mode
C:      In models mode; compare model
"#;

enum InputMode {
    Normal,
    Folder,
}

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
    folders: Vec<Folder>,
}

impl State {
    pub fn new() -> State {
        State {
            mode: InputMode::Normal,
            folders: vec![],
        }
    }

    pub fn add_folder(&mut self, folder: Folder) {
        self.folders.push(folder);
    }

    pub fn change_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Prepare the state
    let mut state = State::new();
    state.add_folder(Folder::new(1, String::from("First")));
    state.add_folder(Folder::new(2, String::from("Second")));

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
                    KeyCode::Char('f') => {
                        state.change_mode(InputMode::Folder);
                    }
                    _ => {}
                },

                InputMode::Folder => match key.code {
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

    let app_container_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Max(3), Constraint::Min(10), Constraint::Max(3)].as_ref())
        .split(size);

    let search_block = Block::default()
        .title("Search")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(search_block, app_container_chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(app_container_chunks[1]);

    let folders_list_section_block = Block::default()
        .title("Folders")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(folders_list_section_block, content_chunks[0]);
    folders_list_section(f, state, content_chunks[0]);

    let models_list_section_block = Block::default()
        .title("Models")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(models_list_section_block, content_chunks[1]);

    let status_block = Block::default()
        .title("Status")
        .title("Status")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(status_block, app_container_chunks[2]);

    // delete_popup(f, state);
}

fn folders_list_section<B: Backend>(f: &mut Frame<B>, state: &mut State, area: Rect) {
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
