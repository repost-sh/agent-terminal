use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const LONGEST: usize = 24;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct Name(String);

impl FromStr for Name {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
        let starts_well = text.chars().next().is_some_and(allowed);
        let fits = text.len() <= LONGEST && text.chars().all(|c| allowed(c) || c == '-');
        if starts_well && fits {
            Ok(Name(text.to_string()))
        } else {
            Err(format!(
                "{text:?} is not a name: a name is 1 to {LONGEST} characters of a-z, 0-9 and -, starting with a letter or digit, such as repost-forward"
            ))
        }
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Name {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl From<Name> for String {
    fn from(name: Name) -> String {
        name.0
    }
}
