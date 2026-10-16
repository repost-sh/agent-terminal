use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Attribute, ItemFn, ItemMacro, ItemMod, Macro, Meta};

pub(crate) fn has_property_test(source: &str) -> syn::Result<bool> {
    let file = syn::parse_file(source)?;
    if file.attrs.iter().any(switches_off) {
        return Ok(false);
    }
    let mut finder = Finder::default();
    finder.visit_file(&file);
    Ok(finder.found)
}

fn switches_off(attribute: &Attribute) -> bool {
    attribute.path().is_ident("cfg")
        && !matches!(&attribute.meta, Meta::List(list) if list.tokens.to_string() == "test")
}

#[derive(Default)]
struct Finder {
    found: bool,
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        if !item.attrs.iter().any(switches_off) {
            visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        if !item.attrs.iter().any(switches_off) {
            visit::visit_item_fn(self, item);
        }
    }

    fn visit_item_macro(&mut self, item: &'ast ItemMacro) {
        if !item.attrs.iter().any(switches_off) {
            visit::visit_item_macro(self, item);
        }
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        let named = mac.path.segments.last().is_some_and(|segment| segment.ident == "proptest");
        self.found |= named && Seen::of(mac.tokens.clone()).is_a_property();
    }
}

#[derive(Default)]
struct Seen {
    test: bool,
    strategy: bool,
    assertion: bool,
}

impl Seen {
    fn of(tokens: TokenStream) -> Self {
        let mut seen = Self::default();
        seen.scan(tokens);
        seen
    }

    fn scan(&mut self, tokens: TokenStream) {
        for tree in tokens {
            match tree {
                TokenTree::Group(group) => {
                    self.test |= group.delimiter() == Delimiter::Bracket && group.stream().to_string() == "test";
                    self.scan(group.stream());
                }
                TokenTree::Ident(ident) => {
                    let name = ident.to_string();
                    self.strategy |= name == "in";
                    self.assertion |= name.starts_with("prop_assert") || name.starts_with("assert");
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {}
            }
        }
    }

    fn is_a_property(&self) -> bool {
        self.test && self.strategy && self.assertion
    }
}
