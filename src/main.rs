#![allow(clippy::needless_return)]

use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use spinoff::{spinners, Spinner};
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

mod render;
use render::main::AppRenderer;

mod utils;
use utils::{
    search::{find_target_folders, Folder},
    state::list_state_listen,
};

#[derive(Debug, Clone)]
pub enum Status {
    Normal,
    Deleting,
    Error,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub folders: Vec<Folder>,
    pub list_state: ListState,
    pub status: Status,
}

impl AppState {
    fn not_deleting_folders(&self) -> Vec<&Folder> {
        return self
            .folders
            .iter()
            .filter(|folder| !folder.deleting)
            .collect();
    }
}

type AppStateArc = Arc<Mutex<AppState>>;

fn main() -> Result<()> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let spinner = Spinner::new(
        spinners::Dots,
        "recursively searching for 'target' directories",
        spinoff::Color::White,
    );

    let mut default_ignored_folders = vec![".git"];
    if cfg!(windows) {
        default_ignored_folders.extend(&["Windows", "Program Files", "Program Files (x86)"]);
    } else if cfg!(unix) {
        default_ignored_folders.extend(&["bin", "boot", "dev", "etc", "lib", "proc", "sys", "usr"]);
    }

    let all_ignored_folders: Vec<&str> = default_ignored_folders
        .into_iter()
        .collect();

    let app_state = Arc::new(Mutex::new(AppState {
        folders: find_target_folders(".", "target", &all_ignored_folders),
        list_state: {
            let mut list_state = ListState::default();
            list_state.select(Some(0));
            list_state
        },
        status: Status::Normal,
    }));

    spinner.stop();
    terminal.clear()?;
    enable_raw_mode()?;

    list_state_listen(Arc::clone(&app_state));

    loop {
        let app_state = Arc::clone(&app_state);

        if app_state.lock().unwrap().folders.is_empty() {
            terminal.clear()?;
            disable_raw_mode()?;

            println!("No 'target' left, the rskill did its job");
            return Ok(());
        }

        terminal.draw(|frame| {
            let app_state = app_state.lock().unwrap();
            let mut app_renderer = AppRenderer::new(frame, app_state);

            app_renderer.render_header();
            app_renderer.render_list();
        })?;

        thread::sleep(Duration::from_millis(12));
    }
}
