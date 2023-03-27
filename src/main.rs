use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dirs::home_dir;
use exitcode;
use log::{debug, warn};
use log::{error, info};
use pcli::{
    configuration::ClientConfiguration,
    model::{Folder, Model},
    service, token,
};
use std::{
    cell::{RefCell, RefMut},
    env,
    error::Error,
    fmt,
};
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

const NORMAL_MODE_HELP: &str = r#"
Normal Mode:

<q>    Exit the program
<t>    Select Physna tenant
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

const TENANT_MODE_HELP: &str = r#"
Tenant Mode:

<Esc>    Exit to Normal mode
<r>      Regenerate matches
"#;

#[derive(Debug)]
pub enum PtuiError {
    FailedToInitializeService,
    TenantNotSelected,
    ConfigurationError { cause: Option<String> },
    InputError,
    DisplayError,
}

impl fmt::Display for PtuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::FailedToInitializeService => {
                write!(f, "Failed to initialize the API service module")
            }
            Self::TenantNotSelected => write!(f, "Tenant not selected"),
            Self::ConfigurationError { cause } => {
                if cause.is_some() {
                    write!(
                        f,
                        "Configuration error occurred: {}",
                        cause.clone().unwrap().to_owned()
                    )
                } else {
                    write!(f, "Configuration error occurred")
                }
            }
            Self::InputError => write!(f, "Error occurred while receiving user input"),
            Self::DisplayError => write!(f, "Error occurred while displaying output"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum InputMode {
    Normal,
    Search,
    Folder,
    Model,
    Match,
    Help,
    Tenant,
}

#[derive(Debug, Clone, Copy)]
enum HelpType {
    General,
    Search,
    Folder,
    Model,
    Match,
    Tenant,
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
            InputMode::Tenant => write!(f, "Tenant"),
        }
    }
}

struct State<'a> {
    mode: InputMode,
    previous_mode: InputMode,
    search_field: TextArea<'a>,
    folder_list: StatefulList<Folder>,
    models_table: StatefulTable<'a, Model>,
    status_line: String,
    help_text: String,
    display_help: bool,
    display_tenants: bool,
    tenants: StatefulList<String>,
    active_tenant: Option<String>,
    active_folder: Option<String>,
    configuration: ClientConfiguration,
    api: Option<service::Api>,
}

impl<'a> State<'a> {
    pub fn new(configuration: ClientConfiguration) -> State<'static> {
        State {
            mode: InputMode::Tenant,
            previous_mode: InputMode::Tenant,
            search_field: TextArea::default(),
            folder_list: StatefulList::default(), //with_items(vec![]),
            models_table: StatefulTable::with_columns(vec!["Name", "Status", "UUID"]),
            status_line: String::new(),
            help_text: String::default(),
            display_help: false,
            display_tenants: true,
            tenants: StatefulList::default(),
            active_tenant: None,
            active_folder: None,
            configuration,
            api: None,
        }
    }

    pub fn initialize(&mut self) {
        self.configuration
            .tenants
            .keys()
            .clone()
            .for_each(|k| self.tenants.items.push(k.to_owned()));
        self.tenants.items.sort();

        self.search_field.set_cursor_line_style(Style::default());

        self.models_table.clear();
    }

    pub fn initialize_service(&mut self) -> Result<(), PtuiError> {
        if self.active_tenant.is_none() {
            return Err(PtuiError::TenantNotSelected);
        }

        let active_tenant = self.active_tenant.clone().unwrap().to_owned();

        token::invalidate_token(&active_tenant).unwrap();
        let api_configuration =
            pcli::configuration::from_client_configuration(&self.configuration, &active_tenant);

        let api: Option<service::Api> = match api_configuration {
            Ok(api_configuration) => Some(service::Api::new(
                api_configuration.base_url,
                active_tenant.clone(),
                api_configuration.access_token,
            )),
            Err(e) => {
                return Err(PtuiError::ConfigurationError {
                    cause: match e.source() {
                        Some(e) => Some(e.to_string()),
                        None => None,
                    },
                });
            }
        };

        info!(
            "Started a new session with Physna for tenant {}",
            active_tenant.clone()
        );
        self.api = api;
        Ok(())
    }

    pub fn clear_folders(&mut self) {
        self.folder_list.items.clear();
    }

    pub fn add_folder(&mut self, folder: Folder) {
        self.folder_list.items.push(folder.clone());
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
            InputMode::Tenant => {
                info!("Please select your tenant from the list");
                self.status_line = String::from(
                    "Select and press <Enter> to specify a tenant, or press <Esc> to cancel",
                )
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
            HelpType::Tenant => {
                self.help_text = String::from(TENANT_MODE_HELP);
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
}

impl<T> Default for StatefulList<T> {
    fn default() -> StatefulList<T> {
        Self::with_items(vec![])
    }
}

struct StatefulTable<'a, T> {
    state: TableState,
    columns: Vec<&'a str>,
    rows: Vec<T>,
}

impl<'a, T> StatefulTable<'a, T> {
    fn with_columns(columns: Vec<&'a str>) -> StatefulTable<'a, T> {
        StatefulTable {
            state: TableState::default(),
            columns,
            rows: vec![],
        }
    }

    fn add_row(&mut self, row: T) {
        self.rows.push(row);
    }

    fn clear(&mut self) {
        self.rows.clear();
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

    let home_directory = home_dir();
    let home_directory = match home_directory {
        Some(dir) => dir,
        None => {
            eprintln!("Error: Failed to determine the home directory");
            ::std::process::exit(exitcode::DATAERR);
        }
    };
    let home_directory = String::from(home_directory.to_str().unwrap());
    let mut default_configuration_file_path = home_directory;
    default_configuration_file_path.push_str("/.pcli.conf");

    let configuration =
        pcli::configuration::initialize(&String::from(default_configuration_file_path));
    let configuration = match configuration {
        Ok(configuration) => configuration,
        Err(e) => {
            eprintln!(
                "Cannot initialize process with the provided configuration: {}",
                e
            );
            ::std::process::exit(exitcode::CONFIG);
        }
    };

    tui_logger::init_logger(level_filter).unwrap();
    tui_logger::set_default_level(level_filter);

    // Prepare the state
    let state = RefCell::new(State::new(configuration));

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, state: RefCell<State>) -> Result<(), PtuiError> {
    let mut state = state.borrow_mut();
    state.initialize();

    loop {
        match terminal.draw(|f| ui(f, &mut state)) {
            Ok(_frame) => {}
            Err(_) => return Err(PtuiError::DisplayError),
        }

        let event = match event::read() {
            Ok(event) => event,
            Err(_) => return Err(PtuiError::InputError),
        };

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
                    KeyEvent {
                        code: KeyCode::Char('t'),
                        ..
                    } => {
                        state.display_tenants = true;
                        state.change_mode(InputMode::Tenant);
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
                    _ => {
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
                            Some(index) => match state.folder_list.items.get(index) {
                                Some(folder) => {
                                    let name = folder.name.to_owned();
                                    let id = folder.id;
                                    state.active_folder = Some(name.clone());
                                    debug!("Selected folder [{}] \"{}\"", id, name.clone());

                                    match &state.api {
                                        Some(api) => {
                                            debug!(
                                                "Reading the list of models for folder {}...",
                                                id
                                            );

                                            let mut folders: Vec<u32> = Vec::new();
                                            folders.push(id);

                                            let models = api.list_all_models(folders, None, false);
                                            match models {
                                                Ok(models) => {
                                                    debug!(
                                                        "Found {} model(s)",
                                                        models.models.len()
                                                    );

                                                    state.models_table.clear();
                                                    models.models.iter().cloned().for_each(
                                                        |model| {
                                                            state.models_table.add_row(model);
                                                        },
                                                    );
                                                }
                                                Err(e) => error!("Error reading models: {}", e),
                                            }
                                        }
                                        None => {
                                            state.active_folder = None;
                                            state.models_table.clear();
                                        }
                                    }
                                }
                                None => {
                                    state.active_folder = None;
                                    state.models_table.clear();
                                }
                            },
                            None => {
                                state.active_folder = None;
                                state.models_table.clear();
                                warn!("No folder selected");
                            }
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
                                let selected_row = state.models_table.rows.get(index).ok_or(Err::<
                                    String,
                                    std::io::Error,
                                >(
                                    std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        "Incompatible model row item",
                                    ),
                                ));
                                debug!("Selected model \"{}\"", selected_row.unwrap().uuid);
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
            InputMode::Tenant => match event {
                Event::Key(key) => match key {
                    KeyEvent {
                        code: KeyCode::Esc, ..
                    } => {
                        state.display_tenants = false;
                        state.change_mode(InputMode::Normal)
                    }
                    KeyEvent {
                        code: KeyCode::Char('h'),
                        ..
                    } => {
                        state.set_help(HelpType::Tenant);
                        state.change_mode(InputMode::Help);
                    }
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        state.tenants.previous();
                    }
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        state.tenants.next();
                    }
                    KeyEvent {
                        code: KeyCode::Home,
                        ..
                    } => {
                        state.tenants.first();
                    }
                    KeyEvent {
                        code: KeyCode::End, ..
                    } => {
                        state.tenants.last();
                    }
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        let selected = state.tenants.state.selected();
                        match selected {
                            Some(index) => {
                                let selected_item = state.tenants.items.get(index).ok_or(Err::<
                                    String,
                                    std::io::Error,
                                >(
                                    std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        "Incompatible tenant list item",
                                    ),
                                ));

                                let active_tenant = selected_item.unwrap().to_owned();
                                state.active_tenant = Some(active_tenant.clone());
                                info!("Selected tenant \"{}\"", active_tenant.clone());

                                match state.initialize_service() {
                                    Ok(()) => {
                                        debug!("Connected to the Physna service");
                                    }
                                    Err(e) => {
                                        error!("Unable to connect to Physna, because of: {}", e)
                                    }
                                }

                                // reloading the list of folders
                                match &state.api {
                                    Some(api) => {
                                        let folders = api.get_list_of_folders();
                                        match folders {
                                            Ok(mut folders) => {
                                                state.clear_folders();
                                                folders.folders.sort();
                                                folders.folders.iter().for_each(|f| {
                                                    state.add_folder(f.clone());
                                                });
                                                debug!("List of folders ready");
                                            }
                                            Err(e) => {
                                                error!("Failed to read the list of fodlers: {}", e);
                                            }
                                        }
                                    }
                                    None => {
                                        warn!("No connection with Physna");
                                    }
                                }

                                state.display_tenants = false;
                                state.change_mode(InputMode::Normal);
                            }
                            None => {
                                state.active_tenant = None;
                                warn!("No tenant selected");
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, state: &mut RefMut<State>) {
    let size = f.size();

    let active_tenant = state
        .active_tenant
        .as_ref()
        .unwrap_or(&String::from("None"))
        .to_owned();

    // Main container
    let app_container = Block::default()
        .title(Spans::from(vec![
            Span::styled(
                "Physna TUI (",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                active_tenant,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                ")",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
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
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(container_chunks[1]);

    folders_section(f, state, content_chunks[0]);

    let active_folder = match state.active_folder.clone() {
        Some(name) => name.clone(),
        None => String::from(""),
    };

    let title = format!(
        "Models ({}:{})",
        active_folder.clone(),
        state.models_table.rows.len()
    );

    let models_list_section_block = Block::default()
        .title(title.as_str())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(match state.mode {
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

    tenant_selection_section(f, state);

    help_section(f, state);
}

fn folders_section<B: Backend>(f: &mut Frame<B>, state: &RefMut<State>, area: Rect) {
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
        .map(|i| ListItem::new(format!("[{: >4}] {}", i.id, i.name)))
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

    let mut folder_list_state = state.folder_list.state.clone();
    f.render_stateful_widget(folder_list, folder_list_chunk[0], &mut folder_list_state);
}

fn status_section<B: Backend>(f: &mut Frame<B>, state: &RefMut<State>, area: Rect) {
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

fn search_section<B: Backend>(f: &mut Frame<B>, state: &mut RefMut<State>, area: Rect) {
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

fn help_section<B: Backend>(f: &mut Frame<B>, state: &RefMut<State>) {
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

fn tenant_selection_section<B: Backend>(f: &mut Frame<B>, state: &mut RefMut<State>) {
    if state.display_tenants {
        let block = Block::default().title("Tenant").borders(Borders::ALL);
        let area = centered_rect(30, 50, f.size());
        f.render_widget(Clear, area); //this clears out the background
        f.render_widget(block, area);

        let margin = Margin {
            horizontal: 2,
            vertical: 2,
        };

        let tenant_list_section_block = Block::default()
            .title("Folders")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(match state.mode {
                InputMode::Tenant => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            });
        f.render_widget(tenant_list_section_block, area);

        // transform a vector of Strings to a vector of ListItems
        let visible_items: Vec<ListItem> = state
            .tenants
            .items
            .iter()
            .cloned()
            .map(|i| ListItem::new(i))
            .collect();

        let selection_indicator = format!(" {}", char::from_u32(0x25B6).unwrap());
        let tenants_list = List::new(visible_items)
            .highlight_style(
                Style::default().add_modifier(Modifier::REVERSED),
                // .fg(Color::Black)
                // .bg(Color::LightBlue)
                // .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(selection_indicator.as_str());

        f.render_stateful_widget(tenants_list, area.inner(&margin), &mut state.tenants.state);
    }
}

fn models_section<B: Backend>(f: &mut Frame<B>, state: &mut RefMut<State>, area: Rect) {
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

    let rows = state.models_table.rows.iter().map(|model| {
        let mut cells: Vec<Cell> = Vec::new();
        cells.push(Cell::from(model.name.clone()));
        cells.push(Cell::from(model.state.clone()));
        cells.push(Cell::from(model.uuid.to_string()));
        Row::new(cells).height(1).bottom_margin(0)
    });

    let selection_indicator = format!(" {}", char::from_u32(0x25B6).unwrap());
    let t = Table::new(rows)
        .header(header)
        .highlight_style(selected_style)
        .highlight_symbol(selection_indicator.as_str())
        .widths(&[
            Constraint::Length(50),
            Constraint::Length(10),
            Constraint::Length(36),
        ]);

    let margin = Margin {
        horizontal: 2,
        vertical: 1,
    };
    f.render_stateful_widget(t, area.inner(&margin), &mut state.models_table.state);
}
