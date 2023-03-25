use crossterm::event::Event::Key;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::trace;
use std::{cell::RefCell, error::Error, fmt};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use tui_logger::*;
use uuid::Uuid;

mod text;
use log::debug;
use text::TextField;

const NORMAL_MODE_HELP: &str = r#"
<q>    Exit the program
<f>    Switch to Folder mode
<m>    Switch to Model mode

To exit this help, enter <Esc>.
"#;

const SEARCH_MODE_HELP: &str = r#"
<Esc>        Exit to Normal mode
<Backspace>  Delete the previous character
<Enter>      Execute search
"#;

const FOLDER_MODE_HELP: &str = r#"
<Esc>    Exit to Normal mode
<r>      Reload the list of folders
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

struct State {
    mode: InputMode,
    previous_mode: InputMode,
    search_field: TextField,
    folders: Vec<Folder>,
    status_line: String,
    help_text: String,
    display_help: bool,
}

impl State {
    pub fn new() -> State {
        State {
            mode: InputMode::Normal,
            previous_mode: InputMode::Normal,
            search_field: TextField::default(),
            folders: vec![],
            status_line: String::new(),
            help_text: String::default(),
            display_help: false,
        }
    }

    pub fn add_folder(&mut self, folder: Folder) {
        self.folders.push(folder);
    }

    pub fn change_mode(&mut self, mode: InputMode) {
        self.previous_mode = self.mode;
        self.status_line.clear();
        self.mode = mode;

        debug!("Change mode from {} to {}", self.previous_mode, self.mode);
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
        }
    }

    pub fn hide_help(&mut self) {
        self.display_help = false;
    }

    pub fn show_help(&self) -> bool {
        self.display_help
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    tui_logger::init_logger(log::LevelFilter::Trace).unwrap();
    tui_logger::set_default_level(log::LevelFilter::Debug);

    // Prepare the state
    let state = RefCell::new(State::new());
    state
        .borrow_mut()
        .add_folder(Folder::new(1, String::from("First")));
    state
        .borrow_mut()
        .add_folder(Folder::new(2, String::from("Second")));

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
    loop {
        terminal.draw(|f| ui(f, &state))?;

        let mut state = state.borrow_mut();
        if let Key(key) = event::read()? {
            match state.mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Char('f') => {
                        state.change_mode(InputMode::Folder);
                        state.status_line = String::from("Press <Esc> to return to Normal mode");
                    }
                    KeyCode::Char('s') => {
                        state.change_mode(InputMode::Search);
                        state.status_line = String::from("Press <Esc> to return to Normal mode");
                    }
                    KeyCode::Char('m') => state.change_mode(InputMode::Model),
                    KeyCode::Char('c') => state.change_mode(InputMode::Match),
                    KeyCode::Char('h') => {
                        state.set_help(HelpType::General);
                        state.change_mode(InputMode::Help);
                    }
                    _ => {
                        debug!("Unsupported key binding. Displaying the help message in the statusbar.");
                        state.status_line = String::from("Press <h> for help or <q> to exit");
                    }
                },
                InputMode::Search => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    KeyCode::Char(c) => {
                        state.search_field.insert_character(c);
                    }
                    KeyCode::Backspace => {
                        state.search_field.backspace();
                    }
                    KeyCode::Left => {
                        state.search_field.left();
                    }
                    KeyCode::Right => {
                        state.search_field.right();
                    }
                    KeyCode::Home => {
                        state.search_field.home();
                    }
                    KeyCode::End => {
                        state.search_field.end();
                    }
                    KeyCode::Delete => {
                        state.search_field.delete();
                    }
                    KeyCode::Enter => {
                        state.change_mode(InputMode::Normal);
                        let text = state.search_field.text();
                        state.status_line = format!("Execute search on \"{}\"", text);
                        debug!("Executing search on \"{}\"...", text);
                    }
                    _ => {}
                },
                InputMode::Folder => match key.code {
                    KeyCode::Esc => {
                        state.change_mode(InputMode::Normal);
                    }
                    KeyCode::Char('h') => {
                        state.set_help(HelpType::Folder);
                        state.change_mode(InputMode::Help);
                    }
                    KeyCode::Up => {
                        debug!("Select folder one line up");
                    }
                    KeyCode::Down => {
                        debug!("Select folder one line down");
                    }
                    KeyCode::Home => {
                        debug!("Select first folder");
                    }
                    KeyCode::End => {
                        debug!("Select last folder");
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
                        let previous_mode = state.previous_mode;
                        state.hide_help();
                        state.change_mode(previous_mode);
                    }
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
        .border_type(BorderType::Rounded);
    f.render_widget(models_list_section_block, content_chunks[1]);

    let tui_w: TuiLoggerWidget = TuiLoggerWidget::default()
        .block(
            Block::default()
                .title("Log")
                .border_style(Style::default().fg(Color::White).bg(Color::Black))
                .borders(Borders::ALL),
        )
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
    // delete_popup(f, state);
}

fn folders_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>, area: Rect) {
    let state = state.borrow();
    let list_to_show = state.folders.clone();

    let items: Vec<ListItem> = list_to_show
        .into_iter()
        .map(|item| ListItem::new(format!("{}: {}", item.id.to_owned(), item.name.to_owned())))
        .collect();

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

    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
        .split(folder_list_chunk[0]);

    let list = List::new(items)
        .highlight_symbol("->")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, list_chunks[0]);
}

fn status_section<B: Backend>(f: &mut Frame<B>, state: &RefCell<State>, area: Rect) {
    let state = state.borrow();
    let text = vec![Spans::from(vec![
        Span::styled(
            format!(" {} ", state.mode),
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
    let state = state.borrow();
    let search_block = Block::default()
        .title("Search")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(match state.mode {
            InputMode::Search => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        });
    f.render_widget(search_block.clone(), area);

    let search = Paragraph::new(state.search_field.text()).style(match state.mode {
        InputMode::Search => Style::default().fg(Color::Yellow),
        _ => Style::default(),
    });

    let margin = Margin {
        horizontal: 2,
        vertical: 1,
    };

    let edit_area = area.inner(&margin);
    f.render_widget(search, edit_area);

    match state.mode {
        InputMode::Search => {
            f.set_cursor(edit_area.x + state.search_field.index() as u16, edit_area.y);
        }
        _ => {}
    }
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
