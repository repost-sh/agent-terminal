use syn::visit::{self, Visit};
use syn::{
    Block, ExprForLoop, ExprIf, ExprLoop, ExprMatch, ExprMethodCall, ExprPath, ExprTry, ExprWhile, ImplItemFn, ItemFn,
    ItemImpl, Signature, Stmt, Visibility,
};

pub(crate) struct Helper {
    pub(crate) name: String,
    pub(crate) line: usize,
}

pub(crate) fn one_expression_helpers(source: &str) -> syn::Result<Vec<Helper>> {
    let mut finder = Helpers::default();
    finder.visit_file(&syn::parse_file(source)?);
    Ok(finder.found)
}

pub(crate) fn names_used(source: &str) -> syn::Result<Vec<String>> {
    let mut names = Names::default();
    names.visit_file(&syn::parse_file(source)?);
    Ok(names.used)
}

#[derive(Default)]
struct Helpers {
    found: Vec<Helper>,
}

impl Helpers {
    fn consider(&mut self, visibility: &Visibility, signature: &Signature, body: &Block) {
        if matches!(visibility, Visibility::Inherited) && signature.ident != "main" && one_plain_expression(body) {
            self.found.push(Helper { name: signature.ident.to_string(), line: signature.ident.span().start().line });
        }
    }
}

impl<'ast> Visit<'ast> for Helpers {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.consider(&item.vis, &item.sig, &item.block);
        visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        if item.trait_.is_none() {
            visit::visit_item_impl(self, item);
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.consider(&item.vis, &item.sig, &item.block);
        visit::visit_impl_item_fn(self, item);
    }
}

fn one_plain_expression(body: &Block) -> bool {
    let [Stmt::Expr(expression, None)] = body.stmts.as_slice() else {
        return false;
    };
    let mut flow = Flow::default();
    flow.visit_expr(expression);
    !flow.branches
}

#[derive(Default)]
struct Flow {
    branches: bool,
}

impl<'ast> Visit<'ast> for Flow {
    fn visit_expr_if(&mut self, _: &'ast ExprIf) {
        self.branches = true;
    }

    fn visit_expr_match(&mut self, _: &'ast ExprMatch) {
        self.branches = true;
    }

    fn visit_expr_loop(&mut self, _: &'ast ExprLoop) {
        self.branches = true;
    }

    fn visit_expr_while(&mut self, _: &'ast ExprWhile) {
        self.branches = true;
    }

    fn visit_expr_for_loop(&mut self, _: &'ast ExprForLoop) {
        self.branches = true;
    }

    fn visit_expr_try(&mut self, _: &'ast ExprTry) {
        self.branches = true;
    }
}

#[derive(Default)]
struct Names {
    used: Vec<String>,
}

impl<'ast> Visit<'ast> for Names {
    fn visit_expr_path(&mut self, expression: &'ast ExprPath) {
        self.used.extend(expression.path.segments.last().map(|segment| segment.ident.to_string()));
        visit::visit_expr_path(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        self.used.push(expression.method.to_string());
        visit::visit_expr_method_call(self, expression);
    }
}
