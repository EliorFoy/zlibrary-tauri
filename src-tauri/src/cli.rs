use std::io;
use std::sync::Arc;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use zlibrary_core::account_pool::AccountPool;
use zlibrary_core::download::ProgressCallback;
use zlibrary_core::mail_receiver::MinMailReceiver;

#[cfg(windows)]
fn enable_utf8_console() {
    unsafe {
        extern "system" {
            fn SetConsoleCP(codepage: u32) -> i32;
            fn SetConsoleOutputCP(codepage: u32) -> i32;
        }
        SetConsoleCP(65001);
        SetConsoleOutputCP(65001);
    }
}

#[cfg(not(windows))]
fn enable_utf8_console() {}
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use zlibrary_core::model::BookInfo;
use zlibrary_core::search;

enum AppState {
    MainMenu,
    SearchInput,
    Searching,
    ShowingResults,
    Downloading,
    Done,
    AccountsList,
    AccountAddEmail,
    AccountAddCode,
    AccountAddVerifyCode,
    AccountRegister,
    AccountLoginEmail,
    AccountLoginPassword,
}

struct App {
    state: AppState,
    input: String,
    cursor_pos: usize,
    results: Vec<BookInfo>,
    list_state: ListState,
    status_msg: String,
    error_msg: String,
    selected_book: Option<BookInfo>,
    download_progress: String,
    download_pct: u64,
    download_size: String,
    current_page: u32,
    accounts: Vec<AccountDisplay>,
    accounts_list_state: ListState,
    account_email: String,
    account_password: String,
    account_name: String,
    account_code: String,
    account_pool: AccountPool,
}

struct AccountDisplay {
    id: i64,
    email: String,
    user_id: i64,
    usage: i32,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let mut accounts_list_state = ListState::default();
        accounts_list_state.select(Some(0));

        let account_pool = AccountPool::new().expect("打开账号数据库");

        // Load accounts
        let accounts = account_pool
            .list_accounts()
            .unwrap_or_default()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                user_id: a.user_id,
                usage: a.usage_count,
            })
            .collect();

        Self {
            state: AppState::MainMenu,
            input: String::new(),
            cursor_pos: 0,
            results: Vec::new(),
            list_state,
            status_msg: String::from("=== Z-Library NoProxy === 请选择操作"),
            error_msg: String::new(),
            selected_book: None,
            download_progress: String::new(),
            download_pct: 0,
            download_size: String::new(),
            current_page: 1,
            accounts,
            accounts_list_state,
            account_email: String::new(),
            account_password: String::new(),
            account_name: String::new(),
            account_code: String::new(),
            account_pool,
        }
    }

    fn run_search(&mut self) {
        if self.input.trim().is_empty() {
            return;
        }
        self.state = AppState::Searching;
        self.status_msg = format!("正在搜索 \"{}\" ...", self.input.trim());
        self.error_msg.clear();
        self.results.clear();
    }

    fn next_page(&mut self) {
        self.current_page += 1;
        self.run_search();
    }

    fn prev_page(&mut self) {
        if self.current_page > 1 {
            self.current_page -= 1;
            self.run_search();
        }
    }

    fn select_book(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.results.len() {
                self.selected_book = Some(self.results[idx].clone());
                self.state = AppState::Downloading;
                self.download_progress = String::from("准备下载…");
            }
        }
    }

    fn refresh_accounts(&mut self) {
        self.accounts = self
            .account_pool
            .list_accounts()
            .unwrap_or_default()
            .into_iter()
            .map(|a| AccountDisplay {
                id: a.id,
                email: a.email,
                user_id: a.user_id,
                usage: a.usage_count,
            })
            .collect();
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    zlibrary_core::client::init_resolver().await;
    zlibrary_core::logger::init();
    enable_utf8_console();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("错误: {err:?}");
    }

    Ok(())
}

async fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.state {
                AppState::MainMenu => match key.code {
                    KeyCode::Esc => return Ok(()),
                    KeyCode::Char('1') | KeyCode::Char('s') => {
                        app.state = AppState::SearchInput;
                        app.status_msg = String::from("输入搜索关键词, 按 Enter 搜索, Esc 返回");
                    }
                    KeyCode::Char('2') | KeyCode::Char('a') => {
                        app.refresh_accounts();
                        app.state = AppState::AccountsList;
                        app.status_msg = String::from("账号管理 | a 手动注册 | r 自动注册 | l 登录已有账号 | d 删除 | Esc 返回");
                    }
                    _ => {}
                },
                AppState::SearchInput => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::MainMenu;
                        app.status_msg =
                            String::from("=== Z-Library NoProxy === 请选择操作");
                    }
                    KeyCode::Enter => {
                        app.current_page = 1;
                        app.run_search();
                    }
                    KeyCode::Char(c) => {
                        let byte_pos = app
                            .input
                            .char_indices()
                            .nth(app.cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(app.input.len());
                        app.input.insert(byte_pos, c);
                        app.cursor_pos += 1;
                    }
                    KeyCode::Backspace => {
                        if app.cursor_pos > 0 {
                            app.cursor_pos -= 1;
                            let byte_pos = app
                                .input
                                .char_indices()
                                .nth(app.cursor_pos)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            app.input.remove(byte_pos);
                        }
                    }
                    KeyCode::Left => {
                        if app.cursor_pos > 0 {
                            app.cursor_pos -= 1;
                        }
                    }
                    KeyCode::Right => {
                        let char_count = app.input.chars().count();
                        if app.cursor_pos < char_count {
                            app.cursor_pos += 1;
                        }
                    }
                    _ => {}
                },
                AppState::Searching => {}
                AppState::ShowingResults => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::SearchInput;
                        app.download_pct = 0;
                        app.download_progress.clear();
                        app.download_size.clear();
                        app.status_msg =
                            String::from("输入搜索关键词, 按 Enter 搜索, Esc 返回");
                    }
                    KeyCode::Up => {
                        let i = app.list_state.selected().unwrap_or(0);
                        if i > 0 {
                            app.list_state.select(Some(i - 1));
                        }
                    }
                    KeyCode::Down => {
                        let i = app.list_state.selected().unwrap_or(0);
                        if i + 1 < app.results.len() {
                            app.list_state.select(Some(i + 1));
                        }
                    }
                    KeyCode::Enter => app.select_book(),
                    KeyCode::Char('n') | KeyCode::Right => app.next_page(),
                    KeyCode::Char('p') | KeyCode::Left => app.prev_page(),
                    _ => {}
                },
                AppState::Downloading => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::ShowingResults;
                        app.download_pct = 0;
                        app.download_progress.clear();
                        app.download_size.clear();
                    }
                    _ => {}
                },
                AppState::Done => match key.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                        app.state = AppState::ShowingResults;
                        app.download_pct = 0;
                        app.download_progress.clear();
                        app.download_size.clear();
                    }
                    _ => {}
                },
                AppState::AccountsList => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::MainMenu;
                        app.status_msg =
                            String::from("=== Z-Library NoProxy === 请选择操作");
                    }
                    KeyCode::Char('a') => {
                        app.state = AppState::AccountAddEmail;
                        app.account_email.clear();
                        app.account_password.clear();
                        app.account_name.clear();
                        app.account_code.clear();
                        app.status_msg =
                            String::from("手动注册: 输入邮箱地址");
                    }
                    KeyCode::Char('r') => {
                        app.state = AppState::AccountRegister;
                        app.status_msg =
                            String::from("自动注册中，请等待...");
                    }
                    KeyCode::Char('l') => {
                        app.state = AppState::AccountLoginEmail;
                        app.account_email.clear();
                        app.account_password.clear();
                        app.status_msg =
                            String::from("登录已有账号: 输入邮箱地址");
                    }
                    KeyCode::Char('d') => {
                        if let Some(idx) = app.accounts_list_state.selected() {
                            if idx < app.accounts.len() {
                                let id = app.accounts[idx].id;
                                match app.account_pool.delete_account(id) {
                                    Ok(_) => {
                                        app.refresh_accounts();
                                        app.status_msg =
                                            String::from("账号已删除");
                                    }
                                    Err(e) => {
                                        app.error_msg = e;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                AppState::AccountAddEmail => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::AccountsList;
                        app.status_msg = String::from("账号管理");
                    }
                    KeyCode::Enter => {
                        if !app.account_email.is_empty() {
                            app.state = AppState::AccountAddCode;
                            app.status_msg = String::from("输入密码:");
                        }
                    }
                    KeyCode::Char(c) => {
                        app.account_email.push(c);
                    }
                    KeyCode::Backspace => {
                        app.account_email.pop();
                    }
                    _ => {}
                },
                AppState::AccountAddCode => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::AccountsList;
                        app.status_msg = String::from("账号管理");
                    }
                    KeyCode::Enter => {
                        if !app.account_password.is_empty() {
                            if app.account_name.is_empty() {
                                app.account_name = format!(
                                    "User_{}",
                                    &rand::random::<u32>().to_string()[..6]
                                );
                            }
                            app.state = AppState::AccountAddVerifyCode;
                            app.status_msg = String::from("输入验证码:");
                        }
                    }
                    KeyCode::Char(c) => {
                        if app.account_password.is_empty() {
                            app.account_password.push(c);
                        } else if app.account_name.is_empty() {
                            app.account_name.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        if !app.account_name.is_empty() {
                            app.account_name.pop();
                        } else if !app.account_password.is_empty() {
                            app.account_password.pop();
                        }
                    }
                    _ => {}
                },
                AppState::AccountAddVerifyCode => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::AccountsList;
                        app.status_msg = String::from("账号管理");
                    }
                    KeyCode::Enter => {
                        if !app.account_code.is_empty() {
                            app.status_msg = String::from("正在提交...");
                            let email = app.account_email.clone();
                            let password = app.account_password.clone();
                            let name = app.account_name.clone();
                            let code = app.account_code.clone();
                            match app.account_pool.manual_register(&email, &password, &name, &code).await {
                                Ok(()) => {
                                    app.status_msg = format!("✅ {email} 注册成功");
                                    app.refresh_accounts();
                                }
                                Err(e) => {
                                    app.error_msg = e;
                                    app.status_msg = String::from("❌ 注册失败");
                                }
                            }
                            app.state = AppState::AccountsList;
                        }
                    }
                    KeyCode::Char(c) => {
                        app.account_code.push(c);
                    }
                    KeyCode::Backspace => {
                        app.account_code.pop();
                    }
                    _ => {}
                },
                AppState::AccountRegister => {}
                AppState::AccountLoginEmail => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::AccountsList;
                        app.status_msg = String::from("账号管理");
                    }
                    KeyCode::Enter => {
                        if !app.account_email.is_empty() {
                            app.state = AppState::AccountLoginPassword;
                            app.status_msg = String::from("输入密码:");
                        }
                    }
                    KeyCode::Char(c) => {
                        app.account_email.push(c);
                    }
                    KeyCode::Backspace => {
                        app.account_email.pop();
                    }
                    _ => {}
                },
                AppState::AccountLoginPassword => match key.code {
                    KeyCode::Esc => {
                        app.state = AppState::AccountsList;
                        app.status_msg = String::from("账号管理");
                    }
                    KeyCode::Enter => {
                        if !app.account_password.is_empty() {
                            app.status_msg = String::from("正在登录...");
                            let email = app.account_email.clone();
                            let password = app.account_password.clone();
                            match app.account_pool.manual_login(&email, &password).await {
                                Ok(()) => {
                                    app.status_msg = format!("✅ {email} 登录成功");
                                    app.refresh_accounts();
                                }
                                Err(e) => {
                                    app.error_msg = e;
                                    app.status_msg = String::from("❌ 登录失败");
                                }
                            }
                            app.state = AppState::AccountsList;
                        }
                    }
                    KeyCode::Char(c) => {
                        app.account_password.push(c);
                    }
                    KeyCode::Backspace => {
                        app.account_password.pop();
                    }
                    _ => {}
                },
            }
        }

        if matches!(app.state, AppState::Searching) {
        let query = app.input.trim().to_string();
        let page = app.current_page;

        let start = std::time::Instant::now();
        let handle = tokio::spawn(async move {
            search::search_books(&query, page).await
        });

        while !handle.is_finished() {
            let elapsed = start.elapsed().as_secs();
            if elapsed < 3 {
                app.status_msg = format!("正在搜索 \"{}\" ...", app.input.trim());
            } else if elapsed < 8 {
                app.status_msg = format!("正在JS验证中... ({}s)", elapsed);
            } else {
                app.status_msg = format!("加载中，请耐心等待... ({}s)", elapsed);
            }
            terminal.draw(|f| ui(f, app))?;
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        match handle.await.unwrap() {
            Ok(result) => {
                app.results = result.books;
                app.list_state
                    .select(if app.results.is_empty() { None } else { Some(0) });
                app.state = AppState::ShowingResults;
                app.status_msg = format!(
                    "搜索 \"{}\" 找到 {} 个结果 | ↑↓ 选择 | Enter 下载 | n/p 翻页 | Esc 返回",
                    app.input.trim(),
                    result.total
                );
            }
            Err(e) => {
                app.error_msg = e;
                app.state = AppState::SearchInput;
            }
        }
    }

    if matches!(app.state, AppState::Downloading) {
        let book = match app.selected_book.clone() {
            Some(b) => b,
            None => continue,
        };
        app.download_progress = format!("正在下载: {}…", book.title);
        app.download_size = String::new();
        app.download_pct = 0u64;
        terminal.draw(|f| ui(f, app))?;
        struct TuiProgress {
            msg: Arc<std::sync::Mutex<String>>,
            pct: Arc<std::sync::atomic::AtomicU64>,
            size: Arc<std::sync::Mutex<String>>,
        }
        impl ProgressCallback for TuiProgress {
            fn on_start(&self, total_bytes: u64) {
                *self.size.lock().unwrap() = format_size(total_bytes);
            }
            fn on_progress(&self, downloaded: u64, total: u64) {
                if total > 0 {
                    let p = downloaded * 100 / total;
                    self.pct.store(p, std::sync::atomic::Ordering::SeqCst);
                    *self.msg.lock().unwrap() = format!(
                        "{:.1}% {}/{}",
                        p as f64,
                        format_size(downloaded),
                        format_size(total)
                    );
                }
            }
            fn on_finish(&self) {
                self.pct.store(100, std::sync::atomic::Ordering::SeqCst);
                *self.msg.lock().unwrap() = "下载完成!".to_string();
            }
        }
        let tp = Arc::new(TuiProgress {
            msg: Arc::new(std::sync::Mutex::new(String::new())),
            pct: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            size: Arc::new(std::sync::Mutex::new(String::new())),
        });
        let tp_clone = tp.clone();
        let handle = tokio::spawn(async move {
            zlibrary_core::download::download_book_with_progress(&book, tp_clone, None).await
        });
        while !handle.is_finished() {
            let pct = tp.pct.load(std::sync::atomic::Ordering::SeqCst);
            let msg = tp.msg.lock().unwrap().clone();
            let size = tp.size.lock().unwrap().clone();
            app.download_pct = pct;
            app.download_progress = msg;
            app.download_size = size;
            terminal.draw(|f| ui(f, app))?;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        match handle.await.unwrap() {
            Ok(path) => {
                app.download_progress = format!("下载完成! 文件: {}", path.display());
                app.download_pct = 100;
                app.state = AppState::Done;
            }
            Err(e) => {
                app.download_progress = format!("下载失败: {}", e);
                app.state = AppState::Done;
            }
        }
    }

    if matches!(app.state, AppState::AccountRegister) {
        app.status_msg = String::from("预热会话...");
        terminal.draw(|f| ui(f, app))?;

        if let Err(e) = zlibrary_core::client::warmup_session().await {
            app.error_msg = format!("会话预热失败: {e}");
            app.state = AppState::AccountsList;
            continue;
        }

        app.status_msg = String::from("自动注册中（获取邮箱 → 发送验证码 → 等待邮件 → 验证）…");
        terminal.draw(|f| ui(f, app))?;

        let receiver = MinMailReceiver::new("eliorfoy");
        let result = app.account_pool.auto_register(1, &receiver).await;
        app.refresh_accounts();
        app.state = AppState::AccountsList;
        app.status_msg = format!(
            "自动注册完成: 成功 {}, 失败 {}",
            result.success, result.fail
        );
    }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    let show_progress = matches!(app.state, AppState::Downloading | AppState::Done)
        || app.download_pct > 0;

    let bottom_height: u16 = if show_progress { 3 } else { 1 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(bottom_height),
        ])
        .split(area);

    let title = Paragraph::new(
        Text::from(vec![
            Line::from(Span::styled(
                "📚 Z-Library NoProxy",
                Style::default()
                    .fg(Color::Rgb(233, 69, 96))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "IP直连 · SNI伪装 · 0代理",
                Style::default().fg(Color::Gray),
            )),
        ]),
    )
    .block(Block::default().borders(Borders::ALL))
    .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    let status_style = if app.error_msg.is_empty() {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(Color::Red)
    };
    let status_text = if !app.error_msg.is_empty() {
        app.error_msg.as_str()
    } else {
        &app.status_msg
    };
    let status =
        Paragraph::new(Span::styled(status_text, status_style)).alignment(Alignment::Center);
    f.render_widget(status, chunks[1]);

    match app.state {
        AppState::MainMenu => render_main_menu(f, app, chunks[2]),
        AppState::SearchInput | AppState::Searching => render_search_input(f, app, chunks[2]),
        AppState::ShowingResults => render_results(f, app, chunks[2]),
        AppState::Downloading | AppState::Done => render_download(f, app, chunks[2]),
        AppState::AccountsList => render_accounts(f, app, chunks[2]),
        AppState::AccountAddEmail => render_account_add_email(f, app, chunks[2]),
        AppState::AccountAddCode => render_account_add_code(f, app, chunks[2]),
        AppState::AccountAddVerifyCode => render_account_add_verify_code(f, app, chunks[2]),
        AppState::AccountLoginEmail => render_account_login_email(f, app, chunks[2]),
        AppState::AccountLoginPassword => render_account_login_password(f, app, chunks[2]),
        AppState::AccountRegister => render_account_register(f, app, chunks[2]),
    }

    if show_progress {
        render_progress_bar(f, app, chunks[3]);
    } else {
        let footer = Paragraph::new(Span::styled(
            "基于 GPL-3.0 | 仅供学习研究使用",
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center);
        f.render_widget(footer, chunks[3]);
    }
}

fn render_main_menu(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let items = vec![
        ListItem::new(Line::from(Span::styled(
            " 1. 搜索并下载书籍",
            Style::default().fg(Color::White),
        ))),
        ListItem::new(Line::from(Span::styled(
            " 2. 账号池管理",
            Style::default().fg(Color::White),
        ))),
    ];

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(124, 77, 255)))
                .title("主菜单"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(124, 77, 255)),
        );

    f.render_widget(list, inner[0]);

    let help = Paragraph::new(vec![
        Line::from(""),
        Line::from(format!(
            "账号池: {} 个可用账号",
            app.accounts.len()
        )),
        Line::from(""),
        Line::from("按 1/s 搜索 | 按 2/a 账号管理 | Esc 退出"),
    ])
    .style(Style::default().fg(Color::Gray));
    f.render_widget(help, inner[1]);
}

fn render_accounts(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.accounts.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  暂无账号",
            Style::default().fg(Color::Gray),
        )))]
    } else {
        app.accounts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let is_sel = app.accounts_list_state.selected() == Some(i);
                let base = if is_sel {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(124, 77, 255))
                } else {
                    Style::default()
                };
                let line = format!(
                    "  {:>3}. {:<30} | uid={} | 剩余次数: {}",
                    a.id, a.email, a.user_id, a.usage
                );
                ListItem::new(Line::from(Span::styled(line, base)))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(124, 77, 255)))
                .title("账号列表"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(124, 77, 255))
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.accounts_list_state.clone());
}

fn render_account_add_email(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let input_display = if app.account_email.is_empty() {
        Span::styled("输入邮箱地址", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&app.account_email, Style::default().fg(Color::White))
    };
    let input_block = Paragraph::new(Line::from(input_display))
        .block(Block::default().borders(Borders::ALL).title("邮箱"));
    f.render_widget(input_block, inner[0]);

    let help = Paragraph::new("输入邮箱后按 Enter 继续，Esc 返回")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help, inner[1]);
}

fn render_account_add_code(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .margin(2)
        .split(area);

    let pw_mask = "*".repeat(app.account_password.len());
    let pw_display = if app.account_password.is_empty() {
        Span::styled("输入密码", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&pw_mask, Style::default().fg(Color::White))
    };
    let pw_block =
        Paragraph::new(Line::from(pw_display)).block(Block::default().borders(Borders::ALL).title("密码"));
    f.render_widget(pw_block, inner[0]);

    let name_display = if app.account_name.is_empty() {
        Span::styled("输入用户名（可选）", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&app.account_name, Style::default().fg(Color::White))
    };
    let name_block =
        Paragraph::new(Line::from(name_display)).block(Block::default().borders(Borders::ALL).title("用户名"));
    f.render_widget(name_block, inner[1]);

    let help = Paragraph::new(
        "输入密码后按 Enter 进入下一步\n继续输入可选用户名（按 Enter 跳过），然后再次按 Enter",
    )
    .style(Style::default().fg(Color::Gray));
    f.render_widget(help, inner[2]);
}

fn render_account_add_verify_code(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .margin(2)
        .split(area);

    let code_display = if app.account_code.is_empty() {
        Span::styled("输入邮件中收到的验证码", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&app.account_code, Style::default().fg(Color::White))
    };
    let code_block =
        Paragraph::new(Line::from(code_display)).block(Block::default().borders(Borders::ALL).title("验证码"));
    f.render_widget(code_block, inner[0]);

    let info = Paragraph::new(format!(
        "邮箱: {}  密码: {}  用户名: {}",
        app.account_email,
        "*".repeat(app.account_password.len()),
        app.account_name,
    ))
    .style(Style::default().fg(Color::Gray));
    f.render_widget(info, inner[1]);

    let help = Paragraph::new("输入验证码后按 Enter 提交，Esc 返回")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help, inner[2]);
}

fn render_account_login_email(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let input_display = if app.account_email.is_empty() {
        Span::styled("输入邮箱地址", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&app.account_email, Style::default().fg(Color::White))
    };
    let input_block = Paragraph::new(Line::from(input_display))
        .block(Block::default().borders(Borders::ALL).title("邮箱"));
    f.render_widget(input_block, inner[0]);

    let help = Paragraph::new("输入邮箱后按 Enter 继续，Esc 返回")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help, inner[1]);
}

fn render_account_login_password(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let pw_mask = "*".repeat(app.account_password.len());
    let pw_display = if app.account_password.is_empty() {
        Span::styled("输入密码", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&pw_mask, Style::default().fg(Color::White))
    };
    let pw_block =
        Paragraph::new(Line::from(pw_display)).block(Block::default().borders(Borders::ALL).title("密码"));
    f.render_widget(pw_block, inner[0]);

    let info = Paragraph::new(format!("邮箱: {}", app.account_email))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(info, inner[1]);
}

fn render_account_register(f: &mut Frame, _app: &App, area: Rect) {
    let msg = Paragraph::new(Span::styled(
        "正在自动注册账号，请等待…",
        Style::default().fg(Color::Yellow),
    ))
    .alignment(Alignment::Center);
    f.render_widget(msg, area);
}

fn render_search_input(f: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let input_display = if app.input.is_empty() {
        Span::styled("输入书名、作者、ISBN…", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&app.input, Style::default().fg(Color::White))
    };

    let input_block = Paragraph::new(Line::from(input_display))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(124, 77, 255)))
                .title("搜索"),
        );
    f.render_widget(input_block, inner[0]);

    let help = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  快捷键:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("    Enter — 搜索"),
        Line::from("    Esc  — 返回主菜单"),
    ])
    .style(Style::default().fg(Color::Gray));
    f.render_widget(help, inner[1]);
}

fn render_results(f: &mut Frame, app: &App, area: Rect) {
    if app.results.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "没有找到结果",
            Style::default().fg(Color::Gray),
        ))
        .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, book)| {
            let is_selected = app.list_state.selected() == Some(i);
            let base = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(124, 77, 255))
            } else {
                Style::default()
            };

            let title = truncate(&book.title, 50);
            let author =
                truncate(if book.author.is_empty() { "未知" } else { &book.author }, 15);
            let ext = &book.extension;
            let size = &book.file_size;

            let line = format!(
                " {:>2}. {:<50} | {:<15} | {:<5} | {}",
                i + 1,
                title,
                author,
                ext,
                size
            );

            ListItem::new(Line::from(Span::styled(line, base)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(124, 77, 255)))
                .title(format!(
                    "搜索结果 — 第 {} 页 ({} 条)",
                    app.current_page,
                    app.results.len()
                )),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(124, 77, 255))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.list_state.clone());
}

fn render_download(f: &mut Frame, app: &App, area: Rect) {
    if let Some(ref book) = app.selected_book {
        let info = vec![
            Line::from(Span::styled(
                format!("📖 {}", book.title),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!(
                    "作者: {}",
                    if book.author.is_empty() {
                        "未知"
                    } else {
                        &book.author
                    }
                ),
                Style::default().fg(Color::Rgb(233, 69, 96)),
            )),
            Line::from(format!(
                "格式: {} | 大小: {} | 语言: {} | 年份: {}",
                book.extension, book.file_size, book.language, book.year
            )),
        ];

        let state_text = match app.state {
            AppState::Downloading => "⏳ 下载中… 进度见底部",
            AppState::Done => {
                if app.error_msg.is_empty() {
                    "✅ 下载完成！"
                } else {
                    "❌ 下载失败"
                }
            }
            _ => "",
        };
        let state_line = Line::from(Span::styled(
            state_text,
            Style::default()
                .fg(if matches!(app.state, AppState::Done) && app.error_msg.is_empty() {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        ));

        let mut all = info;
        all.push(Line::from(""));
        all.push(state_line);

        let info_block = Paragraph::new(all)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(124, 77, 255)))
                    .title("下载"),
            )
            .alignment(Alignment::Center);
        f.render_widget(info_block, area);
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max_len {
        let mut result: String = chars.into_iter().take(max_len - 1).collect();
        result.push('…');
        result
    } else {
        s.to_string()
    }
}

fn render_progress_bar(f: &mut Frame, app: &App, area: Rect) {
    let bar_width = area.width.saturating_sub(4) as usize;
    let pct = app.download_pct.min(100);
    let filled = (bar_width as u64 * pct / 100) as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    let style = if pct >= 100 {
        Style::default().fg(Color::Green)
    } else if app.error_msg.is_empty() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Red)
    };

    let info = format!(
        " {}% | {} | {} ",
        pct,
        app.download_size,
        app.download_progress
    );

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(block.inner(area));

    let bar_line = Paragraph::new(Span::styled(&bar, style))
        .block(Block::default());
    f.render_widget(bar_line, inner[0]);
    f.render_widget(block, area);

    let info_line = Paragraph::new(Span::styled(
        info,
        Style::default().fg(Color::Gray),
    ))
    .alignment(Alignment::Right);
    f.render_widget(info_line, inner[1]);
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}