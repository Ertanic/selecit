use crate::proto::{LogsQueryRequest, query_client::QueryClient};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{event, event::KeyCode},
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, List, Row, Table, Tabs},
};
use std::{io, sync::Arc};
use tokio::{runtime::Handle, sync::RwLock, task::block_in_place};
use tonic::{codegen::tokio_stream::StreamExt, transport::Channel};

#[derive(Default, Clone, Copy)]
enum Tab {
    #[default]
    Query = 0,
    Logs = 1,
}

impl Tab {
    fn next(self) -> Option<Self> {
        match self {
            Tab::Query => Some(Tab::Logs),
            Tab::Logs => None,
        }
    }

    fn back(self) -> Option<Self> {
        match self {
            Tab::Query => None,
            Tab::Logs => Some(Tab::Query),
        }
    }
}

#[derive(Default)]
enum AppMode {
    #[default]
    Input,
    Normal,
}

#[derive(Default)]
pub struct App {
    current_tab: Tab,
    app_mode: AppMode,
    addr: String,

    client: Option<QueryClient<Channel>>,
    _logs_cache: Arc<RwLock<Vec<String>>>,
    _logs_listener: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    pub async fn new(addr: String, mut client: QueryClient<Channel>) -> Self {
        let mut stream = client.logs(LogsQueryRequest {}).await.expect("unable to load server logs").into_inner();
        let logs = Arc::new(RwLock::new(Vec::new()));

        let listener = tokio::spawn({
            let logs = logs.clone();
            async move {
                loop {
                    while let Some(entry) = stream.next().await {
                        match entry {
                            Ok(entry) => {
                                for entry in entry.log {
                                    logs.write().await.push(entry);
                                }
                            }
                            Err(err) => {
                                logs.write().await.push(format!("unable to read logs because: {}", err));
                            }
                        }
                    }
                }
            }
        });

        Self {
            addr,
            client: Some(client),
            _logs_listener: Some(listener),
            _logs_cache: logs,
            ..Default::default()
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Right if let Some(tab) = self.current_tab.next() => self.current_tab = tab,
                    KeyCode::Left if let Some(tab) = self.current_tab.back() => self.current_tab = tab,
                    _ => {}
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let [title_area, tabs_area, content_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(frame.area());

        let title = Line::from(format!("Selecit Client - connected to {}", self.addr)).centered();
        frame.render_widget(title, title_area);

        let tabs = Tabs::new(vec!["Query", "Logs"]).select(self.current_tab as usize);
        frame.render_widget(tabs, tabs_area);

        match self.current_tab {
            Tab::Query => self.render_query(frame, content_area),
            Tab::Logs => self.render_logs(frame, content_area),
        }
    }

    fn render_query(&self, frame: &mut Frame, content_area: Rect) {
        let table = Table::new([Row::new(["Query"])], [Constraint::Fill(1)]);
        frame.render_widget(table, content_area);
    }

    fn render_logs(&self, frame: &mut Frame, content_area: Rect) {
        let logs = block_in_place(|| Handle::current().block_on(async { self._logs_cache.read().await }));

        let list = List::new(logs.iter().cloned()).block(Block::bordered());

        frame.render_widget(list, content_area);
    }
}
