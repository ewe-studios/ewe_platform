use super::{Application, Host, New, Target};
use foundation_repl::{FormKey, FormStep, TerminalForm};
use std::{collections::BTreeSet, path::PathBuf};

const TARGETS: [Target; 3] = [Target::Web, Target::Android, Target::Ios];
enum Page {
    Name,
    Targets,
    Identifier,
    Host,
    Confirm,
}
pub struct Wizard {
    page: Page,
    row: usize,
    name: String,
    id: String,
    targets: BTreeSet<Target>,
    host: Host,
    output: PathBuf,
    error: String,
}
impl Wizard {
    pub fn new(args: New) -> Self {
        Self {
            page: Page::Name,
            row: 0,
            name: args.name.unwrap_or_default(),
            id: args.app_id.unwrap_or_default(),
            targets: if args.target.is_empty() {
                BTreeSet::from([Target::Web])
            } else {
                args.target.into_iter().collect()
            },
            host: args.host.unwrap_or_default(),
            output: args.output,
            error: String::new(),
        }
    }
    fn mobile(&self) -> bool {
        self.targets.iter().any(|v| *v != Target::Web)
    }
    fn plan(&self) -> Result<Application, String> {
        Application::new(
            self.name.clone(),
            self.targets.clone(),
            self.mobile().then(|| self.id.clone()),
            self.host,
        )
        .map_err(|e| e.to_string())
    }
}
impl TerminalForm for Wizard {
    type Output = Application;
    fn lines(&self) -> Vec<String> {
        let mut lines = vec!["Create an Ewe application".into(), String::new()];
        match self.page {
            Page::Name => lines.extend(["Application name".into(), format!("> {}_", self.name)]),
            Page::Targets => {
                lines.push("Targets".into());
                for (i, target) in TARGETS.into_iter().enumerate() {
                    lines.push(format!(
                        "{} [{}] {}",
                        if i == self.row { ">" } else { " " },
                        if self.targets.contains(&target) {
                            "x"
                        } else {
                            " "
                        },
                        target.directory()
                    ));
                }
                lines.push("  [ ] Desktop   Unavailable".into());
                lines.push("Space selects a target. Arrow keys move focus.".into());
            }
            Page::Identifier => lines.extend([
                "Mobile application identifier".into(),
                format!("> {}_", self.id),
            ]),
            Page::Host => lines.extend([
                "Mobile host".into(),
                format!("> {} (arrow keys change host)", self.host.name()),
            ]),
            Page::Confirm => {
                lines.push(format!("Create {}", self.output.join(&self.name).display()));
                lines.push(
                    self.targets
                        .iter()
                        .map(|t| t.directory())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
        }
        lines.extend([
            String::new(),
            self.error.clone(),
            "Enter continues. Shift+Tab goes back. Escape cancels.".into(),
        ]);
        lines
    }
    fn key(&mut self, key: FormKey) -> FormStep<Application> {
        self.error.clear();
        if matches!(key, FormKey::Back) {
            self.page = match self.page {
                Page::Name | Page::Targets => Page::Name,
                Page::Host => Page::Identifier,
                Page::Confirm if self.mobile() => Page::Host,
                Page::Identifier | Page::Confirm => Page::Targets,
            };
            return FormStep::Continue;
        }
        match (&self.page, key) {
            (Page::Name, FormKey::Text(c)) => self.name.push(c),
            (Page::Name, FormKey::Backspace) => {
                self.name.pop();
            }
            (Page::Identifier, FormKey::Text(c)) => self.id.push(c),
            (Page::Identifier, FormKey::Backspace) => {
                self.id.pop();
            }
            (Page::Targets, FormKey::Down) => self.row = (self.row + 1) % 3,
            (Page::Targets, FormKey::Up) => self.row = (self.row + 2) % 3,
            (Page::Targets, FormKey::Text(' ')) => {
                if !self.targets.remove(&TARGETS[self.row]) {
                    self.targets.insert(TARGETS[self.row]);
                }
            }
            (Page::Host, FormKey::Down | FormKey::Up | FormKey::Text(' ')) => {
                self.host = if self.host == Host::Server {
                    Host::OnDevice
                } else {
                    Host::Server
                }
            }
            (_, FormKey::Enter | FormKey::Next) => match self.page {
                Page::Name => match Application::new(
                    self.name.clone(),
                    BTreeSet::from([Target::Web]),
                    None,
                    Host::Server,
                ) {
                    Ok(_) => self.page = Page::Targets,
                    Err(e) => self.error = e.to_string(),
                },
                Page::Targets if self.targets.is_empty() => {
                    self.error = "Select at least one target.".into();
                }
                Page::Targets => {
                    if self.mobile() {
                        self.page = Page::Identifier;
                    } else {
                        self.host = Host::Server;
                        self.page = Page::Confirm;
                    }
                }
                Page::Identifier => match self.plan() {
                    Ok(_) => self.page = Page::Host,
                    Err(e) => self.error = e,
                },
                Page::Host => self.page = Page::Confirm,
                Page::Confirm => match self.plan() {
                    Ok(p) => return FormStep::Accept(p),
                    Err(e) => self.error = e,
                },
            },
            _ => {}
        }
        FormStep::Continue
    }
}
