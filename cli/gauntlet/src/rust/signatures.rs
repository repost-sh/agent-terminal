use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{FnArg, ImplItemFn, ItemFn, ItemImpl, ItemTrait, Signature, TraitItem, Type, Visibility};

pub(crate) struct Parameters {
    pub(crate) function: String,
    pub(crate) line: usize,
    pub(crate) count: usize,
}

pub(crate) fn shared_functions(source: &str) -> syn::Result<Vec<Parameters>> {
    let file = syn::parse_file(source)?;
    let mut finder = Finder::default();
    finder.visit_file(&file);
    Ok(finder.found)
}

#[derive(Default)]
struct Finder {
    owner: Option<String>,
    found: Vec<Parameters>,
}

impl Finder {
    fn shared(&mut self, visibility: &Visibility, signature: &Signature) {
        if matches!(visibility, Visibility::Inherited) {
            return;
        }
        let function = match &self.owner {
            Some(owner) => format!("{owner}::{}", signature.ident),
            None => signature.ident.to_string(),
        };
        let count = signature.inputs.iter().filter(|input| matches!(input, FnArg::Typed(_))).count();
        self.found.push(Parameters { function, line: signature.ident.span().start().line, count });
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.shared(&item.vis, &item.sig);
        visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let owner = self.owner.replace(type_name(&item.self_ty));
        visit::visit_item_impl(self, item);
        self.owner = owner;
    }

    fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
        let owner = self.owner.replace(item.ident.to_string());
        for function in item.items.iter().filter_map(|member| match member {
            TraitItem::Fn(function) => Some(function),
            _ => None,
        }) {
            self.shared(&item.vis, &function.sig);
        }
        visit::visit_item_trait(self, item);
        self.owner = owner;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.shared(&item.vis, &item.sig);
        visit::visit_impl_item_fn(self, item);
    }
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(path) => path.path.segments.last().map_or_else(String::new, |segment| segment.ident.to_string()),
        other => other.to_token_stream().to_string(),
    }
}
