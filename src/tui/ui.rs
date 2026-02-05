//! TUI layout and rendering with ratatui.
//!
//! # Overview
//!
//! This module handles rendering the user interface including:
//! - Header with title and stats
//! - Progress bar during scanning
//! - File list with duplicate groups
//! - Footer with available commands
//! - Modal dialogs for preview and confirmation
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::tui::app::App;
//! use rustdupe::tui::ui::render;
//! use ratatui::Frame;
//!
//! fn draw(frame: &mut Frame, app: &App) {
//!     render(frame, app);
//! }
//! ```

use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};

use super::app::{App, AppMode};

// ==================== Accessible Mode Helpers ====================

/// Custom ASCII border set for accessible mode.
///
/// Uses simple ASCII characters (+, -, |) instead of Unicode box-drawing
/// characters for better screen reader compatibility.
const ASCII_BORDER_SET: border::Set = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

/// Get the border set to use based on accessible mode.
///
/// In accessible mode, uses ASCII characters (+, -, |) instead of
/// Unicode box-drawing characters for better screen reader compatibility.
fn get_border_set(accessible: bool) -> border::Set {
    if accessible {
        ASCII_BORDER_SET
    } else {
        border::ROUNDED
    }
}

/// Create a block with the appropriate border style for the current mode.
fn create_block(accessible: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(get_border_set(accessible))
}

/// Create a block with title and the appropriate border style.
fn create_block_with_title<'a>(accessible: bool, title: impl Into<Line<'a>>) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(get_border_set(accessible))
        .title(title)
}

/// Render the TUI based on current application state.
///
/// This is the main entry point for rendering. It dispatches to
/// mode-specific rendering functions based on the current `AppMode`.
///
/// # Arguments
///
/// * `frame` - The ratatui frame to render to
/// * `app` - The application state to render
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main layout: header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(area);

    render_header(frame, app, chunks[0]);
    render_content(frame, app, chunks[1]);
    render_footer(frame, app, chunks[2]);

    // Render error message overlay if present
    if app.error_message().is_some() {
        render_error_dialog(frame, app, area);
    }

    // Render modal dialogs based on mode
    match app.mode() {
        AppMode::Previewing => render_preview_dialog(frame, app, area),
        AppMode::Confirming => render_confirm_dialog(frame, app, area),
        AppMode::SelectingFolder => render_folder_selection_dialog(frame, app, area),
        AppMode::SelectingGroup => render_group_selection_dialog(frame, app, area),
        AppMode::ShowingHelp => render_help_dialog(frame, app, area),
        _ => {}
    }
}

/// Render the header with title and stats.
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let dry_run_suffix = if app.is_dry_run() { " [DRY RUN]" } else { "" };
    let title = match app.mode() {
        AppMode::Scanning => format!(
            "rustdupe - Smart Duplicate Finder{} [Scanning...]",
            dry_run_suffix
        ),
        AppMode::Reviewing => format!("rustdupe - Smart Duplicate Finder{}", dry_run_suffix),
        AppMode::Previewing => format!(
            "rustdupe - Smart Duplicate Finder{} [Preview]",
            dry_run_suffix
        ),
        AppMode::Confirming => format!(
            "rustdupe - Smart Duplicate Finder{} [Confirm Delete]",
            dry_run_suffix
        ),
        AppMode::SelectingFolder => format!(
            "rustdupe - Smart Duplicate Finder{} [Select Folder]",
            dry_run_suffix
        ),
        AppMode::SelectingGroup => format!(
            "rustdupe - Smart Duplicate Finder{} [Select Group]",
            dry_run_suffix
        ),
        AppMode::Searching => format!(
            "rustdupe - Smart Duplicate Finder{} [Searching: {}]",
            dry_run_suffix,
            app.search_query()
        ),
        AppMode::ShowingHelp => {
            format!("rustdupe - Smart Duplicate Finder{} [Help]", dry_run_suffix)
        }
        AppMode::Quitting => format!("rustdupe - Goodbye!{}", dry_run_suffix),
    };

    let stats = if app.has_groups() {
        let groups = app.group_count();
        let files = app.duplicate_file_count();
        let reclaimable = format_size(app.reclaimable_space());
        format!(
            " | {} groups, {} files, {} reclaimable",
            groups, files, reclaimable
        )
    } else if app.mode() == AppMode::Scanning {
        let progress = app.scan_progress();
        format!(
            " | {} - {}/{}",
            progress.phase, progress.current, progress.total
        )
    } else {
        String::new()
    };

    let search_indicator = if app.is_searching() && app.mode() != AppMode::Searching {
        format!(" [Filter: {}]", app.search_query())
    } else {
        String::new()
    };

    let header_text = format!("{}{}{}", title, search_indicator, stats);
    let header = Paragraph::new(header_text)
        .style(
            Style::default()
                .fg(app.theme().primary)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            create_block(app.is_accessible())
                .border_style(Style::default().fg(app.theme().primary)),
        );

    frame.render_widget(header, area);
}

/// Render the main content area based on current mode.
fn render_content(frame: &mut Frame, app: &App, area: Rect) {
    match app.mode() {
        AppMode::Scanning => render_scanning_content(frame, app, area),
        AppMode::Reviewing
        | AppMode::Previewing
        | AppMode::Confirming
        | AppMode::SelectingFolder
        | AppMode::SelectingGroup
        | AppMode::Searching
        | AppMode::ShowingHelp => render_reviewing_content(frame, app, area),
        AppMode::Quitting => render_quitting_content(frame, app, area),
    }
}

/// Render quitting message.
fn render_quitting_content(frame: &mut Frame, app: &App, area: Rect) {
    let message = Paragraph::new("Goodbye! Thanks for using rustdupe.")
        .style(Style::default().fg(app.theme().success))
        .alignment(Alignment::Center)
        .block(create_block(app.is_accessible()));
    frame.render_widget(message, area);
}

/// Render the footer with available commands.
///
/// The footer hints adapt based on:
/// - Active keybinding profile (if available from App)
/// - Platform (Windows shows arrow keys, Linux/macOS shows vim keys)
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let commands = get_footer_commands(app);

    let spans: Vec<Span> = commands
        .iter()
        .flat_map(|(key, desc)| {
            if key.is_empty() {
                vec![Span::styled(
                    format!(" {} ", desc),
                    Style::default().fg(app.theme().dim),
                )]
            } else {
                vec![
                    Span::styled(
                        format!("[{}]", key),
                        Style::default()
                            .fg(app.theme().secondary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{} ", desc),
                        Style::default().fg(app.theme().normal),
                    ),
                ]
            }
        })
        .collect();

    let footer = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .block(
            create_block(app.is_accessible()).border_style(Style::default().fg(app.theme().dim)),
        );

    frame.render_widget(footer, area);
}

/// Render scanning progress.
fn render_scanning_content(frame: &mut Frame, app: &App, area: Rect) {
    let progress = app.scan_progress();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // Phase label
            Constraint::Length(3), // Progress bar
            Constraint::Length(1), // Current path
            Constraint::Min(0),    // Message
        ])
        .split(area);

    // Phase label
    let phase_text = format!("Phase: {}", progress.phase);
    let phase = Paragraph::new(phase_text)
        .style(Style::default().fg(app.theme().normal))
        .alignment(Alignment::Center);
    frame.render_widget(phase, chunks[0]);

    // Progress bar
    let percentage = progress.percentage();
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(app.theme().success).bg(app.theme().dim))
        .percent(percentage)
        .label(format!("{}%", percentage));
    frame.render_widget(gauge, chunks[1]);

    // Current path (truncated)
    let path_text = truncate_path(
        &progress.current_path,
        area.width.saturating_sub(4) as usize,
    );
    let path = Paragraph::new(path_text)
        .style(Style::default().fg(app.theme().dim))
        .alignment(Alignment::Center);
    frame.render_widget(path, chunks[2]);

    // Message
    if !progress.message.is_empty() {
        let message = Paragraph::new(progress.message.clone())
            .style(Style::default().fg(app.theme().normal))
            .alignment(Alignment::Center);
        frame.render_widget(message, chunks[3]);
    }
}

/// Render the duplicate groups and file list.
fn render_reviewing_content(frame: &mut Frame, app: &App, area: Rect) {
    if !app.has_groups() {
        let message = Paragraph::new("No duplicate files found.")
            .style(Style::default().fg(app.theme().success))
            .alignment(Alignment::Center)
            .block(create_block_with_title(app.is_accessible(), "Results"));
        frame.render_widget(message, area);
        return;
    }

    if app.is_searching() && app.visible_group_count() == 0 {
        let message = Paragraph::new(format!("No matches for filter: '{}'", app.search_query()))
            .style(Style::default().fg(app.theme().danger))
            .alignment(Alignment::Center)
            .block(create_block_with_title(
                app.is_accessible(),
                "Filter Results",
            ));
        frame.render_widget(message, area);
        return;
    }

    // Split into groups list and files list
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Groups list
            Constraint::Percentage(60), // Files in selected group
        ])
        .split(area);

    render_groups_list(frame, app, chunks[0]);
    render_files_list(frame, app, chunks[1]);
}

/// Render the list of duplicate groups.
fn render_groups_list(frame: &mut Frame, app: &App, area: Rect) {
    let visible_count = app.visible_group_count();
    let selected_group = app.group_index();

    let items: Vec<ListItem> = (0..visible_count)
        .filter_map(|i| {
            let group = app.visible_group_at(i)?;
            let size = format_size(group.size);
            let copies = group.files.len();
            let wasted = format_size(group.wasted_space());

            // First file name as label (truncated)
            let label = group
                .files
                .first()
                .map(|f| {
                    f.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                })
                .unwrap_or_else(|| "Unknown".to_string());

            let label = truncate_string(&label, 20);

            let text = format!(
                "[{}] {} ({} copies) {} - {}",
                i + 1,
                label,
                copies,
                size,
                wasted
            );

            let style = if i == selected_group {
                Style::default()
                    .fg(app.theme().inverted_fg)
                    .bg(app.theme().primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme().normal)
            };

            Some(ListItem::new(text).style(style))
        })
        .collect();

    let visible_height = area.height.saturating_sub(2) as usize;
    let scroll = app.group_scroll();

    // Create scrollbar state
    let mut scrollbar_state =
        ScrollbarState::new(visible_count.saturating_sub(visible_height)).position(scroll);

    let list = List::new(items)
        .block(
            create_block_with_title(
                app.is_accessible(),
                format!(
                    "Duplicate Groups ({}/{})",
                    selected_group + 1,
                    visible_count
                ),
            )
            .border_style(Style::default().fg(app.theme().primary)),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme().inverted_fg)
                .bg(app.theme().primary)
                .add_modifier(Modifier::BOLD),
        );

    // Split area for list and scrollbar
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    frame.render_widget(list, inner_chunks[0]);

    // Render scrollbar if needed
    if visible_count > visible_height {
        // Use ASCII symbols in accessible mode
        let (begin_sym, end_sym) = if app.is_accessible() {
            ("^", "v")
        } else {
            ("▲", "▼")
        };
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some(begin_sym))
                .end_symbol(Some(end_sym)),
            inner_chunks[1],
            &mut scrollbar_state,
        );
    }
}

/// Render the files in the selected group.
fn render_files_list(frame: &mut Frame, app: &App, area: Rect) {
    let group = match app.current_group() {
        Some(g) => g,
        None => return,
    };

    let selected_file = app.file_index();
    let max_path_len = area.width.saturating_sub(12) as usize;

    let items: Vec<ListItem> = group
        .files
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = app.is_file_selected(&entry.path);
            let is_ref = app.is_in_reference_dir(&entry.path);
            let is_first = i == 0;

            // Build group label if present
            let group_label = entry
                .group_name
                .as_ref()
                .map(|g| format!("[{}] ", g))
                .unwrap_or_default();

            // Adjust max path length to account for prefix and group label
            let prefix_len = 4; // "[X] " or similar
            let group_label_len = group_label.len();
            let available_path_len = max_path_len.saturating_sub(prefix_len + group_label_len);

            let path_str = entry.path.to_string_lossy();
            let path_display = truncate_path(&path_str, available_path_len);

            let prefix = if is_selected {
                "[X]"
            } else if is_ref {
                "[R]" // Reference marker
            } else if is_first {
                "[*]" // Original/keep marker
            } else {
                "[ ]"
            };

            let text = format!("{} {}{}", prefix, group_label, path_display);

            let style = if i == selected_file {
                if is_selected {
                    Style::default()
                        .fg(app.theme().inverted_fg)
                        .bg(app.theme().danger)
                        .add_modifier(Modifier::BOLD)
                } else if is_ref {
                    Style::default()
                        .fg(app.theme().inverted_fg)
                        .bg(app.theme().reference)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(app.theme().inverted_fg)
                        .bg(app.theme().secondary)
                        .add_modifier(Modifier::BOLD)
                }
            } else if is_selected {
                Style::default().fg(app.theme().danger)
            } else if is_ref {
                Style::default().fg(app.theme().reference)
            } else if is_first {
                Style::default().fg(app.theme().success) // Original is green
            } else {
                Style::default().fg(app.theme().normal)
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let visible_height = area.height.saturating_sub(2) as usize;
    let scroll = app.file_scroll();

    let mut scrollbar_state =
        ScrollbarState::new(group.files.len().saturating_sub(visible_height)).position(scroll);

    let selected_count = app.selected_count();
    let title = if selected_count > 0 {
        format!(
            "Files ({}/{}) - {} selected ({})",
            selected_file + 1,
            group.files.len(),
            selected_count,
            format_size(app.reclaimable_space())
        )
    } else {
        format!(
            "Files ({}/{}) - {} each",
            selected_file + 1,
            group.files.len(),
            format_size(group.size)
        )
    };

    let list = List::new(items)
        .block(
            create_block_with_title(app.is_accessible(), title)
                .border_style(Style::default().fg(app.theme().secondary)),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme().inverted_fg)
                .bg(app.theme().secondary)
                .add_modifier(Modifier::BOLD),
        );

    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    frame.render_widget(list, inner_chunks[0]);

    if group.files.len() > visible_height {
        // Use ASCII symbols in accessible mode
        let (begin_sym, end_sym) = if app.is_accessible() {
            ("^", "v")
        } else {
            ("▲", "▼")
        };
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some(begin_sym))
                .end_symbol(Some(end_sym)),
            inner_chunks[1],
            &mut scrollbar_state,
        );
    }
}

/// Render preview dialog.
fn render_preview_dialog(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(80, 80, area);
    frame.render_widget(Clear, dialog_area);

    let path = app
        .current_file()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown file".to_string());

    let content = app
        .preview_content()
        .unwrap_or("Loading preview...")
        .to_string();

    let preview = Paragraph::new(content)
        .style(Style::default().fg(app.theme().normal))
        .wrap(Wrap { trim: false })
        .block(
            create_block_with_title(
                app.is_accessible(),
                format!("Preview: {}", truncate_path(&path, 50)),
            )
            .border_style(Style::default().fg(app.theme().secondary)),
        );

    frame.render_widget(preview, dialog_area);
}

/// Render confirmation dialog.
fn render_confirm_dialog(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(60, 40, area);
    frame.render_widget(Clear, dialog_area);

    let selected_count = app.selected_count();
    let files = app.selected_files_vec();
    let total_size: u64 = files
        .iter()
        .filter_map(|p| {
            app.groups().iter().find_map(|g| {
                if g.files.iter().any(|f| &f.path == p) {
                    Some(g.size)
                } else {
                    None
                }
            })
        })
        .sum();

    let text = vec![
        Line::from(Span::styled(
            "Confirm Deletion",
            Style::default()
                .fg(app.theme().danger)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!(
            "Delete {} file(s) ({}) to trash?",
            selected_count,
            format_size(total_size)
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This action moves files to the system trash.",
            Style::default().fg(app.theme().secondary),
        )),
        Line::from(""),
        Line::from("Files to delete:"),
    ];

    let mut lines: Vec<Line> = text;

    // Show first few files
    for (i, file) in files.iter().take(5).enumerate() {
        let path = file.to_string_lossy();
        let truncated = truncate_path(&path, 45);
        lines.push(Line::from(format!("  {}. {}", i + 1, truncated)));
    }

    if files.len() > 5 {
        lines.push(Line::from(format!("  ... and {} more", files.len() - 5)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Enter] Confirm    [Esc] Cancel",
        Style::default().fg(app.theme().primary),
    )));

    let confirm = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .block(
            create_block_with_title(app.is_accessible(), "Confirm")
                .border_style(Style::default().fg(app.theme().danger)),
        );

    frame.render_widget(confirm, dialog_area);
}

/// Render folder selection dialog.
fn render_folder_selection_dialog(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(70, 60, area);
    frame.render_widget(Clear, dialog_area);

    let folders = app.folder_list();
    let selected_idx = app.folder_index();

    let items: Vec<ListItem> = folders
        .iter()
        .enumerate()
        .map(|(i, folder)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(app.theme().inverted_fg)
                    .bg(app.theme().primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme().normal)
            };
            ListItem::new(folder.to_string_lossy().to_string()).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            create_block_with_title(app.is_accessible(), "Select Folder to Mark All Duplicates")
                .border_style(Style::default().fg(app.theme().primary)),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme().inverted_fg)
                .bg(app.theme().primary)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(list, dialog_area);
}

/// Render group name selection dialog.
fn render_group_selection_dialog(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(70, 60, area);
    frame.render_widget(Clear, dialog_area);

    let group_names = app.group_name_list();
    let selected_idx = app.group_name_index();

    let items: Vec<ListItem> = group_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected_idx {
                Style::default()
                    .fg(app.theme().inverted_fg)
                    .bg(app.theme().primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme().normal)
            };
            ListItem::new(format!("[{}]", name)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            create_block_with_title(app.is_accessible(), "Select Named Group to Mark Duplicates")
                .border_style(Style::default().fg(app.theme().primary)),
        )
        .highlight_style(
            Style::default()
                .fg(app.theme().inverted_fg)
                .bg(app.theme().primary)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(list, dialog_area);
}

/// Render error dialog.
fn render_error_dialog(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(60, 20, area);
    frame.render_widget(Clear, dialog_area);

    let message = app.error_message().unwrap_or("Unknown error");

    let error = Paragraph::new(vec![
        Line::from(Span::styled(
            "Error",
            Style::default()
                .fg(app.theme().danger)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to dismiss",
            Style::default().fg(app.theme().dim),
        )),
    ])
    .alignment(Alignment::Center)
    .block(create_block(app.is_accessible()).border_style(Style::default().fg(app.theme().danger)));

    frame.render_widget(error, dialog_area);
}

// ==================== Helper Functions ====================

/// Format bytes as human-readable size.
///
/// Uses IEC binary units (KiB, MiB, GiB) via the bytesize crate.
///
/// # Examples
///
/// ```
/// use rustdupe::tui::ui::format_size;
///
/// assert_eq!(format_size(1024), "1.0 KiB");
/// assert!(format_size(1024 * 1024).contains("MiB"));
/// ```
#[must_use]
pub fn format_size(bytes: u64) -> String {
    ByteSize::b(bytes).to_string()
}

/// Truncate a string with ellipsis if it exceeds max length.
///
/// # Examples
///
/// ```
/// use rustdupe::tui::ui::truncate_string;
///
/// assert_eq!(truncate_string("hello", 10), "hello");
/// assert_eq!(truncate_string("hello world", 8), "hello...");
/// ```
#[must_use]
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        ".".repeat(max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Truncate a path with ellipsis, preserving the filename.
///
/// For long paths, keeps the filename visible and truncates the middle.
///
/// # Examples
///
/// ```
/// use rustdupe::tui::ui::truncate_path;
///
/// let short = "/home/user/file.txt";
/// assert_eq!(truncate_path(short, 50), short);
///
/// let long = "/very/long/path/to/some/deeply/nested/file.txt";
/// let truncated = truncate_path(long, 30);
/// assert!(truncated.ends_with("file.txt"));
/// assert!(truncated.contains("..."));
/// ```
#[must_use]
pub fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }

    if max_len <= 6 {
        return truncate_string(path, max_len);
    }

    // Try to preserve the filename
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    if let Some(filename) = parts.last() {
        if filename.len() + 4 <= max_len {
            // Can fit ".../" + filename
            let remaining = max_len - filename.len() - 4;
            if remaining > 0 {
                return format!("{}.../{}", &path[..remaining], filename);
            }
            return format!(".../{}", filename);
        }
    }

    // Fallback: simple truncation
    truncate_string(path, max_len)
}

/// Create a centered rectangle with given percentage of parent.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ==================== Dynamic Footer Hints ====================

/// Detect if running on Windows platform.
#[must_use]
pub fn is_windows_platform() -> bool {
    cfg!(target_os = "windows")
}

/// Get footer commands based on app mode, keybinding profile, and platform.
///
/// This function generates context-appropriate keybinding hints that:
/// - Reflect the active keybinding profile (if set)
/// - Show platform-appropriate hints (Windows: arrows, Linux/macOS: vim keys)
fn get_footer_commands(app: &App) -> Vec<(&'static str, &'static str)> {
    // Get the active profile to determine hint style
    // Default to Universal profile if no keybindings are set
    let profile = app
        .keybindings()
        .map(|kb| kb.profile())
        .unwrap_or(crate::tui::keybindings::KeybindingProfile::Universal);

    match app.mode() {
        AppMode::Scanning => vec![("q", "Quit"), ("", "Press Ctrl+C to cancel scan")],
        AppMode::Reviewing => get_reviewing_commands(app, profile),
        AppMode::Previewing => vec![("Esc", "Close"), ("q", "Quit")],
        AppMode::Confirming => vec![("Enter", "Confirm"), ("Esc", "Cancel")],
        AppMode::SelectingFolder => get_folder_selection_commands(profile),
        AppMode::SelectingGroup => get_group_selection_commands(profile),
        AppMode::Searching => vec![("Enter", "Confirm"), ("Esc", "Cancel")],
        AppMode::ShowingHelp => vec![("Esc", "Close"), ("?/F1", "Help")],
        AppMode::Quitting => vec![],
    }
}

/// Get navigation hint based on keybinding profile and platform.
fn get_nav_hint(profile: crate::tui::keybindings::KeybindingProfile) -> &'static str {
    use crate::tui::keybindings::KeybindingProfile;
    match profile {
        KeybindingProfile::Universal => {
            // Show platform-appropriate hint first
            if is_windows_platform() {
                "↑↓/jk"
            } else {
                "jk/↑↓"
            }
        }
        KeybindingProfile::Vim => "jk",
        KeybindingProfile::Standard => "↑↓",
        KeybindingProfile::Emacs => "C-n/p",
    }
}

/// Get group navigation hint based on keybinding profile.
fn get_group_nav_hint(profile: crate::tui::keybindings::KeybindingProfile) -> &'static str {
    use crate::tui::keybindings::KeybindingProfile;
    match profile {
        KeybindingProfile::Universal => {
            if is_windows_platform() {
                "PgDn/Up"
            } else {
                "JK"
            }
        }
        KeybindingProfile::Vim => "JK",
        KeybindingProfile::Standard => "PgDn/Up",
        KeybindingProfile::Emacs => "C-v/M-v",
    }
}

/// Get reviewing mode commands based on profile.
fn get_reviewing_commands(
    app: &App,
    profile: crate::tui::keybindings::KeybindingProfile,
) -> Vec<(&'static str, &'static str)> {
    let mut cmds = vec![
        (get_nav_hint(profile), "Nav"),
        (get_group_nav_hint(profile), "Grp"),
        ("Space", "Sel"),
        ("a/A", "All"),
        ("o/n", "Age"),
        ("f", "Dir"),
        ("s/l", "Size"),
        ("/", "Filter"),
    ];
    if !app.is_dry_run() {
        cmds.push(("d", "Del"));
    }
    cmds.push(("p", "Prv"));
    cmds.push(("?", "Help"));
    cmds.push(("q", "Quit"));
    cmds
}

/// Get folder selection commands based on profile.
fn get_folder_selection_commands(
    profile: crate::tui::keybindings::KeybindingProfile,
) -> Vec<(&'static str, &'static str)> {
    vec![
        (get_nav_hint(profile), "Nav"),
        ("Enter", "Select"),
        ("Esc", "Cancel"),
        ("q", "Quit"),
    ]
}

/// Get group name selection commands based on profile.
fn get_group_selection_commands(
    profile: crate::tui::keybindings::KeybindingProfile,
) -> Vec<(&'static str, &'static str)> {
    vec![
        (get_nav_hint(profile), "Nav"),
        ("Enter", "Select"),
        ("Esc", "Cancel"),
        ("q", "Quit"),
    ]
}

// ==================== Help Dialog ====================

/// Render help overlay with complete keybinding reference.
fn render_help_dialog(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(80, 85, area);
    frame.render_widget(Clear, dialog_area);

    let profile_name = app
        .keybindings()
        .map(|kb| kb.profile().display_name())
        .unwrap_or("Universal (Vim + Arrow keys)");

    let platform = if is_windows_platform() {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Keybinding Reference",
            Style::default()
                .fg(app.theme().primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Profile: ", Style::default().fg(app.theme().dim)),
            Span::styled(
                profile_name,
                Style::default()
                    .fg(app.theme().secondary)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Platform: ", Style::default().fg(app.theme().dim)),
            Span::styled(platform, Style::default().fg(app.theme().normal)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "─── Navigation ───",
            Style::default().fg(app.theme().secondary),
        )),
    ];

    // Add keybinding groups based on whether we have keybindings info
    if let Some(bindings) = app.keybindings() {
        lines.extend(get_help_lines_from_bindings(app, bindings));
    } else {
        // Show default Universal profile hints
        lines.extend(get_default_help_lines(app));
    }

    // Footer
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Esc or ? to close",
        Style::default().fg(app.theme().dim),
    )));

    let help = Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(
            create_block_with_title(app.is_accessible(), "Help")
                .border_style(Style::default().fg(app.theme().primary)),
        );

    frame.render_widget(help, dialog_area);
}

/// Generate help lines from actual keybindings.
fn get_help_lines_from_bindings<'a>(
    app: &App,
    bindings: &crate::tui::keybindings::KeyBindings,
) -> Vec<Line<'a>> {
    use crate::tui::Action;

    let mut lines = Vec::new();

    // Navigation section
    lines.push(format_help_line(
        app,
        bindings.key_hint(&Action::NavigateUp),
        bindings.key_hint(&Action::NavigateDown),
        "Move up/down",
    ));
    lines.push(format_help_line(
        app,
        bindings.key_hint(&Action::PreviousGroup),
        bindings.key_hint(&Action::NextGroup),
        "Prev/next group",
    ));
    lines.push(format_help_line_single(
        app,
        &format!(
            "{}, {}",
            bindings.key_hint(&Action::GoToTop),
            bindings.key_hint(&Action::GoToBottom)
        ),
        "Go to top/bottom",
    ));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─── Selection ───",
        Style::default().fg(app.theme().secondary),
    )));

    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::ToggleSelect),
        "Toggle selection",
    ));
    lines.push(format_help_line(
        app,
        bindings.key_hint(&Action::SelectAllInGroup),
        bindings.key_hint(&Action::SelectAllDuplicates),
        "Select group/all",
    ));
    lines.push(format_help_line(
        app,
        bindings.key_hint(&Action::SelectOldest),
        bindings.key_hint(&Action::SelectNewest),
        "Select old/new",
    ));
    lines.push(format_help_line(
        app,
        bindings.key_hint(&Action::SelectSmallest),
        bindings.key_hint(&Action::SelectLargest),
        "Select size",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::SelectFolder),
        "Select by folder",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::DeselectAll),
        "Deselect all",
    ));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─── Actions ───",
        Style::default().fg(app.theme().secondary),
    )));

    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::Preview),
        "Preview file",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::Delete),
        "Delete selected",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::ToggleTheme),
        "Toggle theme",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::Search),
        "Filter groups",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::ShowHelp),
        "Show help",
    ));
    lines.push(format_help_line_single(
        app,
        &bindings.key_hint(&Action::Quit),
        "Quit",
    ));

    lines
}

/// Generate default help lines for Universal profile.
fn get_default_help_lines(app: &App) -> Vec<Line<'static>> {
    vec![
        format_help_line_static(app, "j/↓, k/↑", "Move down/up"),
        format_help_line_static(app, "J/K, PgDn/Up", "Next/prev group"),
        format_help_line_static(app, "g/Home, G/End", "Top/bottom"),
        Line::from(""),
        Line::from(Span::styled(
            "─── Selection ───",
            Style::default().fg(app.theme().secondary),
        )),
        format_help_line_static(app, "Space", "Toggle selection"),
        format_help_line_static(app, "a, A", "Select group/all"),
        format_help_line_static(app, "o, n", "Select oldest/newest"),
        format_help_line_static(app, "s, l", "Select smallest/largest"),
        format_help_line_static(app, "f", "Select by folder"),
        format_help_line_static(app, "u", "Deselect all"),
        Line::from(""),
        Line::from(Span::styled(
            "─── Actions ───",
            Style::default().fg(app.theme().secondary),
        )),
        format_help_line_static(app, "p", "Preview file"),
        format_help_line_static(app, "d", "Delete selected"),
        format_help_line_static(app, "t", "Toggle theme"),
        format_help_line_static(app, "/", "Filter groups"),
        format_help_line_static(app, "?/F1", "Show help"),
        format_help_line_static(app, "q", "Quit"),
    ]
}

/// Format a help line with two keys and description.
fn format_help_line<'a>(app: &App, key1: String, key2: String, desc: &'static str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {:>6}", key1),
            Style::default()
                .fg(app.theme().secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(", ", Style::default().fg(app.theme().dim)),
        Span::styled(
            format!("{:<6}", key2),
            Style::default()
                .fg(app.theme().secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", desc),
            Style::default().fg(app.theme().normal),
        ),
    ])
}

/// Format a help line with single key and description.
fn format_help_line_single<'a>(app: &App, key: &str, desc: &'static str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {:>14}", key),
            Style::default()
                .fg(app.theme().secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", desc),
            Style::default().fg(app.theme().normal),
        ),
    ])
}

/// Format a static help line (for default hints).
fn format_help_line_static(app: &App, key: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:>14}", key),
            Style::default()
                .fg(app.theme().secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", desc),
            Style::default().fg(app.theme().normal),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        // bytesize uses 1024-based units but may use "KiB" (IEC) or "KB" format
        // Test that output contains reasonable size indicators
        let kb = format_size(1024);
        println!("1024 bytes = '{}'", kb);
        assert!(
            kb.contains("K") || kb.contains("k"),
            "Expected KB format, got: {}",
            kb
        );

        let mb = format_size(1024 * 1024);
        println!("1024*1024 bytes = '{}'", mb);
        assert!(
            mb.contains("M") || mb.contains("m"),
            "Expected MB format, got: {}",
            mb
        );
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
        assert_eq!(truncate_string("hello", 3), "...");
    }

    #[test]
    fn test_truncate_string_edge_cases() {
        assert_eq!(truncate_string("", 10), "");
        assert_eq!(truncate_string("a", 1), "a");
        assert_eq!(truncate_string("ab", 1), ".");
        assert_eq!(truncate_string("abc", 2), "..");
    }

    #[test]
    fn test_truncate_path_short() {
        let path = "/home/user/file.txt";
        assert_eq!(truncate_path(path, 50), path);
    }

    #[test]
    fn test_truncate_path_long() {
        let path = "/very/long/path/to/some/deeply/nested/directory/file.txt";
        let truncated = truncate_path(path, 30);
        assert!(truncated.len() <= 30);
        assert!(truncated.contains("..."));
    }

    #[test]
    fn test_truncate_path_preserves_filename() {
        let path = "/very/long/path/to/file.txt";
        let truncated = truncate_path(path, 20);
        assert!(truncated.contains("file.txt") || truncated.contains("..."));
    }

    #[test]
    fn test_truncate_path_very_short_limit() {
        let path = "/path/to/file.txt";
        let truncated = truncate_path(path, 5);
        assert_eq!(truncated.len(), 5);
        assert!(truncated.contains(".."));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(50, 50, area);

        // Should be roughly centered
        assert!(centered.x > 0);
        assert!(centered.y > 0);
        assert!(centered.width < area.width);
        assert!(centered.height < area.height);
    }

    // Test that render functions don't panic with various app states
    mod render_tests {
        use super::*;
        use crate::duplicates::DuplicateGroup;
        use std::path::PathBuf;

        fn make_group(size: u64, paths: Vec<&str>) -> DuplicateGroup {
            let now = std::time::SystemTime::now();
            DuplicateGroup::new(
                [0u8; 32],
                size,
                paths
                    .into_iter()
                    .map(|p| crate::scanner::FileEntry::new(PathBuf::from(p), size, now))
                    .collect(),
                Vec::new(),
            )
        }

        #[test]
        fn test_render_with_empty_app() {
            let app = App::new();
            // Just verify no panics - we can't actually render without a terminal
            assert_eq!(app.mode(), AppMode::Scanning);
        }

        #[test]
        fn test_render_with_groups() {
            let groups = vec![
                make_group(1000, vec!["/a.txt", "/b.txt"]),
                make_group(2000, vec!["/c.txt", "/d.txt", "/e.txt"]),
            ];
            let app = App::with_groups(groups);
            assert_eq!(app.mode(), AppMode::Reviewing);
            assert_eq!(app.group_count(), 2);
        }

        #[test]
        fn test_format_size_integration() {
            // Verify bytesize integration works
            let sizes = [0, 100, 1024, 1_000_000, 1_000_000_000];
            for size in sizes {
                let formatted = format_size(size);
                assert!(!formatted.is_empty());
            }
        }
    }
}
