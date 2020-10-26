//! Assorted utility functions (missing batteries).

/// Shortcut for defining a lazily-compiled regular expression
macro_rules! _regex {
    ($regex_body:literal) => {{
        static RE: ::once_cell::sync::OnceCell<regex::Regex> = ::once_cell::sync::OnceCell::new();
        RE.get_or_init(|| ::regex::Regex::new($regex_body).unwrap())
    }};
}

pub(crate) use _regex as regex;
