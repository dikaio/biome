use crate::{semantic_services::Semantic, JsRuleAction};
use rome_analyze::{
    context::RuleContext, declare_rule, ActionCategory, Rule, RuleCategory, RuleDiagnostic,
};
use rome_console::markup;
use rome_diagnostics::Applicability;
use rome_js_semantic::{AllReferencesExtensions, Reference};
use rome_js_syntax::{
    JsAnyExpression, JsAnyLiteralExpression, JsAnyRoot, JsIdentifierBinding,
    JsIdentifierExpression, JsLanguage, JsStringLiteralExpression, JsVariableDeclaration,
    JsVariableDeclarator, JsVariableDeclaratorList, JsVariableStatement,
};
use rome_rowan::{AstNode, AstSeparatedList, BatchMutation, BatchMutationExt, SyntaxNodeCast};

declare_rule! {
    /// Disallow the use of constants which its value is the upper-case version of its name.
    ///
    /// ## Examples
    ///
    /// ### Invalid
    ///
    /// ```js,expect_diagnostic
    /// const FOO = "FOO";
    /// console.log(FOO);
    /// ```
    pub(crate) NoShoutyConstants {
        version: "0.7.0",
        name: "noShoutyConstants",
        recommended: true,
    }
}

/// Check for
/// a = "a" (true)
/// a = "b" (false)
fn is_id_and_string_literal_inner_text_equal(
    declarator: &JsVariableDeclarator,
) -> Option<(JsIdentifierBinding, JsStringLiteralExpression)> {
    let id = declarator.id().ok()?;
    let id = id.as_js_any_binding()?.as_js_identifier_binding()?;
    let id_text = id.syntax().text_trimmed();

    let expression = declarator.initializer()?.expression().ok()?;
    let literal = expression
        .as_js_any_literal_expression()?
        .as_js_string_literal_expression()?;
    let literal_text = literal.inner_string_text();

    if id_text == literal_text {
        Some((id.clone(), literal.clone()))
    } else {
        None
    }
}

/// Removes the declarator, and:
/// 1 - removes the statement if the declaration only has one declarator;
/// 2 - removes commas around the declarator to keep the declaration list valid.
fn remove_declarator(
    batch: &mut BatchMutation<JsLanguage, JsAnyRoot>,
    declarator: &JsVariableDeclarator,
) -> Option<()> {
    let list = declarator.parent::<JsVariableDeclaratorList>()?;
    let declaration = list.parent::<JsVariableDeclaration>()?;

    if list.syntax_list().len() == 1 {
        let statement = declaration.parent::<JsVariableStatement>()?;
        batch.remove_node(statement);
    } else {
        let mut elements = list.elements();

        // Find the declarator we want to remove
        // remove its trailing comma, if there is one
        let mut previous_element = None;
        for element in elements.by_ref() {
            if let Ok(node) = element.node() {
                if node == declarator {
                    batch.remove_node(node.clone());
                    if let Some(comma) = element.trailing_separator().ok().flatten() {
                        batch.remove_token(comma.clone());
                    }
                    break;
                }
            }
            previous_element = Some(element);
        }

        // if it is the last declarator of the list
        // removes the comma before this element
        let is_last = elements.next().is_none();
        if is_last {
            if let Some(element) = previous_element {
                if let Some(comma) = element.trailing_separator().ok().flatten() {
                    batch.remove_token(comma.clone());
                }
            }
        }
    }

    Some(())
}

pub struct State {
    literal: JsStringLiteralExpression,
    references: Vec<Reference>,
}

impl Rule for NoShoutyConstants {
    const CATEGORY: RuleCategory = RuleCategory::Lint;

    type Query = Semantic<JsVariableDeclarator>;
    type State = State;
    type Signals = Option<Self::State>;

    fn run(ctx: &RuleContext<Self>) -> Option<Self::State> {
        let declarator = ctx.query();
        let declaration = declarator
            .parent::<JsVariableDeclaratorList>()?
            .parent::<JsVariableDeclaration>()?;

        if declaration.is_const() {
            if let Some((binding, literal)) = is_id_and_string_literal_inner_text_equal(declarator)
            {
                return Some(State {
                    literal,
                    references: binding.all_references(ctx.model()).collect(),
                });
            }
        }

        None
    }

    fn diagnostic(ctx: &RuleContext<Self>, state: &Self::State) -> Option<RuleDiagnostic> {
        let declarator = ctx.query();

        let mut diag = RuleDiagnostic::warning(
            declarator.syntax().text_trimmed_range(),
            markup! {
                "Redundant constant declaration."
            },
        );

        for reference in state.references.iter() {
            let node = reference.node();
            diag = diag.secondary(node.text_trimmed_range(), "Used here.")
        }

        let diag = diag.footer_note(
            markup! {"You should avoid declaring constants with a string that's the same
    value as the variable name. It introduces a level of unnecessary
    indirection when it's only two additional characters to inline."},
        );

        Some(diag)
    }

    fn action(ctx: &RuleContext<Self>, state: &Self::State) -> Option<JsRuleAction> {
        let root = ctx.root();
        let literal = JsAnyLiteralExpression::JsStringLiteralExpression(state.literal.clone());

        let mut batch = root.begin();

        remove_declarator(&mut batch, ctx.query());

        for reference in state.references.iter() {
            let node = reference
                .node()
                .parent()?
                .cast::<JsIdentifierExpression>()?;

            batch.replace_node(
                JsAnyExpression::JsIdentifierExpression(node),
                JsAnyExpression::JsAnyLiteralExpression(literal.clone()),
            );
        }

        Some(JsRuleAction {
            category: ActionCategory::Refactor,
            applicability: Applicability::Unspecified,
            message: markup! { "Use the constant value directly" }.to_owned(),
            mutation: batch,
        })
    }
}