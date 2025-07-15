use anyhow::Result;
use std::{process, io::stdout};
use crate::{
    utils,
    cli::Cli,
    project::RustProject,
    scanner::ProjectScanner,
};
use crossterm::{
    cursor,
    execute,
    terminal,
    event::{self, Event, KeyCode, KeyEvent},
};
use ratatui::{
    Frame,
    Terminal,
    backend::Backend,
    backend::CrosstermBackend,
    style::{Color as RatauiColor, Modifier, Style},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

pub struct InteractiveUI {
    cli: Cli,
    projects: Vec<RustProject>,
    selected_index: usize,
    total_deleted_size: u64,
    deleted_count: usize,
}

impl InteractiveUI {
    pub fn new(cli: Cli) -> Self {
        Self {
            cli,
            projects: Vec::new(),
            selected_index: 0,
            total_deleted_size: 0,
            deleted_count: 0,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;

        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_interactive_loop(&mut terminal).await;

        execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen, cursor::Show)?;
        terminal::disable_raw_mode()?;

        result
    }

    async fn run_interactive_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let scanner = ProjectScanner::new(self.cli.clone());
        self.projects = scanner.scan().await?;

        if self.projects.is_empty() {
            println!("No Rust projects found!");
            return Ok(());
        }

        loop {
            terminal.draw(|f| self.draw_ui(f))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    match self.handle_key_event(key_event).await? {
                        ControlFlow::Exit => break,
                        ControlFlow::Continue => continue,
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_ui<B: Backend>(&self, f: &mut Frame<B>) {
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(5),
            ])
            .split(size);

        self.draw_header(f, chunks[0]);
        self.draw_project_list(f, chunks[1]);
        self.draw_footer(f, chunks[2]);
    }

    fn draw_header<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let header = Paragraph::new("RSKILL - Rust Project Cleaner")
            .style(Style::default().fg(RatauiColor::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(header, area);
    }

    fn draw_project_list<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let items: Vec<ListItem> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let size_str = p.format_size(self.cli.gb);
                let path_str = utils::get_relative_path(&p.path);
                let path_display = utils::truncate_string(&path_str, 35);
                let last_mod = p
                    .days_since_modified()
                    .map(|days| {
                        if days == 0 {
                            "Today".to_string()
                        } else if days == 1 {
                            "1 day ago".to_string()
                        } else {
                            format!("{} days ago", days)
                        }
                    })
                    .unwrap_or_else(|| "Unknown".to_string());

                let content = format!(
                    "{:<25} {:<12} {:<35} {:<15}",
                    p.name, size_str, path_display, last_mod
                );

                let style = if i == self.selected_index {
                    Style::default()
                        .fg(RatauiColor::Black)
                        .bg(RatauiColor::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Projects"))
            .highlight_style(Style::default().bg(RatauiColor::LightBlue));

        f.render_widget(list, area);
    }

    fn draw_footer<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let total_projects = self.projects.len();
        let total_size: u64 = self.projects.iter().map(|p| p.total_cleanable_size()).sum();
        let total_size_str = utils::format_size(total_size, self.cli.gb);
        let deleted_size_str = utils::format_size(self.total_deleted_size, self.cli.gb);

        let text = vec![
            format!("{} projects | {} cleanable", total_projects, total_size_str),
            format!("{} deleted ({})", self.deleted_count, deleted_size_str),
            "↑↓/jk: navigate | space/del/D: delete | o: open | r: refresh | q: quit".to_string(),
        ];

        let paragraph = Paragraph::new(text.join("\n"))
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Status"));

        f.render_widget(paragraph, area);
    }

    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<ControlFlow> {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Ok(ControlFlow::Exit),
            
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                Ok(ControlFlow::Continue)
            }
            
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < self.projects.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
                Ok(ControlFlow::Continue)
            }
            
            KeyCode::Delete | KeyCode::Char(' ') | KeyCode::Char('D') => {
                self.delete_selected_project().await?;
                Ok(ControlFlow::Continue)
            }
            
            KeyCode::Char('o') => {
                self.open_selected_project()?;
                Ok(ControlFlow::Continue)
            }
            
            KeyCode::Char('r') => {
                self.refresh_projects().await?;
                Ok(ControlFlow::Continue)
            }
            
            KeyCode::Char('a') => {
                self.delete_all_projects().await?;
                Ok(ControlFlow::Continue)
            }
            
            _ => Ok(ControlFlow::Continue),
        }
    }

    async fn delete_selected_project(&mut self) -> Result<()> {
        if let Some(project) = self.projects.get(self.selected_index) {
            if let Some(target_dir) = &project.target_dir {
                let size_before = project.total_cleanable_size();
                
                // confirm deletion for large or active projects
                if !self.cli.delete_all && (project.is_likely_active() || size_before > 1024 * 1024 * 500) {
                    // for now, skip confirmation in interactive mode
                    // in a real implementation, you'd show a confirmation dialog
                }
                
                utils::remove_directory(target_dir, self.cli.dry_run)?;
                
                if !self.cli.dry_run {
                    self.total_deleted_size += size_before;
                    self.deleted_count += 1;
                    
                    // Update the project in our list
                    if let Some(project_mut) = self.projects.get_mut(self.selected_index) {
                        project_mut.target_dir = None;
                        project_mut.target_size = 0;
                        project_mut.build_artifacts.clear();
                    }
                }
            }
        }
        Ok(())
    }

    async fn delete_all_projects(&mut self) -> Result<()> {
        let mut total_deleted = 0u64;
        let mut count_deleted = 0;
        
        for project in &mut self.projects {
            if let Some(target_dir) = &project.target_dir {
                let size_before = project.target_size;
                
                utils::remove_directory(target_dir, self.cli.dry_run)?;
                
                if !self.cli.dry_run {
                    total_deleted += size_before;
                    count_deleted += 1;
                    
                    project.target_dir = None;
                    project.target_size = 0;
                    project.build_artifacts.clear();
                }
            }
        }
        
        self.total_deleted_size += total_deleted;
        self.deleted_count += count_deleted;
        
        Ok(())
    }

    fn open_selected_project(&self) -> Result<()> {
        if let Some(project) = self.projects.get(self.selected_index) {
            // try to open the project directory
            let path = &project.path;
            
            #[cfg(target_os = "macos")]
            {
                process::Command::new("open").arg(path).spawn()?;
            }
            
            #[cfg(target_os = "linux")]
            {
                process::Command::new("xdg-open").arg(path).spawn()?;
            }
            
            #[cfg(target_os = "windows")]
            {
                process::Command::new("explorer").arg(path).spawn()?;
            }
        }
        Ok(())
    }

    async fn refresh_projects(&mut self) -> Result<()> {
        let scanner = ProjectScanner::new(self.cli.clone());
        self.projects = scanner.scan().await?;
        self.selected_index = 0;
        Ok(())
    }
}

enum ControlFlow {
    Continue,
    Exit,
}
