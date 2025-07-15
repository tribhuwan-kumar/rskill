use anyhow::Result;
use clap::Parser;

mod ui;
mod cli;
mod utils;
mod scanner;
mod project;

use cli::Cli;
use scanner::ProjectScanner;
use ui::InteractiveUI;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let scanner = ProjectScanner::new(cli.clone());
    
    if cli.list_only {
        let projects = scanner.scan().await?;
        scanner.print_projects(&projects).await?;
    } else {
        let mut ui = InteractiveUI::new(cli.clone());
        ui.run().await?;
    }
    
    Ok(())
}
