use rayon::prelude::*;
use serde::Deserialize;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, List, ListState, Paragraph, Row, Table},
    DefaultTerminal, Frame,
};

#[derive(Debug, Deserialize)]
pub struct Changelog {
    current_version: u32,
    current_time: String,
    merge_requests: Vec<MergeRequest>,
}
#[derive(Debug, Deserialize)]
pub struct MergeRequest {
    ticket_number: String,
    title: String,
    github: String,
    flags: String,
}

fn get_changelog_info(project_id: &str, token: &str) -> Changelog {
    let output = std::process::Command::new("php")
        .arg("/home/mamazu/packages/brille24/ecom-docker/www/sulu/etc/change_log_generator.php")
        .arg("--format=json")
        .arg("--projectId=".to_owned() + project_id)
        .arg("--token=".to_owned()+token)
        .output()
        .expect("Failed to get change logs");
    if !output.status.success() {
        panic!("{}", String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let output_content = String::from_utf8_lossy(&output.stdout);
    return serde_json::from_str(&output_content).expect("JSON was not well-formatted");
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN not set");
    let mut app = App::new(
        ["251", "65"]
            .par_iter()
            .map(|version| {
                return get_changelog_info(version, &token);
            })
            .collect(),
    );
    let result = run(terminal, &mut app);
    ratatui::restore();
    result
}

#[derive(PartialEq)]
pub enum SelectedBlock {
    Left,
    Right,
}

pub struct Deployment {
    send_mail: bool,
    deployment_running: bool,
}

impl Deployment {
    pub fn new() -> Self {
        return Self {
            send_mail: false,
            deployment_running: false,
        };
    }
}

pub struct App {
    pub selected: SelectedBlock,
    pub ready_for_deployment: bool,
    pub deployment: Deployment,
    pub changelog: Vec<Changelog>,
}

impl App {
    pub fn new(changelog: Vec<Changelog>) -> Self {
        return Self {
            selected: SelectedBlock::Left,
            ready_for_deployment: false,
            deployment: Deployment::new(),
            changelog,
        };
    }

    pub fn get_current_commit_status(&self) -> &Changelog {
        match self.selected {
            SelectedBlock::Left => &self.changelog[0],
            SelectedBlock::Right => &self.changelog[1],
        }
    }
}

fn run(mut terminal: DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, app))?;
        match event::read()? {
            Event::Key(key) => {
                if key.code == KeyCode::Char('q') {
                    return Ok(());
                }
                if !app.ready_for_deployment {
                    match key.code {
                        KeyCode::Char('c') => app.ready_for_deployment = true,
                        KeyCode::Backspace => app.ready_for_deployment = false,
                        KeyCode::Left => {
                            if app.selected == SelectedBlock::Right {
                                app.selected = SelectedBlock::Left;
                            }
                        }
                        KeyCode::Right => {
                            if app.selected == SelectedBlock::Left {
                                app.selected = SelectedBlock::Right;
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Enter => {
                            app.deployment.deployment_running = true;
                        }
                        KeyCode::Char(' ') => {}
                        KeyCode::Tab => {
                            app.deployment.send_mail = !app.deployment.send_mail;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    if app.ready_for_deployment {
        render_deployment_view(frame, app);
    } else {
        render_commit_overview(frame, app);
    }
}

fn render_commit_overview(frame: &mut Frame, app: &mut App) {
    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(3), Constraint::Min(1)])
        .split(frame.area());
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(outer_layout[0]);

    let left = render_commit_view(
        "Sulu",
        &app.changelog[0],
        app.selected == SelectedBlock::Left,
    );
    let right = render_commit_view(
        "Sylius",
        &app.changelog[1],
        app.selected == SelectedBlock::Right,
    );
    let commit = render_commit_section(app);

    frame.render_widget(left, sections[0]);
    frame.render_widget(right, sections[2]);
    frame.render_widget(commit, outer_layout[1]);
}

fn render_deployment_view(frame: &mut Frame, app: &App) {
    let block = Block::bordered().title(Line::from("Deployment").centered());

    let layout = Layout::vertical([
        Constraint::Percentage(40),
        Constraint::Max(1),
        Constraint::Min(1),
    ])
    .split(block.inner(frame.area()));
    frame.render_widget(block, frame.area());

    let items = ["Send email"];
    let mut settings_state = ListState::default();
    // Clear selection
    settings_state.select(None);
    if app.deployment.send_mail {
        settings_state.select(Some(0));
    }
    let settings_list = List::new(items)
        .highlight_symbol("[x] ")
        .repeat_highlight_symbol(true);
    frame.render_stateful_widget(settings_list, layout[0], &mut settings_state);

    if app.deployment.deployment_running {
        let text = Paragraph::new("Deployment running")
            .style(Style::default().bg(Color::Yellow).fg(Color::Black))
            .centered();
        frame.render_widget(text, layout[1]);
    } else {
        let text = Paragraph::new("Start deployment")
            .style(Style::default().bg(Color::Red))
            .centered();
        frame.render_widget(text, layout[1]);
    }

    let mut send_release_mail = "Send release mail ".to_string();
    if !app.deployment.send_mail {
        send_release_mail += "[skipped]";
    }
    let items = [
        "Generate release notes",
        &send_release_mail,
        "Starting Sylius Pipeline",
        "Starting Sulu Pipeline",
    ];
    let mut state = ListState::default();
    let list = List::new(items)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::new().add_modifier(Modifier::BOLD))
        .highlight_symbol("✅ ")
        .repeat_highlight_symbol(true);
    frame.render_stateful_widget(list, layout[2], &mut state);
}

fn render_commit_view<'a>(
    title: &'static str,
    changelog: &Changelog,
    selected: bool,
) -> Paragraph<'a> {
    let text = format!(
        "Version {} ({})",
        changelog.current_version, changelog.current_time,
    )
    .to_string();
    let block = Block::bordered().title(title).style(Style::default());

    let mut style = Style::default();
    if selected {
        style = style.fg(Color::Yellow);
    }

    return Paragraph::new(Text::styled(text, style)).block(block);
}

fn render_commit_section(app: &App) -> Table {
    let block = Block::bordered()
        .title("Commit")
        .title_bottom(
            Line::from("(c) Move to deployment view")
                .style(Style::default().fg(Color::Red))
                .left_aligned(),
        )
        .style(Style::default());

    let rows = app
        .get_current_commit_status()
        .merge_requests
        .iter()
        .map(|changelog| {
            return Row::new(vec![
                changelog.ticket_number.clone(),
                changelog.title.clone(),
                changelog.github.clone(),
                changelog.flags.clone(),
            ]);
        });
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Min(20),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Ticket", "Description", "Gitlab", "Tags"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(block);

    return table;
}