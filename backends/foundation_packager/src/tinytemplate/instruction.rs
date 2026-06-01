use std::ops::Deref;

#[derive(Eq, PartialEq, Debug, Clone)]
pub(crate) enum PathStep<'template> {
    Name(&'template str),
    Index(&'template str, usize),
}
impl<'template> Deref for PathStep<'template> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            PathStep::Name(s) => s,
            PathStep::Index(s, _) => s,
        }
    }
}

pub(crate) type Path<'template> = Vec<PathStep<'template>>;
pub(crate) type PathSlice<'a, 'template> = &'a [PathStep<'template>];

#[derive(Eq, PartialEq, Debug, Clone)]
pub(crate) enum Instruction<'template> {
    Literal(&'template str),
    Value(Path<'template>),
    FormattedValue(Path<'template>, &'template str),
    Branch(Path<'template>, bool, usize),
    PushNamedContext(Path<'template>, &'template str),
    PushIterationContext(Path<'template>, &'template str),
    PopContext,
    Iterate(usize),
    Goto(usize),
    Call(&'template str, Path<'template>),
}

pub(crate) fn path_to_str(path: PathSlice) -> String {
    let mut path_str = "".to_string();
    for (i, step) in path.iter().enumerate() {
        if i > 0 {
            path_str.push('.');
        }
        path_str.push_str(step);
    }
    path_str
}
