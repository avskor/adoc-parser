use crate::event::CowStr;

pub struct Parser<'a> {
    _input: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { _input: input }
    }
}
