//! The build file grammar (feature 14, section 6.1).
//!
//! [`Sandboxfile::parse`] turns the text of a build file into [`Step`] values.
//! [`Sandboxfile::print`] gives the canonical form: parsing that form gives the
//! same steps (invariant I2).
//!
//! Parsing is total: every input gives a [`Sandboxfile`] or a typed
//! [`SandboxError::InvalidSpec`] with the line number in its reason (invariant
//! I1). It never panics.
//!
//! One instruction is one logical line. A line that ends with a backslash
//! continues on the next line. A line whose first character after spaces is
//! `#` is a comment. A `RUN` line that ends with `<<WORD` takes the lines that
//! follow, up to a line that is exactly `WORD`, as its script.

use std::path::{Path, PathBuf};

use foundation_errstacks::ErrorTrace;

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::limits::BuildLimits;
use crate::shared::spec::{ImageRef, NetworkMode};

/// The parsed build file: one instruction for each step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sandboxfile {
    /// The steps, in file order. The first step is `FROM`.
    pub steps: Vec<Step>,
}

/// One instruction with its line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The number of the first physical line of the instruction, starting at 1.
    pub line: u32,
    /// The instruction.
    pub kind: StepKind,
}

/// The base image of a `FROM` step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseRef {
    /// A version of feature 03's Alpine manifest, for example `3.22`.
    Alpine(String),
    /// A tag or a key in the local image store.
    Image(ImageRef),
    /// An image name or identifier in the local Docker daemon.
    Docker(String),
    /// A tar file of a root file system on the host.
    Tar(PathBuf),
}

/// One instruction of a build file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepKind {
    /// The base image. It is always the first step.
    From(BaseRef),
    /// The kernel name of feature 03's manifest, or `default`.
    Kernel(String),
    /// One environment entry.
    Env {
        /// The key.
        key: String,
        /// The value.
        value: String,
    },
    /// The work directory. It is absolute.
    Workdir(String),
    /// The user of the final image: `uid`, `uid:gid`, `name` or
    /// `name:group`.
    User(String),
    /// Copy files from the build context into the image.
    Add {
        /// The source, relative to the build context.
        source: String,
        /// The absolute destination in the image.
        destination: String,
        /// The file mode, when the instruction names one.
        mode: Option<u32>,
    },
    /// Download one file and put it in the image.
    Fetch {
        /// The `http://` or `https://` address.
        url: String,
        /// The expected SHA-256, in lowercase hexadecimal.
        sha256: String,
        /// The absolute destination file path in the image.
        destination: String,
    },
    /// Run one program in a build sandbox.
    Run {
        /// The program and its arguments. The default is `/bin/sh -e -s`.
        shell: Vec<String>,
        /// The network mode of this step only.
        network: NetworkMode,
        /// The program text, on standard input.
        script: String,
    },
    /// The start command of the final image.
    Cmd(Vec<String>),
}

/// The default shell of a `RUN` step.
pub const DEFAULT_SHELL: [&str; 3] = ["/bin/sh", "-e", "-s"];

/// A physical or logical line with its number.
struct LogicalLine {
    /// The number of the first physical line.
    number: u32,
    /// The text with continuations joined.
    text: String,
    /// The body of a here-document, when the line names one.
    heredoc: Option<String>,
}

impl Sandboxfile {
    /// Parses a build file with the default limits.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] with `field = "sandboxfile"` and a reason
    /// that starts with `line <n>: `.
    pub fn parse(text: &str) -> SandboxResult<Self> {
        Self::parse_with_limits(text, &BuildLimits::default())
    }

    /// Parses a build file with `limits`.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] with `field = "sandboxfile"` and a reason
    /// that starts with `line <n>: `.
    pub fn parse_with_limits(text: &str, limits: &BuildLimits) -> SandboxResult<Self> {
        if u64::try_from(text.len()).unwrap_or(u64::MAX) > limits.max_file_bytes {
            return Err(invalid(
                1,
                format!(
                    "the build file is larger than {} bytes",
                    limits.max_file_bytes
                ),
            ));
        }
        let lines = logical_lines(text)?;
        let mut steps = Vec::new();
        let mut seen_from = false;
        let mut seen_cmd = false;
        let mut seen_kernel = false;
        for line in lines {
            if steps.len() >= limits.max_steps {
                return Err(invalid(
                    line.number,
                    format!("the build file has more than {} steps", limits.max_steps),
                ));
            }
            let step = parse_line(&line)?;
            match &step.kind {
                StepKind::From(_) => {
                    if seen_from {
                        return Err(invalid(
                            line.number,
                            "the build file has more than one FROM",
                        ));
                    }
                    if !steps.is_empty() {
                        return Err(invalid(line.number, "FROM must be the first instruction"));
                    }
                    seen_from = true;
                }
                StepKind::Cmd(_) => {
                    if seen_cmd {
                        return Err(invalid(line.number, "the build file has more than one CMD"));
                    }
                    seen_cmd = true;
                }
                StepKind::Kernel(_) => {
                    if seen_kernel {
                        return Err(invalid(
                            line.number,
                            "the build file has more than one KERNEL",
                        ));
                    }
                    seen_kernel = true;
                }
                _ => {
                    if !seen_from {
                        return Err(invalid(line.number, "the first instruction must be FROM"));
                    }
                }
            }
            steps.push(step);
        }
        if !seen_from {
            return Err(invalid(1, "the first instruction must be FROM"));
        }
        Ok(Self { steps })
    }

    /// The canonical text of this build file.
    ///
    /// `parse` of this text gives the same steps (invariant I2).
    #[must_use]
    pub fn print(&self) -> String {
        let mut text = String::new();
        for step in &self.steps {
            text.push_str(&print_step(step));
        }
        text
    }

    /// The canonical text of each step, in order.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.steps.iter().map(print_step).collect()
    }
}

/// The canonical text of one step, with its final line break.
fn print_step(step: &Step) -> String {
    match &step.kind {
        StepKind::From(base) => format!("FROM {}\n", print_base(base)),
        StepKind::Kernel(name) => format!("KERNEL {name}\n"),
        StepKind::Env { key, value } => {
            format!("ENV {key}={}\n", quote_value(value))
        }
        StepKind::Workdir(path) => format!("WORKDIR {}\n", quote_token(path)),
        StepKind::User(user) => format!("USER {}\n", quote_token(user)),
        StepKind::Add {
            source,
            destination,
            mode,
        } => {
            let mode = mode.map_or_else(String::new, |mode| format!("--mode {mode:04o} "));
            format!(
                "ADD {mode}{} {}\n",
                quote_token(source),
                quote_token(destination)
            )
        }
        StepKind::Fetch {
            url,
            sha256,
            destination,
        } => format!(
            "FETCH {url} sha256:{sha256} AS {}\n",
            quote_token(destination)
        ),
        StepKind::Run {
            shell,
            network,
            script,
        } => print_run(shell, *network, script),
        StepKind::Cmd(argv) => {
            let json = serde_json::to_string(argv).unwrap_or_else(|_| "[]".to_string());
            format!("CMD {json}\n")
        }
    }
}

/// The canonical text of one base reference.
fn print_base(base: &BaseRef) -> String {
    match base {
        BaseRef::Alpine(version) => format!("alpine:{version}"),
        BaseRef::Image(ImageRef::Tag(tag)) => format!("image:{tag}"),
        BaseRef::Image(ImageRef::Key(key)) => format!("image:{key}"),
        BaseRef::Docker(name) => format!("docker:{name}"),
        BaseRef::Tar(path) => format!("tar:{}", path.display()),
    }
}

/// The canonical text of one `RUN` step.
///
/// A single-line script stays on the instruction line. A script with a line
/// break goes to a here-document with a terminator that the script does not
/// contain.
fn print_run(shell: &[String], network: NetworkMode, script: &str) -> String {
    let mut head = String::from("RUN");
    if shell != DEFAULT_SHELL {
        head.push_str(" --shell ");
        head.push_str(&quote_token(&shell_join(shell)));
    }
    if network == NetworkMode::Host {
        head.push_str(" --network host");
    }
    if !script.contains('\n') && !script.is_empty() {
        return format!("{head} {script}\n");
    }
    let terminator = heredoc_terminator(script);
    let mut body = script.to_string();
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    format!("{head} <<{terminator}\n{body}{terminator}\n")
}

/// A terminator that no line of `script` is equal to.
fn heredoc_terminator(script: &str) -> String {
    let mut word = String::from("EOF");
    while script.lines().any(|line| line == word) {
        word.push('_');
    }
    word
}

/// Splits `text` into logical lines, with continuations and here-documents.
fn logical_lines(text: &str) -> SandboxResult<Vec<LogicalLine>> {
    let mut physical: Vec<(u32, &str)> = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let line = line.strip_suffix('\r').unwrap_or(line);
        physical.push((number, line));
    }
    // `split('\n')` gives one empty last item for text that ends with a line
    // break. It is not a line.
    if text.ends_with('\n') {
        physical.pop();
    }
    let mut lines = Vec::new();
    let mut index = 0;
    while index < physical.len() {
        let (number, first) = physical[index];
        index += 1;
        let mut text = first.to_string();
        while text.ends_with('\\') && index < physical.len() {
            text.pop();
            text.push_str(physical[index].1);
            index += 1;
        }
        let trimmed = text.trim_start_matches(' ');
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (text, heredoc_word) = split_heredoc(&text);
        let mut heredoc = None;
        if let Some(word) = heredoc_word {
            let mut body = String::new();
            let mut closed = false;
            while index < physical.len() {
                let (_, line) = physical[index];
                index += 1;
                if line == word {
                    closed = true;
                    break;
                }
                body.push_str(line);
                body.push('\n');
            }
            if !closed {
                return Err(invalid(
                    number,
                    format!("the here-document {word} is not ended"),
                ));
            }
            heredoc = Some(body);
        }
        lines.push(LogicalLine {
            number,
            text,
            heredoc,
        });
    }
    Ok(lines)
}

/// Removes a trailing `<<WORD` from `text` and gives it back.
fn split_heredoc(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim_end();
    let Some(start) = trimmed.rfind("<<") else {
        return (text.to_string(), None);
    };
    let word = &trimmed[start + 2..];
    let valid = !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if !valid {
        return (text.to_string(), None);
    }
    let before = trimmed[..start].trim_end().to_string();
    (before, Some(word.to_string()))
}

/// Parses one logical line into a step.
fn parse_line(line: &LogicalLine) -> SandboxResult<Step> {
    let text = line.text.as_str();
    let (word, rest) = split_word(text);
    if word.is_empty() {
        return Err(invalid(line.number, "the line has no instruction"));
    }
    let kind = parse_instruction(word, rest, line)?;
    Ok(Step {
        line: line.number,
        kind,
    })
}

/// Parses one instruction word and its arguments.
fn parse_instruction(word: &str, rest: &str, line: &LogicalLine) -> SandboxResult<StepKind> {
    let upper = word.to_ascii_uppercase();
    match upper.as_str() {
        "FROM" => {
            let value = rest.trim();
            if value.is_empty() {
                return Err(invalid(line.number, "FROM needs a base"));
            }
            Ok(StepKind::From(parse_base(value, line.number)?))
        }
        "KERNEL" => {
            let value = rest.trim();
            if value.is_empty() {
                return Err(invalid(line.number, "KERNEL needs a name"));
            }
            if value.split_whitespace().count() != 1 {
                return Err(invalid(line.number, "KERNEL takes one name"));
            }
            Ok(StepKind::Kernel(value.to_string()))
        }
        "ENV" => parse_env(rest, line.number),
        "WORKDIR" => {
            let value = rest.trim();
            if value.is_empty() {
                return Err(invalid(line.number, "WORKDIR needs a path"));
            }
            if !Path::new(value).is_absolute() {
                return Err(invalid(line.number, "the WORKDIR path is not absolute"));
            }
            Ok(StepKind::Workdir(value.to_string()))
        }
        "USER" => {
            let value = rest.trim();
            if value.is_empty() {
                return Err(invalid(line.number, "USER needs a user"));
            }
            if value.split_whitespace().count() != 1 {
                return Err(invalid(line.number, "USER takes one user"));
            }
            Ok(StepKind::User(value.to_string()))
        }
        "ADD" => parse_add(rest, line.number),
        "FETCH" => parse_fetch(rest, line.number),
        "RUN" => parse_run(rest, line, line.number),
        "CMD" => {
            let value = rest.trim();
            if value.is_empty() {
                return Err(invalid(line.number, "CMD needs a JSON array of strings"));
            }
            let argv: Vec<String> = serde_json::from_str(value)
                .map_err(|_| invalid(line.number, "CMD needs a JSON array of strings"))?;
            if argv.is_empty() {
                return Err(invalid(
                    line.number,
                    "CMD needs a JSON array with at least one element",
                ));
            }
            Ok(StepKind::Cmd(argv))
        }
        other => Err(invalid(line.number, format!("unknown instruction {other}"))),
    }
}

/// Parses a `FROM` base.
fn parse_base(value: &str, line: u32) -> SandboxResult<BaseRef> {
    if let Some(version) = value.strip_prefix("alpine:") {
        if version.is_empty() {
            return Err(invalid(line, "the Alpine version is empty"));
        }
        return Ok(BaseRef::Alpine(version.to_string()));
    }
    if let Some(name) = value.strip_prefix("image:") {
        if name.is_empty() {
            return Err(invalid(line, "the image reference is empty"));
        }
        let reference = if name.len() == 32 && name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            ImageRef::Key(name.to_string())
        } else {
            ImageRef::Tag(name.to_string())
        };
        return Ok(BaseRef::Image(reference));
    }
    if let Some(name) = value.strip_prefix("docker:") {
        if name.is_empty() {
            return Err(invalid(line, "the Docker image name is empty"));
        }
        return Ok(BaseRef::Docker(name.to_string()));
    }
    if let Some(path) = value.strip_prefix("tar:") {
        if path.is_empty() {
            return Err(invalid(line, "the tar path is empty"));
        }
        return Ok(BaseRef::Tar(PathBuf::from(path)));
    }
    if value.starts_with("oci:") {
        return Err(invalid(line, "oci: is not available in this build"));
    }
    Err(invalid(
        line,
        "FROM needs alpine:<version>, image:<tag-or-key>, docker:<name> or tar:<path>",
    ))
}

/// Parses an `ENV` instruction.
fn parse_env(rest: &str, line: u32) -> SandboxResult<StepKind> {
    let Some((key, value)) = rest.trim_start().split_once('=') else {
        return Err(invalid(line, "ENV needs KEY=value"));
    };
    if key.is_empty() {
        return Err(invalid(line, "the ENV key is empty"));
    }
    if key.contains(char::is_whitespace) {
        return Err(invalid(line, "the ENV key contains a space"));
    }
    let value = unquote_value(value.trim_end());
    Ok(StepKind::Env {
        key: key.to_string(),
        value,
    })
}

/// Parses an `ADD` instruction.
fn parse_add(rest: &str, line: u32) -> SandboxResult<StepKind> {
    let mut words = Words::new(rest);
    let mut mode = None;
    if words.peek() == Some("--mode") {
        words.next();
        let Some(value) = words.next() else {
            return Err(invalid(line, "ADD --mode needs an octal mode"));
        };
        mode = Some(parse_mode(&value, line)?);
    }
    let Some(source) = words.next() else {
        return Err(invalid(line, "ADD needs a source and a destination"));
    };
    let Some(destination) = words.next() else {
        return Err(invalid(line, "ADD needs a source and a destination"));
    };
    if words.next().is_some() {
        return Err(invalid(line, "ADD takes a source and a destination"));
    }
    if Path::new(&source).is_absolute() {
        return Err(invalid(
            line,
            "the ADD source must be relative to the build context",
        ));
    }
    if !Path::new(&destination).is_absolute() {
        return Err(invalid(line, "the ADD destination is not absolute"));
    }
    Ok(StepKind::Add {
        source,
        destination,
        mode,
    })
}

/// Parses one octal file mode.
fn parse_mode(value: &str, line: u32) -> SandboxResult<u32> {
    let digits = value.strip_prefix("0o").unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err(invalid(line, "the ADD mode is not an octal number"));
    }
    let mode = u32::from_str_radix(digits, 8)
        .map_err(|_| invalid(line, "the ADD mode is not an octal number"))?;
    if mode > 0o7777 {
        return Err(invalid(line, "the ADD mode is larger than 07777"));
    }
    Ok(mode)
}

/// Parses a `FETCH` instruction.
fn parse_fetch(rest: &str, line: u32) -> SandboxResult<StepKind> {
    let mut words = Words::new(rest);
    let Some(url) = words.next() else {
        return Err(invalid(
            line,
            "FETCH needs a URL, a sha256: hash and AS <destination>",
        ));
    };
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(invalid(
            line,
            "the FETCH address is not http:// or https://",
        ));
    }
    let Some(hash) = words.next() else {
        return Err(invalid(
            line,
            "FETCH needs a URL, a sha256: hash and AS <destination>",
        ));
    };
    let Some(sha256) = hash.strip_prefix("sha256:") else {
        return Err(invalid(line, "the FETCH hash starts with sha256:"));
    };
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(
            line,
            "the FETCH hash is not 64 hexadecimal characters",
        ));
    }
    let Some(phrase) = words.next() else {
        return Err(invalid(
            line,
            "FETCH needs a URL, a sha256: hash and AS <destination>",
        ));
    };
    if phrase != "AS" {
        return Err(invalid(line, "FETCH needs AS before the destination"));
    }
    let Some(destination) = words.next() else {
        return Err(invalid(line, "FETCH needs a destination after AS"));
    };
    if words.next().is_some() {
        return Err(invalid(line, "FETCH takes one destination"));
    }
    if !Path::new(&destination).is_absolute() {
        return Err(invalid(line, "the FETCH destination is not absolute"));
    }
    Ok(StepKind::Fetch {
        url,
        sha256: sha256.to_ascii_lowercase(),
        destination,
    })
}

/// Parses a `RUN` instruction, with or without a here-document.
fn parse_run(rest: &str, line: &LogicalLine, number: u32) -> SandboxResult<StepKind> {
    let mut shell = DEFAULT_SHELL.map(str::to_string).to_vec();
    let mut network = NetworkMode::None;
    let mut words = Words::new(rest);
    while let Some(word) = words.peek() {
        match word {
            "--shell" => {
                words.next();
                let Some(value) = words.next() else {
                    return Err(invalid(number, "RUN --shell needs a program"));
                };
                let program = shell_split(&value);
                if program.is_empty() {
                    return Err(invalid(number, "the RUN shell is empty"));
                }
                shell = program;
            }
            "--network" => {
                words.next();
                let Some(value) = words.next() else {
                    return Err(invalid(number, "RUN --network needs none or host"));
                };
                network = parse_network(&value, number)?;
            }
            _ => break,
        }
    }
    let text = words.remaining().trim().to_string();
    if let Some(script) = &line.heredoc {
        if !text.is_empty() {
            return Err(invalid(
                number,
                "RUN has a here-document and text on the instruction line",
            ));
        }
        return Ok(StepKind::Run {
            shell,
            network,
            script: script.clone(),
        });
    }
    if text.is_empty() {
        return Err(invalid(number, "RUN needs a program or a here-document"));
    }
    Ok(StepKind::Run {
        shell,
        network,
        script: text,
    })
}

/// Parses one network mode.
fn parse_network(value: &str, line: u32) -> SandboxResult<NetworkMode> {
    match value {
        "none" => Ok(NetworkMode::None),
        "host" => Ok(NetworkMode::Host),
        _ => Err(invalid(line, "the RUN network is none or host")),
    }
}

/// The text of a step that failed to parse.
fn invalid(line: u32, reason: impl std::fmt::Display) -> ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: "sandboxfile".to_string(),
        reason: format!("line {line}: {reason}"),
    }
    .into()
}

/// Splits the instruction word from the rest of the line.
fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim_start_matches(' ');
    match text.find(|character: char| character.is_whitespace()) {
        Some(index) => (&text[..index], text[index..].trim_start()),
        None => (text, ""),
    }
}

/// A word reader that removes surrounding double quotes and backslash
/// escapes.
struct Words<'a> {
    rest: &'a str,
}

impl<'a> Words<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            rest: text.trim_start(),
        }
    }

    fn peek(&self) -> Option<&'a str> {
        let rest = self.rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        if let Some(inner) = rest.strip_prefix('"') {
            let end = inner.find('"')?;
            return Some(&inner[..end]);
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        Some(&rest[..end])
    }

    fn next(&mut self) -> Option<String> {
        let rest = self.rest.trim_start();
        if rest.is_empty() {
            self.rest = "";
            return None;
        }
        if let Some(inner) = rest.strip_prefix('"') {
            let end = inner.find('"')?;
            let value = unescape(&inner[..end]);
            self.rest = &inner[end + 1..];
            return Some(value);
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let value = rest[..end].to_string();
        self.rest = &rest[end..];
        Some(value)
    }

    fn remaining(&self) -> &'a str {
        self.rest
    }
}

/// Removes the surrounding double quotes of an `ENV` value and its escapes.
fn unquote_value(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        unescape(&value[1..value.len() - 1])
    } else {
        value.to_string()
    }
}

/// Resolves the backslash escapes of a quoted value.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(next) = characters.next() {
                out.push(next);
                continue;
            }
        }
        out.push(character);
    }
    out
}

/// The canonical `ENV` value: quoted only when it needs quotes.
fn quote_value(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.trim() != value
        || value
            .chars()
            .any(|character| matches!(character, '"' | '\\' | '#' | '\n' | '\r'));
    if needs_quotes {
        quote_token(value)
    } else {
        value.to_string()
    }
}

/// One token with quotes and escapes when the text needs them.
fn quote_token(value: &str) -> String {
    let safe = !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    '-' | '_' | '.' | '/' | ':' | '=' | '+' | ',' | '@' | '%'
                )
        });
    if safe {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        if matches!(character, '"' | '\\') {
            out.push('\\');
        }
        out.push(character);
    }
    out.push('"');
    out
}

/// Joins an argument vector into one command line that [`shell_split`] reads
/// back.
fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|word| quote_token(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits one command line into an argument vector.
///
/// Double quotes group words, and a backslash escapes the next character.
fn shell_split(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => {
                if let Some(next) = characters.next() {
                    word.push(next);
                }
                started = true;
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            character => {
                word.push(character);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}
