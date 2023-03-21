use crossterm::event::Event::Key;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::error::Error;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
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

    pub fn change_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut state = State::new();
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
    let parent_chunk = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    let folders_list_section_block = Block::default()
        .title("Folders")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(folders_list_section_block, parent_chunk[0]);
    folders_list_section(f, state, parent_chunk[0]);

    // let list_section_block = Block::default()
    //     .title("List of passwords")
    //     .borders(Borders::ALL)
    //     .border_type(BorderType::Rounded);
    // f.render_widget(list_section_block, parent_chunk[1]);
    // list_section(f, state, parent_chunk[1]);

    // delete_popup(f, state);
}

fn folders_list_section<B: Backend>(f: &mut Frame<B>, state: &mut State, area: Rect) {
    let list_to_show = state.folders.to_owned();

    let items: Vec<ListItem> = list_to_show
        .into_iter()
        .map(|item| match state.mode {
            InputMode::Normal => {
                ListItem::new(format!("{}: {}", item.id.to_owned(), item.name.to_owned()))
            }
            _ => ListItem::new(Span::from(item.name)),
        })
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
}
