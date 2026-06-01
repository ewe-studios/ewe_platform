use super::compiler::TemplateCompiler;
use super::error::Error::*;
use super::error::*;
use super::instruction::{Instruction, PathSlice, PathStep};
use crate::tinytemplate::ValueFormatter;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write;
use std::slice;

enum ContextElement<'render, 'template> {
    Object(&'render Value),
    Named(&'template str, &'render Value),
    Iteration(
        &'template str,
        &'render Value,
        usize,
        usize,
        slice::Iter<'render, Value>,
    ),
}

struct RenderContext<'render, 'template> {
    original_text: &'template str,
    context_stack: Vec<ContextElement<'render, 'template>>,
}
impl<'render, 'template> RenderContext<'render, 'template> {
    fn lookup(&self, path: PathSlice) -> Result<&'render Value> {
        for stack_layer in self.context_stack.iter().rev() {
            match stack_layer {
                ContextElement::Object(obj) => return self.lookup_in(path, obj),
                ContextElement::Named(name, obj) => {
                    if *name == &*path[0] {
                        return self.lookup_in(&path[1..], obj);
                    }
                }
                ContextElement::Iteration(name, obj, _, _, _) => {
                    if *name == &*path[0] {
                        return self.lookup_in(&path[1..], obj);
                    }
                }
            }
        }
        panic!("Attempted to do a lookup with an empty context stack.")
    }

    fn lookup_in(&self, path: PathSlice, object: &'render Value) -> Result<&'render Value> {
        let mut current = object;
        for step in path.iter() {
            if let PathStep::Index(_, n) = step {
                if let Some(next) = current.get(n) {
                    current = next;
                    continue;
                }
            }

            if let PathStep::Name("@root") = step {
                return Ok(current);
            }

            let step: &str = &*step;

            match current.get(step) {
                Some(next) => current = next,
                None => return Err(lookup_error(self.original_text, step, path, current)),
            }
        }
        Ok(current)
    }

    fn lookup_index(&self) -> Result<(usize, usize)> {
        for stack_layer in self.context_stack.iter().rev() {
            match stack_layer {
                ContextElement::Iteration(_, _, index, length, _) => return Ok((*index, *length)),
                _ => continue,
            }
        }
        Err(GenericError {
            msg: "Used @index outside of a foreach block.".to_string(),
        })
    }

    fn lookup_root(&self) -> Result<&'render Value> {
        match self.context_stack.get(0) {
            Some(ContextElement::Object(obj)) => Ok(obj),
            Some(_) => {
                panic!("Expected Object value at root of context stack.")
            }
            None => panic!("Attempted to do a lookup with an empty context stack."),
        }
    }
}

pub(crate) struct Template<'template> {
    original_text: &'template str,
    instructions: Vec<Instruction<'template>>,
    template_len: usize,
}
impl<'template> Template<'template> {
    pub fn compile(text: &'template str) -> Result<Template<'template>> {
        Ok(Template {
            original_text: text,
            template_len: text.len(),
            instructions: TemplateCompiler::new(text).compile()?,
        })
    }

    pub fn render(
        &self,
        context: &Value,
        template_registry: &HashMap<&str, Template>,
        formatter_registry: &HashMap<&str, Box<ValueFormatter>>,
        default_formatter: &ValueFormatter,
    ) -> Result<String> {
        let mut output = String::with_capacity(self.template_len);
        self.render_into(
            context,
            template_registry,
            formatter_registry,
            default_formatter,
            &mut output,
        )?;
        Ok(output)
    }

    pub fn render_into(
        &self,
        context: &Value,
        template_registry: &HashMap<&str, Template>,
        formatter_registry: &HashMap<&str, Box<ValueFormatter>>,
        default_formatter: &ValueFormatter,
        output: &mut String,
    ) -> Result<()> {
        let mut program_counter = 0;
        let mut render_context = RenderContext {
            original_text: self.original_text,
            context_stack: vec![ContextElement::Object(context)],
        };

        while program_counter < self.instructions.len() {
            match &self.instructions[program_counter] {
                Instruction::Literal(text) => {
                    output.push_str(text);
                    program_counter += 1;
                }
                Instruction::Value(path) => {
                    let first = path.first().unwrap();
                    if first.starts_with('@') {
                        let first: &str = &*first;
                        match first {
                            "@index" => {
                                write!(output, "{}", render_context.lookup_index()?.0).unwrap()
                            }
                            "@first" => {
                                write!(output, "{}", render_context.lookup_index()?.0 == 0).unwrap()
                            }
                            "@last" => {
                                let (index, length) = render_context.lookup_index()?;
                                write!(output, "{}", index == length - 1).unwrap()
                            }
                            "@root" => {
                                let value_to_render = render_context.lookup_root()?;
                                default_formatter(value_to_render, output)?;
                            }
                            _ => panic!(),
                        }
                    } else {
                        let value_to_render = render_context.lookup(path)?;
                        default_formatter(value_to_render, output)?;
                    }
                    program_counter += 1;
                }
                Instruction::FormattedValue(path, name) => {
                    let value_to_render = render_context.lookup(path)?;
                    match formatter_registry.get(name) {
                        Some(formatter) => {
                            let formatter_result = formatter(value_to_render, output);
                            if let Err(err) = formatter_result {
                                return Err(called_formatter_error(self.original_text, name, err));
                            }
                        }
                        None => return Err(unknown_formatter(self.original_text, name)),
                    }
                    program_counter += 1;
                }
                Instruction::Branch(path, negate, target) => {
                    let first = path.first().unwrap();
                    let mut truthy = if first.starts_with('@') {
                        let first: &str = &*first;
                        match &*first {
                            "@index" => render_context.lookup_index()?.0 != 0,
                            "@first" => render_context.lookup_index()?.0 == 0,
                            "@last" => {
                                let (index, length) = render_context.lookup_index()?;
                                index == (length - 1)
                            }
                            "@root" => self.value_is_truthy(render_context.lookup_root()?, path)?,
                            other => panic!("Unknown keyword {}", other),
                        }
                    } else {
                        let value_to_render = render_context.lookup(path)?;
                        self.value_is_truthy(value_to_render, path)?
                    };
                    if *negate {
                        truthy = !truthy;
                    }

                    if truthy {
                        program_counter = *target;
                    } else {
                        program_counter += 1;
                    }
                }
                Instruction::PushNamedContext(path, name) => {
                    let context_value = render_context.lookup(path)?;
                    render_context
                        .context_stack
                        .push(ContextElement::Named(name, context_value));
                    program_counter += 1;
                }
                Instruction::PushIterationContext(path, name) => {
                    let first = path.first().unwrap();
                    let context_value = match first {
                        PathStep::Name("@root") => render_context.lookup_root()?,
                        PathStep::Name(other) if other.starts_with('@') => {
                            return Err(not_iterable_error(self.original_text, path))
                        }
                        _ => render_context.lookup(path)?,
                    };
                    match context_value {
                        Value::Array(ref arr) => {
                            render_context.context_stack.push(ContextElement::Iteration(
                                name,
                                &Value::Null,
                                ::std::usize::MAX,
                                arr.len(),
                                arr.iter(),
                            ))
                        }
                        _ => return Err(not_iterable_error(self.original_text, path)),
                    };
                    program_counter += 1;
                }
                Instruction::PopContext => {
                    render_context.context_stack.pop();
                    program_counter += 1;
                }
                Instruction::Goto(target) => {
                    program_counter = *target;
                }
                Instruction::Iterate(target) => {
                    match render_context.context_stack.last_mut() {
                        Some(ContextElement::Iteration(_, val, index, _, iter)) => {
                            match iter.next() {
                                Some(new_val) => {
                                    *val = new_val;
                                    *index = index.wrapping_add(1);
                                    program_counter += 1;
                                }
                                None => {
                                    program_counter = *target;
                                }
                            }
                        }
                        _ => panic!("Malformed program."),
                    };
                }
                Instruction::Call(template_name, path) => {
                    let context_value = render_context.lookup(path)?;
                    match template_registry.get(template_name) {
                        Some(templ) => {
                            let called_templ_result = templ.render_into(
                                context_value,
                                template_registry,
                                formatter_registry,
                                default_formatter,
                                output,
                            );
                            if let Err(err) = called_templ_result {
                                return Err(called_template_error(
                                    self.original_text,
                                    template_name,
                                    err,
                                ));
                            }
                        }
                        None => return Err(unknown_template(self.original_text, template_name)),
                    }
                    program_counter += 1;
                }
            }
        }
        Ok(())
    }

    fn value_is_truthy(&self, value: &Value, path: PathSlice) -> Result<bool> {
        let truthy = match value {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => match n.as_f64() {
                Some(float) => float != 0.0,
                None => {
                    return Err(truthiness_error(self.original_text, path));
                }
            },
            Value::String(s) => !s.is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(_) => true,
        };
        Ok(truthy)
    }
}
