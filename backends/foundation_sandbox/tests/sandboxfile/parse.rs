//! The grammar rows of feature 14, section 7: I1, I2 and the syntax table.

use foundation_sandbox::native::images::sandboxfile::{BaseRef, Sandboxfile, Step, StepKind};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::spec::{ImageRef, NetworkMode};
use proptest::prelude::*;

/// The reason of a failed parse.
fn reason(text: &str) -> String {
    match Sandboxfile::parse(text) {
        Ok(file) => panic!("the text parses: {file:?}"),
        Err(error) => match error.current_context() {
            SandboxError::InvalidSpec { field, reason } => {
                assert_eq!(field, "sandboxfile");
                reason.clone()
            }
            other => panic!("the error is not InvalidSpec: {other:?}"),
        },
    }
}

#[test]
fn parse_table() {
    // FROM: each source form, and `oci:` is refused.
    for (text, base) in [
        ("FROM alpine:3.22\n", BaseRef::Alpine("3.22".to_string())),
        (
            "FROM image:app\n",
            BaseRef::Image(ImageRef::Tag("app".to_string())),
        ),
        (
            "FROM docker:python:3.12-slim\n",
            BaseRef::Docker("python:3.12-slim".to_string()),
        ),
        (
            "FROM tar:/images/root.tar\n",
            BaseRef::Tar("/images/root.tar".into()),
        ),
    ] {
        let file = Sandboxfile::parse(text).expect("the FROM line parses");
        assert_eq!(file.steps[0].kind, StepKind::From(base));
    }
    assert!(reason("FROM oci:example.com/app:1\n").contains("oci:"));

    // The instruction word is not case sensitive.
    let file = Sandboxfile::parse("from alpine:3.22\nworkdir /opt\n").expect("lower case parses");
    assert!(matches!(file.steps[1].kind, StepKind::Workdir(_)));

    // KERNEL: one name.
    let file = Sandboxfile::parse("FROM alpine:3.22\nKERNEL default\n").expect("KERNEL parses");
    assert_eq!(file.steps[1].kind, StepKind::Kernel("default".to_string()));
    assert!(reason("FROM alpine:3.22\nKERNEL a b\n").contains("one name"));

    // ENV: the value is the rest of the line, and quotes are removed.
    let file =
        Sandboxfile::parse("FROM alpine:3.22\nENV A=x=y\nENV B=\" two \"\n").expect("ENV parses");
    assert_eq!(
        file.steps[1].kind,
        StepKind::Env {
            key: "A".to_string(),
            value: "x=y".to_string()
        }
    );
    assert_eq!(
        file.steps[2].kind,
        StepKind::Env {
            key: "B".to_string(),
            value: " two ".to_string()
        }
    );
    assert!(reason("FROM alpine:3.22\nENV NOVALUE\n").contains("KEY=value"));

    // WORKDIR: absolute.
    assert!(reason("FROM alpine:3.22\nWORKDIR relative\n").contains("not absolute"));

    // USER: one word.
    assert!(reason("FROM alpine:3.22\nUSER two words\n").contains("one user"));

    // ADD: a source and an absolute destination, and an optional octal mode.
    let file = Sandboxfile::parse("FROM alpine:3.22\nADD --mode 0755 ./tool /bin/tool\n")
        .expect("ADD parses");
    assert_eq!(
        file.steps[1].kind,
        StepKind::Add {
            source: "./tool".to_string(),
            destination: "/bin/tool".to_string(),
            mode: Some(0o755)
        }
    );
    let error = reason("FROM alpine:3.22\nADD onlyone\n");
    assert_eq!(error, "line 2: ADD needs a source and a destination");
    assert!(reason("FROM alpine:3.22\nADD ./x relative\n").contains("not absolute"));
    assert!(reason("FROM alpine:3.22\nADD /abs /x\n").contains("relative"));

    // FETCH: the URL, the hash and AS.
    let hash = "9f2c".repeat(16);
    let text = format!("FROM alpine:3.22\nFETCH https://example.com/t sha256:{hash} AS /t\n");
    let file = Sandboxfile::parse(&text).expect("FETCH parses");
    assert!(matches!(file.steps[1].kind, StepKind::Fetch { .. }));
    assert!(reason("FROM alpine:3.22\nFETCH ftp://x a AS /x\n").contains("http://"));
    assert!(
        reason("FROM alpine:3.22\nFETCH https://x sha256:00 AS /x\n").contains("64 hexadecimal")
    );

    // RUN: the default shell, the network mode and a here-document.
    let file = Sandboxfile::parse("FROM alpine:3.22\nRUN echo hi\n").expect("RUN parses");
    let StepKind::Run {
        shell,
        network,
        script,
    } = &file.steps[1].kind
    else {
        panic!("the step is not RUN");
    };
    assert_eq!(shell, &["/bin/sh", "-e", "-s"]);
    assert_eq!(*network, NetworkMode::None);
    assert_eq!(script, "echo hi");

    let text = "FROM alpine:3.22\nRUN --shell \"python3 -\" --network host <<EOF\nprint(1)\nEOF\n";
    let file = Sandboxfile::parse(text).expect("the here-document parses");
    assert_eq!(
        file.steps[1].kind,
        StepKind::Run {
            shell: vec!["python3".to_string(), "-".to_string()],
            network: NetworkMode::Host,
            script: "print(1)\n".to_string(),
        }
    );
    assert!(reason("FROM alpine:3.22\nRUN --network wan echo\n").contains("none or host"));
    assert!(reason("FROM alpine:3.22\nRUN <<EOF\nx\n").contains("not ended"));
}

#[test]
fn parse_rules() {
    // CMD: a JSON array of strings.
    let file = Sandboxfile::parse("FROM alpine:3.22\nCMD [\"sh\", \"-c\", \"echo ok\"]\n")
        .expect("CMD parses");
    assert_eq!(
        file.steps[1].kind,
        StepKind::Cmd(vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo ok".to_string()
        ])
    );
    assert!(reason("FROM alpine:3.22\nCMD onlyone\n").contains("JSON array"));

    // The line continuation joins the next physical line.
    let file = Sandboxfile::parse("FROM alpine:3.22\nRUN echo one \\\nand two\n")
        .expect("the continuation parses");
    assert_eq!(
        file.steps[1].kind,
        StepKind::Run {
            shell: vec!["/bin/sh".to_string(), "-e".to_string(), "-s".to_string()],
            network: NetworkMode::None,
            script: "echo one and two".to_string()
        }
    );
    assert_eq!(file.steps[1].line, 2);

    // A comment and an empty line are ignored.
    let file = Sandboxfile::parse("# a comment\n\nFROM alpine:3.22\n  # another\n")
        .expect("the comments are ignored");
    assert_eq!(file.steps.len(), 1);
    assert_eq!(file.steps[0].line, 3);

    // The first instruction is FROM, and FROM, CMD and KERNEL appear once.
    assert!(reason("RUN echo x\n").contains("must be FROM"));
    assert!(reason("").contains("must be FROM"));
    assert!(reason("FROM alpine:3.22\nFROM alpine:3.23\n").contains("more than one FROM"));
    assert!(reason("FROM alpine:3.22\nCMD [\"a\"]\nCMD [\"b\"]\n").contains("more than one CMD"));
    assert!(reason("FROM alpine:3.22\nKERNEL a\nKERNEL b\n").contains("more than one KERNEL"));

    // An unknown instruction names its line.
    let error = reason("FROM alpine:3.22\nCOPY a b\n");
    assert_eq!(error, "line 2: unknown instruction COPY");
}

proptest! {
    #![proptest_config(crate::control::support::property_config(0x14_01))]

    /// Invariant I1: any valid UTF-8 text gives a result, and the parser never
    /// panics.
    #[test]
    fn parse_never_panics(text in any::<String>()) {
        let _ = Sandboxfile::parse(&text);
    }

    /// Invariant I2: the canonical form parses to the same steps.
    #[test]
    fn parse_print_roundtrip(file in file_strategy()) {
        let printed = file.print();
        let parsed = Sandboxfile::parse(&printed)
            .unwrap_or_else(|error| panic!("the canonical form does not parse: {error}\n{printed}"));
        prop_assert_eq!(parsed, file);
    }
}

/// A step strategy of a file whose printed form is one physical line per step.
fn file_strategy() -> impl Strategy<Value = Sandboxfile> {
    let from = (word_strategy(1, 8), 0_u8..5).prop_map(|(value, choice)| {
        let base = match choice {
            0 => BaseRef::Alpine(value),
            1 => BaseRef::Image(ImageRef::Tag(format!("t{value}"))),
            2 => BaseRef::Image(ImageRef::Key(format!(
                "{:032x}",
                0xabc_u128 + value.len() as u128
            ))),
            3 => BaseRef::Docker(value),
            _ => BaseRef::Tar(format!("/images/{value}.tar").into()),
        };
        StepKind::From(base)
    });
    let body = prop_oneof![
        (word_strategy(1, 6), value_strategy(0, 8))
            .prop_map(|(key, value)| StepKind::Env { key, value }),
        word_strategy(1, 8).prop_map(|path| StepKind::Workdir(format!("/{path}"))),
        word_strategy(1, 8).prop_map(StepKind::User),
        (
            word_strategy(1, 8),
            word_strategy(1, 8),
            0_u16..0o10000_u16,
            any::<bool>(),
        )
            .prop_map(|(source, destination, mode, has_mode)| StepKind::Add {
                source,
                destination: format!("/{destination}"),
                mode: has_mode.then_some(u32::from(mode)),
            }),
        (word_strategy(1, 8), word_strategy(1, 8)).prop_map(|(host, file)| StepKind::Fetch {
            url: format!("https://{host}.example/{file}"),
            sha256: "0123456789abcdef".repeat(4),
            destination: format!("/{file}"),
        }),
        (words_strategy(1, 3), any::<bool>(), word_strategy(1, 12)).prop_map(
            |(shell, host, script)| StepKind::Run {
                shell,
                network: if host {
                    NetworkMode::Host
                } else {
                    NetworkMode::None
                },
                script,
            }
        ),
    ];
    (
        from,
        proptest::collection::vec(body, 0..5),
        proptest::option::of(proptest::collection::vec(word_strategy(1, 6), 1..3)),
        proptest::option::of(word_strategy(1, 6)),
    )
        .prop_map(|(from, body, cmd, kernel)| {
            let mut kinds = vec![from];
            kinds.extend(body);
            if let Some(cmd) = cmd {
                kinds.push(StepKind::Cmd(cmd));
            }
            if let Some(kernel) = kernel {
                kinds.push(StepKind::Kernel(kernel));
            }
            let steps = kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| Step {
                    line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    kind,
                })
                .collect();
            Sandboxfile { steps }
        })
}

/// One word of the safe alphabet.
fn word_strategy(min: usize, max: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(
        proptest::sample::select(&['a', 'b', 'c', '0', '1', '_', '-', '.', ':']),
        min..max,
    )
    .prop_map(|characters| characters.into_iter().collect())
}

/// A value that may hold spaces, quotes and a backslash.
fn value_strategy(min: usize, max: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(
        proptest::sample::select(&['a', 'b', '0', ' ', '"', '\\', '#', '/', '=']),
        min..max,
    )
    .prop_map(|characters| characters.into_iter().collect())
}

/// A shell argument vector.
fn words_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(word_strategy(1, 6), min..max)
}
