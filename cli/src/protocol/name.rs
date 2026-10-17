use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize, Serializer};

const LONGEST: usize = 24;

pub(crate) const NAME_RULE: &str = "1 to 24 characters of a-z, 0-9 and -, starting with a letter or digit";

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(try_from = "String")]
pub(crate) struct Name(String);

fn follows_the_rule(text: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    let starts_well = text.chars().next().is_some_and(allowed);
    let fits = text.len() <= LONGEST && text.chars().all(|c| allowed(c) || c == '-');
    if starts_well && fits {
        Ok(())
    } else {
        Err(format!("{text:?} is not a name: a name is {NAME_RULE}, such as repost-forward"))
    }
}

impl FromStr for Name {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        follows_the_rule(text).map(|()| Name(text.to_string()))
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
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
        follows_the_rule(&text).map(|()| Name(text))
    }
}
