#![allow(deprecated)]

use super::error::Error::*;
use super::error::{get_offset, Error, Result};
use super::instruction::{Instruction, Path, PathStep};

const UNKNOWN: usize = ::std::usize::MAX;

enum Block {
    Branch(usize),
    For(usize),
    With,
}

static KNOWN_KEYWORDS: [&str; 4] = ["@index", "@first", "@last", "@root"];

pub(crate) struct TemplateCompiler<'template> {
    original_text: &'template str,
    remaining_text: &'template str,
    instructions: Vec<Instruction<'template>>,
    block_stack: Vec<(&'template str, Block)>,
    trim_next: bool,
}
impl<'template> TemplateCompiler<'template> {
    pub fn new(text: &'template str) -> TemplateCompiler<'template> {
        TemplateCompiler {
            original_text: text,
            remaining_text: text,
            instructions: vec![],
            block_stack: vec![],
            trim_next: false,
        }
    }

    pub fn compile(mut self) -> Result<Vec<Instruction<'template>>> {
        while !self.remaining_text.is_empty() {
            if self.remaining_text.starts_with("{#") {
                self.trim_next = false;
                let tag = self.consume_tag("#}")?;
                let comment = tag[2..(tag.len() - 2)].trim();
                if comment.starts_with('-') {
                    self.trim_last_whitespace();
                }
                if comment.ends_with('-') {
                    self.trim_next_whitespace();
                }
            } else if self.remaining_text.starts_with("{{") {
                self.trim_next = false;
                let (discriminant, rest) = self.consume_block()?;
                match discriminant {
                    "if" => {
                        let (path, negated) = if rest.starts_with("not") {
                            (self.parse_path(&rest[4..])?, true)
                        } else {
                            (self.parse_path(rest)?, false)
                        };
                        self.block_stack
                            .push((discriminant, Block::Branch(self.instructions.len())));
                        self.instructions
                            .push(Instruction::Branch(path, !negated, UNKNOWN));
                    }
                    "else" => {
                        self.expect_empty(rest)?;
                        let num_instructions = self.instructions.len() + 1;
                        self.close_branch(num_instructions, discriminant)?;
                        self.block_stack
                            .push((discriminant, Block::Branch(self.instructions.len())));
                        self.instructions.push(Instruction::Goto(UNKNOWN))
                    }
                    "endif" => {
                        self.expect_empty(rest)?;
                        let num_instructions = self.instructions.len();
                        self.close_branch(num_instructions, discriminant)?;
                    }
                    "with" => {
                        let (path, name) = self.parse_with(rest)?;
                        let instruction = Instruction::PushNamedContext(path, name);
                        self.instructions.push(instruction);
                        self.block_stack.push((discriminant, Block::With));
                    }
                    "endwith" => {
                        self.expect_empty(rest)?;
                        if let Some((_, Block::With)) = self.block_stack.pop() {
                            self.instructions.push(Instruction::PopContext)
                        } else {
                            return Err(self.parse_error(
                                discriminant,
                                "Found a closing endwith that doesn't match with a preceeding with.".to_string()
                            ));
                        }
                    }
                    "for" => {
                        let (path, name) = self.parse_for(rest)?;
                        self.instructions
                            .push(Instruction::PushIterationContext(path, name));
                        self.block_stack
                            .push((discriminant, Block::For(self.instructions.len())));
                        self.instructions.push(Instruction::Iterate(UNKNOWN));
                    }
                    "endfor" => {
                        self.expect_empty(rest)?;
                        let num_instructions = self.instructions.len() + 1;
                        let goto_target = self.close_for(num_instructions, discriminant)?;
                        self.instructions.push(Instruction::Goto(goto_target));
                        self.instructions.push(Instruction::PopContext);
                    }
                    "call" => {
                        let (name, path) = self.parse_call(rest)?;
                        self.instructions.push(Instruction::Call(name, path));
                    }
                    _ => {
                        return Err(self.parse_error(
                            discriminant,
                            format!("Unknown block type '{}'", discriminant),
                        ));
                    }
                }
            } else if self.remaining_text.starts_with('{') {
                self.trim_next = false;
                let (path, name) = self.consume_value()?;
                let instruction = match name {
                    Some(name) => Instruction::FormattedValue(path, name),
                    None => Instruction::Value(path),
                };
                self.instructions.push(instruction);
            } else {
                let mut escaped = false;
                loop {
                    let mut text = self.consume_text(escaped);
                    if self.trim_next {
                        text = text.trim_left();
                        self.trim_next = false;
                    }
                    escaped = text.ends_with('\\');
                    if escaped {
                        text = &text[..text.len() - 1];
                    }
                    self.instructions.push(Instruction::Literal(text));
                    if !escaped {
                        break;
                    }
                    if escaped && self.remaining_text.is_empty() {
                        return Err(self.parse_error(
                            text,
                            "Found an escape that doesn't escape any character.".to_string(),
                        ));
                    }
                }
            }
        }

        if let Some((text, _)) = self.block_stack.pop() {
            return Err(self.parse_error(
                text,
                "Expected block-closing tag, but reached the end of input.".to_string(),
            ));
        }

        Ok(self.instructions)
    }

    fn parse_path(&self, text: &'template str) -> Result<Path<'template>> {
        if !text.starts_with('@') {
            Ok(text
                .split('.')
                .map(|s| match s.parse::<usize>() {
                    Ok(n) => PathStep::Index(s, n),
                    Err(_) => PathStep::Name(s),
                })
                .collect::<Vec<_>>())
        } else if KNOWN_KEYWORDS.iter().any(|k| *k == text) {
            Ok(vec![PathStep::Name(text)])
        } else {
            Err(self.parse_error(text, format!("Invalid keyword name '{}'", text)))
        }
    }

    fn parse_error(&self, location: &str, msg: String) -> Error {
        let (line, column) = get_offset(self.original_text, location);
        ParseError { msg, line, column }
    }

    fn expect_empty(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            Ok(())
        } else {
            Err(self.parse_error(text, format!("Unexpected text '{}'", text)))
        }
    }

    fn close_branch(&mut self, new_target: usize, discriminant: &str) -> Result<()> {
        let branch_block = self.block_stack.pop();
        if let Some((_, Block::Branch(index))) = branch_block {
            match &mut self.instructions[index] {
                Instruction::Branch(_, _, target) => {
                    *target = new_target;
                    Ok(())
                }
                Instruction::Goto(target) => {
                    *target = new_target;
                    Ok(())
                }
                _ => panic!(),
            }
        } else {
            Err(self.parse_error(
                discriminant,
                "Found a closing endif or else which doesn't match with a preceding if."
                    .to_string(),
            ))
        }
    }

    fn close_for(&mut self, new_target: usize, discriminant: &str) -> Result<usize> {
        let branch_block = self.block_stack.pop();
        if let Some((_, Block::For(index))) = branch_block {
            match &mut self.instructions[index] {
                Instruction::Iterate(target) => {
                    *target = new_target;
                    Ok(index)
                }
                _ => panic!(),
            }
        } else {
            Err(self.parse_error(
                discriminant,
                "Found a closing endfor which doesn't match with a preceding for.".to_string(),
            ))
        }
    }

    fn consume_text(&mut self, escaped: bool) -> &'template str {
        let search_substr = if escaped {
            &self.remaining_text[1..]
        } else {
            self.remaining_text
        };
        let mut position = search_substr
            .find('{')
            .unwrap_or_else(|| search_substr.len());
        if escaped {
            position += 1;
        }
        let (text, remaining) = self.remaining_text.split_at(position);
        self.remaining_text = remaining;
        text
    }

    fn consume_value(&mut self) -> Result<(Path<'template>, Option<&'template str>)> {
        let tag = self.consume_tag("}")?;
        let mut tag = tag[1..(tag.len() - 1)].trim();
        if tag.starts_with('-') {
            tag = tag[1..].trim();
            self.trim_last_whitespace();
        }
        if tag.ends_with('-') {
            tag = tag[0..tag.len() - 1].trim();
            self.trim_next_whitespace();
        }
        if let Some(index) = tag.find('|') {
            let (path_str, name_str) = tag.split_at(index);
            let name = name_str[1..].trim();
            let path = self.parse_path(path_str.trim())?;
            Ok((path, Some(name)))
        } else {
            Ok((self.parse_path(tag)?, None))
        }
    }

    fn trim_last_whitespace(&mut self) {
        if let Some(Instruction::Literal(text)) = self.instructions.last_mut() {
            *text = text.trim_right();
        }
    }

    fn trim_next_whitespace(&mut self) {
        self.trim_next = true;
    }

    fn consume_block(&mut self) -> Result<(&'template str, &'template str)> {
        let tag = self.consume_tag("}}")?;
        let mut block = tag[2..(tag.len() - 2)].trim();
        if block.starts_with('-') {
            block = block[1..].trim();
            self.trim_last_whitespace();
        }
        if block.ends_with('-') {
            block = block[0..block.len() - 1].trim();
            self.trim_next_whitespace();
        }
        let discriminant = block.split_whitespace().next().unwrap_or(block);
        let rest = block[discriminant.len()..].trim();
        Ok((discriminant, rest))
    }

    fn consume_tag(&mut self, expected_close: &str) -> Result<&'template str> {
        let start_len = expected_close.len();
        let end_len = expected_close.len();
        if let Some(line) = self.remaining_text.lines().next() {
            if let Some(pos) = line[start_len..].find(expected_close) {
                let (tag, remaining) = self.remaining_text.split_at(pos + start_len + end_len);
                self.remaining_text = remaining;
                Ok(tag)
            } else {
                Err(self.parse_error(
                    line,
                    format!(
                        "Expected a closing '{}' but found end-of-line instead.",
                        expected_close
                    ),
                ))
            }
        } else {
            Err(self.parse_error(
                self.remaining_text,
                format!(
                    "Expected a closing '{}' but found end-of-text instead.",
                    expected_close
                ),
            ))
        }
    }

    fn parse_with(&self, with_text: &'template str) -> Result<(Path<'template>, &'template str)> {
        if let Some(index) = with_text.find(" as ") {
            let (path_str, name_str) = with_text.split_at(index);
            let path = self.parse_path(path_str.trim())?;
            let name = name_str[" as ".len()..].trim();
            Ok((path, name))
        } else {
            Err(self.parse_error(
                with_text,
                format!(
                    "Expected 'as <path>' in with block, but found \"{}\" instead",
                    with_text
                ),
            ))
        }
    }

    fn parse_for(&self, for_text: &'template str) -> Result<(Path<'template>, &'template str)> {
        if let Some(index) = for_text.find(" in ") {
            let (name_str, path_str) = for_text.split_at(index);
            let name = name_str.trim();
            let path = self.parse_path(path_str[" in ".len()..].trim())?;
            Ok((path, name))
        } else {
            Err(self.parse_error(
                for_text,
                format!("Unable to parse for block text '{}'", for_text),
            ))
        }
    }

    fn parse_call(&self, call_text: &'template str) -> Result<(&'template str, Path<'template>)> {
        if let Some(index) = call_text.find(" with ") {
            let (name_str, path_str) = call_text.split_at(index);
            let name = name_str.trim();
            let path = self.parse_path(path_str[" with ".len()..].trim())?;
            Ok((name, path))
        } else {
            Err(self.parse_error(
                call_text,
                format!("Unable to parse call block text '{}'", call_text),
            ))
        }
    }
}
