use biome_analyze::{context::RuleContext, declare_rule, Ast, Rule, RuleDiagnostic};
use biome_console::markup;
use biome_js_syntax::{AnyJsMemberExpression, JsCallExpression};
use biome_rowan::AstNode;

declare_rule! {
    /// Prefer `for...of` statement instead of `Array.forEach`.
    ///
    /// Here's a summary of why `forEach` may be disallowed, and why `for...of` is preferred for almost any use-case of `forEach`:
    /// - Performance: Using `forEach` can lead to performance issues, especially when working with large arrays.
    /// When more requirements are added on, `forEach` typically gets chained with other methods like `filter` or `map`, causing multiple iterations over the same Array.
    /// Encouraging for loops discourages chaining and encourages single-iteration logic (e.g. using a continue instead of `filter`).
    ///
    /// - Readability: While `forEach` is a simple and concise way to iterate over an array, it can make the code less readable, especially when the callback function is complex.
    /// In contrast, using a for loop or a `for...of` loop can make the code more explicit and easier to read.
    ///
    /// - Debugging: `forEach` can make debugging more difficult, because it hides the iteration process.
    ///
    /// ## Caveat
    ///
    /// We consider all objects with a method named `forEach` to be iterable.
    /// This way, this rule applies to all objects with a method called `forEach`, not just `Array` instances.
    ///
    /// ## Examples
    ///
    /// ### Invalid
    ///
    /// ```js,expect_diagnostic
    /// els.forEach(el => {
    ///   el
    /// })
    /// ```
    ///
    /// ```js,expect_diagnostic
    /// els['forEach'](el => {
    ///   el
    /// })
    /// ```
    ///
    /// ## Valid
    ///
    /// ```js
    /// for (const el of els) {
    ///   el
    /// }
    /// ```
    ///
    pub(crate) NoForEach {
        version: "1.0.0",
        name: "noForEach",
        recommended: true,
    }
}

impl Rule for NoForEach {
    type Query = Ast<JsCallExpression>;
    type State = ();
    type Signals = Option<Self::State>;
    type Options = ();

    fn run(ctx: &RuleContext<Self>) -> Self::Signals {
        let node = ctx.query();
        let member_expression =
            AnyJsMemberExpression::cast_ref(node.callee().ok()?.omit_parentheses().syntax())?;
        (member_expression.member_name()?.text() == "forEach").then_some(())
    }

    fn diagnostic(ctx: &RuleContext<Self>, _state: &Self::State) -> Option<RuleDiagnostic> {
        let node = ctx.query();

        Some(RuleDiagnostic::new(
            rule_category!(),
            node.syntax().text_trimmed_range(),
            markup! {
                "Prefer for...of instead of Array.forEach"
            },
        ).note(markup!{
            <Emphasis>"forEach"</Emphasis>" could lead to performance issues when working with large arrays. When combined with functions like .filter or .map, this causes multiple iterations over the same type."
        }))
    }
}
