use std::collections::BTreeSet;

use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{ExprStruct, Fields, FnArg, ImplItemFn, ItemFn, ItemImpl, ItemStruct, Signature, Type, TypePath};

pub(crate) struct Shape {
    pub(crate) name: String,
    pub(crate) line: usize,
}

#[derive(Default)]
pub(crate) struct Bags {
    pub(crate) shapes: Vec<Shape>,
    pub(crate) built: Vec<String>,
    pub(crate) taken: Vec<String>,
}

pub(crate) fn bags(source: &str) -> syn::Result<Bags> {
    let mut finder = Finder::default();
    finder.visit_file(&syn::parse_file(source)?);
    Ok(finder.bags)
}

#[derive(Default)]
struct Finder {
    owner: Option<String>,
    bags: Bags,
}

impl Finder {
    fn takes(&mut self, signature: &Signature) {
        let mut named = Named::default();
        for input in &signature.inputs {
            match input {
                FnArg::Typed(typed) => named.visit_type(&typed.ty),
                FnArg::Receiver(_) => named.types.extend(self.owner.clone()),
            }
        }
        self.bags.taken.extend(named.types);
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
        let serialized = item.attrs.iter().any(|attr| {
            let tokens = attr.to_token_stream().to_string();
            tokens.contains("Serialize") || tokens.contains("Deserialize")
        });
        if matches!(item.fields, Fields::Named(_)) && !serialized {
            self.bags.shapes.push(Shape { name: item.ident.to_string(), line: item.ident.span().start().line });
        }
        visit::visit_item_struct(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let owner = match &*item.self_ty {
            Type::Path(path) => path.path.segments.last().map(|segment| segment.ident.to_string()),
            _ => None,
        };
        let outer = std::mem::replace(&mut self.owner, owner);
        visit::visit_item_impl(self, item);
        self.owner = outer;
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.takes(&item.sig);
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.takes(&item.sig);
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_expr_struct(&mut self, expression: &'ast ExprStruct) {
        let name = expression.path.segments.last().map(|segment| segment.ident.to_string());
        self.bags.built.extend(name.map(|name| if name == "Self" { self.owner.clone().unwrap_or(name) } else { name }));
        visit::visit_expr_struct(self, expression);
    }
}

#[derive(Default)]
struct Named {
    types: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for Named {
    fn visit_type_path(&mut self, path: &'ast TypePath) {
        self.types.extend(path.path.segments.last().map(|segment| segment.ident.to_string()));
        visit::visit_type_path(self, path);
    }
}
