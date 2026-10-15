use std::ops::RangeInclusive;

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    BinOp, Block, ExprBinary, ExprForLoop, ExprIf, ExprMatch, ExprWhile, ImplItemFn, Item, ItemFn, ItemImpl, ItemTrait,
    Local, Signature, TraitItemFn, Type,
};

pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) lines: RangeInclusive<u32>,
    pub(crate) nested: Vec<RangeInclusive<u32>>,
    pub(crate) cyclomatic: u32,
}

impl Function {
    pub(crate) fn owns(&self, line: u32) -> bool {
        self.lines.contains(&line) && !self.nested.iter().any(|nested| nested.contains(&line))
    }
}

pub(crate) fn functions(source: &str) -> syn::Result<Vec<Function>> {
    let file = syn::parse_file(source)?;
    let mut finder = Finder::default();
    finder.visit_file(&file);
    Ok(finder.found)
}

fn line(span: Span) -> u32 {
    u32::try_from(span.start().line).unwrap_or(u32::MAX)
}

fn last_line(span: Span) -> u32 {
    u32::try_from(span.end().line).unwrap_or(u32::MAX)
}

#[derive(Default)]
struct Finder {
    owner: Option<String>,
    found: Vec<Function>,
}

impl Finder {
    fn function(&mut self, signature: &Signature, block: &Block) {
        let name = match &self.owner {
            Some(owner) => format!("{owner}::{}", signature.ident),
            None => signature.ident.to_string(),
        };
        let mut branches = Branches { count: 1, nested: Vec::new() };
        branches.visit_block(block);
        self.found.push(Function {
            name,
            lines: line(signature.fn_token.span)..=last_line(block.brace_token.span.close()),
            nested: branches.nested,
            cyclomatic: branches.count,
        });
        let owner = self.owner.take();
        self.visit_block(block);
        self.owner = owner;
    }

    fn owned(&mut self, owner: String, visit: impl FnOnce(&mut Self)) {
        let outer = self.owner.replace(owner);
        visit(self);
        self.owner = outer;
    }
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(path) => {
            path.path.segments.last().map_or_else(|| "_".to_string(), |segment| segment.ident.to_string())
        }
        _ => "_".to_string(),
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.function(&item.sig, &item.block);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.function(&item.sig, &item.block);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast TraitItemFn) {
        if let Some(block) = &item.default {
            self.function(&item.sig, block);
        }
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        self.owned(type_name(&item.self_ty), |finder| visit::visit_item_impl(finder, item));
    }

    fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
        self.owned(item.ident.to_string(), |finder| visit::visit_item_trait(finder, item));
    }
}

struct Branches {
    count: u32,
    nested: Vec<RangeInclusive<u32>>,
}

impl<'ast> Visit<'ast> for Branches {
    fn visit_item(&mut self, item: &'ast Item) {
        let span = item.span();
        self.nested.push(line(span)..=last_line(span));
    }

    fn visit_expr_if(&mut self, expr: &'ast ExprIf) {
        self.count += 1;
        visit::visit_expr_if(self, expr);
    }

    fn visit_expr_while(&mut self, expr: &'ast ExprWhile) {
        self.count += 1;
        visit::visit_expr_while(self, expr);
    }

    fn visit_expr_for_loop(&mut self, expr: &'ast ExprForLoop) {
        self.count += 1;
        visit::visit_expr_for_loop(self, expr);
    }

    fn visit_expr_match(&mut self, expr: &'ast ExprMatch) {
        let guards = expr.arms.iter().filter(|arm| arm.guard.is_some()).count();
        let paths = expr.arms.len().saturating_sub(1) + guards;
        self.count += u32::try_from(paths).unwrap_or(u32::MAX);
        visit::visit_expr_match(self, expr);
    }

    fn visit_expr_binary(&mut self, expr: &'ast ExprBinary) {
        if matches!(expr.op, BinOp::And(_) | BinOp::Or(_)) {
            self.count += 1;
        }
        visit::visit_expr_binary(self, expr);
    }

    fn visit_local(&mut self, local: &'ast Local) {
        if local.init.as_ref().is_some_and(|init| init.diverge.is_some()) {
            self.count += 1;
        }
        visit::visit_local(self, local);
    }
}
