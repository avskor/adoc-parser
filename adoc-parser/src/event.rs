use std::borrow::Cow;

pub type CowStr<'a> = Cow<'a, str>;
