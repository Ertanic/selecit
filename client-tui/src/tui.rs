use crate::proto::{LogsQueryRequest, QueryRequest, TableRow, query_client::QueryClient, table_col, table_col::Data};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{
        event,
        event::{Event, KeyCode, KeyModifiers, ModifierKeyCode},
    },
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::Line,
    widgets::{Block, List, Paragraph, Row, Table, Tabs},
};
use std::{io, sync::Arc};
use tokio::{runtime::Handle, sync::RwLock, task::block_in_place};
use tonic::{codegen::tokio_stream::StreamExt, transport::Channel};
use tui_input::{Input, backend::crossterm::EventHandler};

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

#[derive(Default, PartialEq)]
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

    input: Input,

    client: Option<QueryClient<Channel>>,
    _logs_cache: Arc<RwLock<Vec<String>>>,
    _logs_listener: Option<tokio::task::JoinHandle<()>>,
    _query_cache: Option<Vec<TableRow>>,
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

            let event = event::read()?;
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                    KeyCode::Enter if self.app_mode == AppMode::Input => {
                        if let Some(ref mut client) = self.client {
                            let command = self.input.value_and_reset();
                            let result = block_in_place(|| Handle::current().block_on(async move { client.query(QueryRequest { command }).await }));
                            match result {
                                Ok(response) => {
                                    let rows = response.into_inner().rows;
                                    self._query_cache = Some(rows);
                                }
                                Err(err) => block_in_place(|| {
                                    Handle::current().block_on(async {
                                        self._logs_cache
                                            .write()
                                            .await
                                            .push(format!("an error occurred while trying to execute the command: {err}"))
                                    });
                                }),
                            }
                        }
                    }
                    KeyCode::Right if let Some(tab) = self.current_tab.next() => {
                        self.stop_editing();
                        self.current_tab = tab;
                    }
                    KeyCode::Left if let Some(tab) = self.current_tab.back() => {
                        self.start_editing();
                        self.current_tab = tab;
                    }
                    _ if self.app_mode == AppMode::Input => {
                        self.input.handle_event(&event);
                    }
                    _ => {}
                }
            }
        }
    }

    fn start_editing(&mut self) {
        self.app_mode = AppMode::Input;
    }

    fn stop_editing(&mut self) {
        self.app_mode = AppMode::Normal;
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
        let [table_area, input_area] = Layout::vertical([Constraint::Fill(8), Constraint::Fill(2)]).areas(content_area);

        let (rows, widths) = if let Some(rows) = self._query_cache.as_ref() {
            if let Some(first) = rows.first() {
                let widths = first.cols.iter().map(|_| Constraint::Percentage(30)).collect();

                let cols = first.cols.iter().map(|c| c.key.clone()).collect::<Vec<_>>();
                let body = rows
                    .into_iter()
                    .map(|r| {
                        Row::new(
                            r.cols
                                .iter()
                                .map(|c| {
                                    if let Some(ref data) = c.data {
                                        match data {
                                            Data::Str(str) => str.clone(),
                                            Data::Integer(int) => int.to_string(),
                                            Data::Floating(float) => float.to_string(),
                                            Data::Boolean(b) => b.to_string(),
                                        }
                                    }
                                    else {
                                        "".to_string()
                                    }
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();
                let rows = vec![Row::new(cols)].into_iter().chain(body).collect();

                (rows, widths)
            }
            else {
                (vec![], vec![])
            }
        }
        else {
            (vec![], vec![])
        };

        let table = Table::new(rows, widths).block(Block::bordered());
        frame.render_widget(table, table_area);

        let input = Paragraph::new(self.input.value()).block(Block::bordered().title("Command"));
        frame.render_widget(input, input_area);
    }

    fn render_logs(&self, frame: &mut Frame, content_area: Rect) {
        let logs = block_in_place(|| Handle::current().block_on(async { self._logs_cache.read().await }));

        let list = List::new(logs.iter().cloned()).block(Block::bordered());

        frame.render_widget(list, content_area);
    }
}
