use std::collections::BTreeMap;

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, ExprMethodCall, ExprPath, ItemFn, Lit};

#[derive(Default)]
pub(crate) struct StepCode {
    pub(crate) thens: Vec<(String, String)>,
    pub(crate) asserting: BTreeMap<String, bool>,
    pub(crate) marks_wip: bool,
}

impl StepCode {
    pub(crate) fn read(&mut self, source: &str) -> syn::Result<()> {
        let file = syn::parse_file(source)?;
        self.visit_file(&file);
        Ok(())
    }
}

fn literal(expr: Option<&Expr>) -> Option<String> {
    match expr {
        Some(Expr::Lit(ExprLit { lit: Lit::Str(text), .. })) => Some(text.value()),
        _ => None,
    }
}

fn path_name(expr: Option<&Expr>) -> Option<String> {
    match expr {
        Some(Expr::Path(ExprPath { path, .. })) => path.segments.last().map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn asserts(tokens: TokenStream) -> bool {
    let mut previous: Option<String> = None;
    tokens.into_iter().any(|tree| match tree {
        TokenTree::Group(group) => asserts(group.stream()),
        TokenTree::Punct(punct) => {
            let bang = punct.as_char() == '!' && previous.as_deref().is_some_and(|name| name.starts_with("assert"));
            previous = None;
            bang
        }
        TokenTree::Ident(ident) => {
            previous = Some(ident.to_string());
            false
        }
        TokenTree::Literal(_) => false,
    })
}

impl<'ast> Visit<'ast> for StepCode {
    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let method = call.method.to_string();
        self.marks_wip |= method == "wip" || method == "wip_scenarios";
        if method == "then"
            && let (Some(pattern), Some(step)) = (literal(call.args.first()), path_name(call.args.get(1)))
        {
            self.thens.push((pattern, step));
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.asserting.insert(item.sig.ident.to_string(), asserts(item.block.to_token_stream()));
        visit::visit_item_fn(self, item);
    }
}
