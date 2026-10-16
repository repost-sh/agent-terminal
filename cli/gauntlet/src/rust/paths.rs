use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{Item, ItemMod, ItemUse, Macro, Path, UseTree, Visibility};

pub(crate) fn crate_paths(source: &str, module: &[String]) -> syn::Result<Vec<Vec<String>>> {
    let file = syn::parse_file(source)?;
    let mut paths = Paths { scope: Scope::of(module.to_vec(), &file.items), found: Vec::new() };
    paths.visit_file(&file);
    paths.found.sort();
    paths.found.dedup();
    Ok(paths.found)
}

struct Scope {
    module: Vec<String>,
    children: Vec<String>,
}

impl Scope {
    fn of(module: Vec<String>, items: &[Item]) -> Self {
        let children = items
            .iter()
            .filter_map(|item| match item {
                Item::Mod(child) => Some(child.ident.to_string()),
                _ => None,
            })
            .collect();
        Self { module, children }
    }

    fn absolute(&self, segments: &[String]) -> Option<Vec<String>> {
        let (mut module, rest) = match segments {
            [first, rest @ ..] if first == "crate" => (Vec::new(), rest),
            [first, rest @ ..] if first == "self" => (self.module.clone(), rest),
            [first, _, ..] if self.children.contains(first) => (self.module.clone(), segments),
            [first, ..] if first == "super" => self.climbed(segments),
            _ => return None,
        };
        module.extend(rest.iter().cloned());
        Some(module)
    }

    fn climbed<'s>(&self, segments: &'s [String]) -> (Vec<String>, &'s [String]) {
        let climbs = segments.iter().take_while(|segment| *segment == "super").count();
        let kept = self.module.len().saturating_sub(climbs);
        (self.module.iter().take(kept).cloned().collect(), segments.get(climbs..).unwrap_or_default())
    }
}

struct Paths {
    scope: Scope,
    found: Vec<Vec<String>>,
}

impl Paths {
    fn resolve(&mut self, segments: &[String]) {
        self.found.extend(self.scope.absolute(segments));
    }
}

impl<'ast> Visit<'ast> for Paths {
    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        let Some((_, items)) = &item.content else {
            return;
        };
        let mut module = self.scope.module.clone();
        module.push(item.ident.to_string());
        let outer = std::mem::replace(&mut self.scope, Scope::of(module, items));
        visit::visit_item_mod(self, item);
        self.scope = outer;
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let mut found = Vec::new();
        flatten(&item.tree, &mut Vec::new(), &mut found);
        for segments in found {
            self.resolve(&segments);
        }
    }

    fn visit_path(&mut self, path: &'ast Path) {
        let segments: Vec<String> = path.segments.iter().map(|segment| segment.ident.to_string()).collect();
        self.resolve(&segments);
        visit::visit_path(self, path);
    }

    fn visit_visibility(&mut self, _: &'ast Visibility) {}

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.visit_path(&mac.path);
        for segments in token_paths(mac.tokens.clone()) {
            self.resolve(&segments);
        }
    }
}

fn flatten(tree: &UseTree, prefix: &mut Vec<String>, found: &mut Vec<Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten(&path.tree, prefix, found);
            prefix.pop();
        }
        UseTree::Name(name) => found.push(prefix.iter().cloned().chain([name.ident.to_string()]).collect()),
        UseTree::Rename(rename) => found.push(prefix.iter().cloned().chain([rename.ident.to_string()]).collect()),
        UseTree::Glob(_) => found.push(prefix.clone()),
        UseTree::Group(group) => group.items.iter().for_each(|tree| flatten(tree, prefix, found)),
    }
}

fn token_paths(tokens: TokenStream) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut colons = 0;
    for tree in tokens {
        match tree {
            TokenTree::Ident(ident) if colons == 2 => {
                current.push(ident.to_string());
                colons = 0;
            }
            TokenTree::Ident(ident) => {
                found.extend(finished(std::mem::replace(&mut current, vec![ident.to_string()])));
                colons = 0;
            }
            TokenTree::Punct(punct) if punct.as_char() == ':' => colons += 1,
            TokenTree::Group(group) => {
                found.extend(finished(std::mem::take(&mut current)));
                found.extend(token_paths(group.stream()));
                colons = 0;
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {
                found.extend(finished(std::mem::take(&mut current)));
                colons = 0;
            }
        }
    }
    found.extend(finished(current));
    found
}

fn finished(path: Vec<String>) -> Option<Vec<String>> {
    (path.len() > 1).then_some(path)
}
