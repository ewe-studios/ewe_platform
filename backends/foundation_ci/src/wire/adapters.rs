//! Preserve the public language bindings across schema compiler versions.
use foundation_tasks::{TaskError, TaskResult};
use regex::Regex;
use std::{fs, path::Path};

pub(super) fn rust(root: &Path) -> TaskResult {
    let wildcard = Regex::new(r"(?m)^( +)_ => Ok\(\(\)\),$").map_err(TaskError::operation)?;
    let mut files = super::files(root)?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "rs"));
    let mut total = 0;
    for file in files {
        let text = fs::read_to_string(&file).map_err(TaskError::operation)?;
        let count = text.matches(".visit_union::<").count();
        if count == 0 {
            continue;
        }
        if wildcard.find_iter(&text).count() != count {
            return Err(TaskError::operation(format!(
                "Unexpected union verifier in {}",
                file.display()
            )));
        }
        let changed =
            wildcard.replace_all(&text, r"${1}_ => crate::read::verify_opaque_union(v, pos),");
        fs::write(file, changed.as_bytes()).map_err(TaskError::operation)?;
        total += count;
    }
    if total == 0 {
        Err(TaskError::operation("No generated union verifiers"))
    } else {
        Ok(())
    }
}

pub(super) fn typescript(root: &Path) -> TaskResult {
    let path = root.join("ewe/ui/action-input.ts");
    let mut text = fs::read_to_string(&path).map_err(TaskError::operation)?;
    for (old, new, count) in [
        ("public json: (number)[] = []", "public json: number[]|null = null",1),
        ("this.bb!.createScalarList<number>(this.json.bind(this), this.jsonLength())",
         "this.jsonArray() === null ? null : Array.from(this.jsonArray()!)",2),
        ("const json = ActionInput.createJsonVector(builder, this.json);",
         "const json = this.json === null ? 0 : ActionInput.createJsonVector(builder, this.json);",1),
    ] {
        if text.matches(old).count()!=count {return Err(TaskError::operation("Unexpected generated TypeScript action input"));}
        text=text.replace(old,new);
    }
    fs::write(path, text).map_err(TaskError::operation)
}

pub(super) fn swift(root: &Path, vocabulary: &Path) -> TaskResult {
    let path = root.join("ops_generated.swift");
    let mut text = fs::read_to_string(&path).map_err(TaskError::operation)?;
    for kind in ["BehaviourId", "MobileBehaviourId"] {
        let pattern = Regex::new(&format!(
            r"  public var behaviour: ewe_ui_{kind} \{{[^\n]+\}}"
        ))
        .map_err(TaskError::operation)?;
        let item = pattern
            .find(&text)
            .ok_or_else(|| TaskError::operation(format!("Missing Swift accessor for {kind}")))?;
        let raw = "_accessor.readBuffer(of: UInt16.self, at: o)";
        if !item.as_str().contains(raw) {
            return Err(TaskError::operation(format!(
                "Unexpected Swift accessor for {kind}"
            )));
        }
        text.insert_str(item.end(), &format!("\n  public var rawBehaviour: UInt16 {{ let o = _accessor.offset(VTOFFSET.behaviour.v); return o == 0 ? 0 : {raw} }}"));
    }
    fs::write(path, text).map_err(TaskError::operation)?;
    let frame = root.join("frame_generated.swift");
    let contents = fs::read_to_string(&frame).map_err(TaskError::operation)?;
    let id = Regex::new(r#"(?m)(public static var id: String \{ "EWF1" \}) +$"#)
        .map_err(TaskError::operation)?;
    fs::write(frame, id.replace_all(&contents, "$1").as_bytes()).map_err(TaskError::operation)?;
    let contents = fs::read_to_string(vocabulary).map_err(TaskError::operation)?;
    let version = Regex::new(r"(?m)^pub const VOCABULARY_VERSION: u32 = ([0-9]+);$")
        .map_err(TaskError::operation)?;
    let value = version
        .captures(&contents)
        .ok_or_else(|| TaskError::operation("Missing wire vocabulary version"))?;
    fs::write(root.join("vocabulary_generated.swift"),format!("// Generated from foundation_ui_wire/src/vocab/version.rs.\npublic let eweWireVocabularyVersion: UInt32 = {}\n",&value[1])).map_err(TaskError::operation)
}

pub(super) fn kotlin(root: &Path) -> TaskResult {
    let hello = &[("subscriptionResume", "false"), ("requestTag", "0uL")][..];
    for (table, defaults) in [
        ("ClientHello", hello),
        ("ServerCaps", hello),
        ("Control", &[("replayExpiredOffset", "0")]),
        ("FrameMessage", &[("subscriptionId", "0uL")]),
        ("FrameRequest", &[("resumeOffset", "0")]),
        (
            "LiveMount",
            &[
                ("sharing", "ConnectionSharing.TransportDefault"),
                ("keepAlive", "false"),
            ],
        ),
    ] {
        let path = root.join("ewe/ui").join(format!("{table}.kt"));
        let text = fs::read_to_string(&path).map_err(TaskError::operation)?;
        let pattern = Regex::new(&format!(
            r"(?m)^        fun create{table}\(([^)]+)\) : Int \{{"
        ))
        .map_err(TaskError::operation)?;
        let matched = pattern
            .captures(&text)
            .ok_or_else(|| TaskError::operation(format!("Missing Kotlin builder for {table}")))?;
        let fields: Vec<_> = matched[1].split(", ").collect();
        let names: Vec<_> = fields
            .iter()
            .map(|f| f.split(':').next().unwrap_or(f))
            .collect();
        let tail: Vec<_> = defaults.iter().map(|d| d.0).collect();
        if !names.ends_with(&tail) {
            return Err(TaskError::operation(format!(
                "Unexpected Kotlin builder fields for {table}"
            )));
        }
        let split = fields.len() - defaults.len();
        let args = names[..split]
            .iter()
            .copied()
            .chain(defaults.iter().map(|d| d.1))
            .collect::<Vec<_>>()
            .join(", ");
        let overload = format!(
            "        fun create{table}({}): Int = create{table}({args})\n",
            fields[..split].join(", ")
        );
        let index = matched
            .get(0)
            .ok_or_else(|| TaskError::operation("Missing Kotlin builder"))?
            .start();
        let mut updated = text;
        updated.insert_str(index, &overload);
        fs::write(path, updated).map_err(TaskError::operation)?;
    }
    Ok(())
}
