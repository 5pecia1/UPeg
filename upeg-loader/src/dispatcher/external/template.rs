//! `args_template` token rendering for the External invoker.
//!
//! One token of the declared `args_template` becomes at most one
//! argument of the spawned command. Substitution used to require a
//! token to be *exactly* `{key}`, which made `--manifest-path={path}`
//! and `-p{crate}` impossible to express — real CLIs use those shapes
//! constantly. A token is now a small template:
//!
//!   * `{key}` is substituted anywhere inside the token;
//!   * `{{` and `}}` are literal `{` and `}`;
//!   * a token holding no placeholder is passed through verbatim;
//!   * a token that is *nothing but* `{key}` for an **optional** input
//!     with no value and no `default` is **dropped** from the arg list,
//!     so `--flag`-style optional arguments leave no stray `""` behind.
//!
//! Every other absent placeholder renders as the empty string and the
//! token **keeps its position**. Dropping a token that carries literal
//! text would shift every following argument by one — `["rm", "-rf",
//! "{dir}/build"]` collapsing to `["rm", "-rf"]` is the difference
//! between deleting a build directory and deleting the working
//! directory. Which keys may be absent at all is settled earlier, at
//! load time: [`placeholders`](ArgTemplate::placeholders) feeds
//! `crate::parse::templates`, which rejects a manifest whose token
//! names an input it never declared.
//!
//! Whitespace inside the braces is trimmed (`{ input }` looks up
//! `input`), and an unterminated `{` is treated as literal text rather
//! than a parse failure — a manifest typo should degrade to a visible
//! literal argument, not to a silently dropped one.

const PLACEHOLDER_OPEN: char = '{';
const PLACEHOLDER_CLOSE: char = '}';

/// One piece of a parsed token.
#[derive(Debug, PartialEq, Eq)]
enum Segment {
    Literal(String),
    Placeholder(String),
}

/// A parsed `args_template` token, ready to render against one
/// invocation's arguments.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ArgTemplate {
    segments: Vec<Segment>,
}

/// What one placeholder key resolves to for a single invocation.
///
/// The invoker owns the declaration knowledge (caller value → declared
/// `default` → nothing) and this module owns the token structure; the
/// drop decision needs both, so it is spelled out here rather than
/// inferred from an empty string.
pub(super) enum Substitution {
    /// A caller-supplied value, or the input's declared `default`.
    Text(String),
    /// An optional input with neither a caller value nor a `default`.
    AbsentOptional,
    /// A required input with no value — still rendered (as nothing) so
    /// the token keeps its argument position.
    AbsentRequired,
}

impl ArgTemplate {
    pub(crate) fn parse(token: &str) -> Self {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut rest = token;
        while let Some(open) = rest.find(PLACEHOLDER_OPEN) {
            let (before, after_open) = rest.split_at(open);
            literal.push_str(&unescape_closing_braces(before));
            let after_open = &after_open[PLACEHOLDER_OPEN.len_utf8()..];
            if let Some(escaped) = after_open.strip_prefix(PLACEHOLDER_OPEN) {
                literal.push(PLACEHOLDER_OPEN);
                rest = escaped;
                continue;
            }
            let Some(close) = after_open.find(PLACEHOLDER_CLOSE) else {
                // Unterminated `{`: the rest of the token is literal.
                literal.push(PLACEHOLDER_OPEN);
                literal.push_str(after_open);
                rest = "";
                break;
            };
            let (key, after_close) = after_open.split_at(close);
            if !literal.is_empty() {
                segments.push(Segment::Literal(std::mem::take(&mut literal)));
            }
            segments.push(Segment::Placeholder(key.trim().to_string()));
            rest = &after_close[PLACEHOLDER_CLOSE.len_utf8()..];
        }
        literal.push_str(&unescape_closing_braces(rest));
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }
        Self { segments }
    }

    /// Every input name this token substitutes, in declaration order.
    ///
    /// Load-time validation uses this to reject a placeholder naming an
    /// input the tool never declared, so one parser serves both the
    /// manifest check and dispatch.
    pub(crate) fn placeholders(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().filter_map(|segment| match segment {
            Segment::Placeholder(key) => Some(key.as_str()),
            Segment::Literal(_) => None,
        })
    }

    /// Render this token. `None` means "drop this argument".
    ///
    /// A token is dropped only when it is a lone placeholder for an
    /// absent optional input; see the module doc for why every other
    /// absent placeholder renders as nothing in place.
    pub(super) fn render(&self, resolve: &dyn Fn(&str) -> Substitution) -> Option<String> {
        if let [Segment::Placeholder(key)] = self.segments.as_slice() {
            return match resolve(key) {
                Substitution::Text(text) => Some(text),
                Substitution::AbsentOptional => None,
                Substitution::AbsentRequired => Some(String::new()),
            };
        }
        let mut rendered = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(text) => rendered.push_str(text),
                Segment::Placeholder(key) => match resolve(key) {
                    Substitution::Text(text) => rendered.push_str(&text),
                    Substitution::AbsentOptional | Substitution::AbsentRequired => {}
                },
            }
        }
        Some(rendered)
    }
}

/// Collapse `}}` into `}` in a run of literal text.
fn unescape_closing_braces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find(PLACEHOLDER_CLOSE) {
        let (before, after) = rest.split_at(position);
        out.push_str(before);
        out.push(PLACEHOLDER_CLOSE);
        let after = &after[PLACEHOLDER_CLOSE.len_utf8()..];
        rest = after.strip_prefix(PLACEHOLDER_CLOSE).unwrap_or(after);
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render `token` with `값` supplying every known key. Any other key
    /// is an absent **optional** input, the only shape that may drop a
    /// token.
    fn 렌더(token: &str, 값: &[(&str, &str)]) -> Option<String> {
        let 표 = 값.to_vec();
        ArgTemplate::parse(token).render(&|key| {
            표.iter()
                .find(|(name, _)| *name == key)
                .map_or(Substitution::AbsentOptional, |(_, value)| {
                    Substitution::Text((*value).to_string())
                })
        })
    }

    /// Same, but every unknown key is an absent **required** input.
    fn 필수_렌더(token: &str) -> Option<String> {
        ArgTemplate::parse(token).render(&|_| Substitution::AbsentRequired)
    }

    #[test]
    fn 자리표시자가_없는_토큰은_그대로_통과한다() {
        assert_eq!(렌더("--all", &[]).as_deref(), Some("--all"));
        assert_eq!(렌더("", &[]).as_deref(), Some(""));
    }

    #[test]
    fn 토큰_중간의_자리표시자를_치환한다() {
        assert_eq!(
            렌더("--manifest-path={path}", &[("path", "/repo/Cargo.toml")]).as_deref(),
            Some("--manifest-path=/repo/Cargo.toml")
        );
        assert_eq!(
            렌더("-p{crate}", &[("crate", "upeg-core")]).as_deref(),
            Some("-pupeg-core")
        );
    }

    #[test]
    fn 값이_없는_선택_입력의_단독_자리표시자만_버려진다() {
        assert_eq!(렌더("{count}", &[]), None);
    }

    #[test]
    fn 값이_없는_필수_입력의_단독_자리표시자는_자리를_지킨다() {
        assert_eq!(
            필수_렌더("{dir}").as_deref(),
            Some(""),
            "dropping it would shift every following argument"
        );
    }

    #[test]
    fn 리터럴이_붙은_토큰은_값이_없어도_자리를_지킨다() {
        assert_eq!(
            렌더("--manifest-path={path}", &[]).as_deref(),
            Some("--manifest-path=")
        );
        assert_eq!(
            렌더("{dir}/build", &[]).as_deref(),
            Some("/build"),
            "`rm -rf {{dir}}/build` must not collapse to `rm -rf`"
        );
    }

    #[test]
    fn 자리표시자가_둘_이상이면_비어도_토큰을_유지한다() {
        assert_eq!(렌더("{a}{b}", &[]).as_deref(), Some(""));
    }

    #[test]
    fn 중괄호_이스케이프는_리터럴_중괄호가_된다() {
        assert_eq!(렌더("{{literal}}", &[]).as_deref(), Some("{literal}"));
        assert_eq!(
            렌더("{{{key}}}", &[("key", "v")]).as_deref(),
            Some("{v}"),
            "escaped braces around a real placeholder"
        );
    }

    #[test]
    fn 중괄호_안의_공백은_잘라낸다() {
        assert_eq!(렌더("{ input }", &[("input", "hi")]).as_deref(), Some("hi"));
    }

    #[test]
    fn 닫히지_않은_중괄호는_리터럴로_남는다() {
        assert_eq!(렌더("{unclosed", &[]).as_deref(), Some("{unclosed"));
    }

    #[test]
    fn 여러_자리표시자를_한_토큰에서_이어붙인다() {
        assert_eq!(
            렌더("{a}-{b}", &[("a", "x"), ("b", "y")]).as_deref(),
            Some("x-y")
        );
    }

    #[test]
    fn 자리표시자_목록은_선언_순서로_모든_키를_돌려준다() {
        let template = ArgTemplate::parse("--from={a}/{ b }{{c}}");

        assert_eq!(template.placeholders().collect::<Vec<_>>(), vec!["a", "b"]);
        assert!(ArgTemplate::parse("--all").placeholders().next().is_none());
    }
}
