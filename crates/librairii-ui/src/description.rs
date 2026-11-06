//! HTML metadata becomes inert text. This tokenizer creates no DOM, URL, image,
//! script, network request or native control. Conversion runs on the controller.
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, states::RawKind,
};
use std::cell::RefCell;

#[derive(Debug, Default)]
struct Text {
    text: String,
    hidden: Vec<String>,
}
#[derive(Debug, Default)]
struct Sink(RefCell<Text>);
impl TokenSink for Sink {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        let mut state = self.0.borrow_mut();
        match token {
            Token::CharacterTokens(text) if state.hidden.is_empty() => state.text.push_str(&text),
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                let hidden = matches!(
                    name,
                    "script" | "style" | "iframe" | "object" | "template" | "noscript"
                );
                let start = tag.kind == TagKind::StartTag;
                if hidden {
                    if start {
                        state.hidden.push(name.into());
                    } else if let Some(index) = state.hidden.iter().rposition(|n| n == name) {
                        state.hidden.truncate(index);
                    }
                }
                if state.hidden.is_empty() && !hidden {
                    if name == "br" && start {
                        state.text.push('\n');
                    } else if name == "li" {
                        state.text.push('\n');
                        if start {
                            state.text.push_str("• ");
                        }
                    } else if matches!(
                        name,
                        "p" | "div"
                            | "section"
                            | "article"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                            | "blockquote"
                            | "ul"
                            | "ol"
                    ) {
                        state.text.push_str("\n\n");
                    }
                }
                if start {
                    match name {
                        "script" => return TokenSinkResult::RawData(RawKind::ScriptData),
                        "style" | "iframe" | "noscript" | "xmp" | "noembed" => {
                            return TokenSinkResult::RawData(RawKind::Rawtext);
                        }
                        "textarea" | "title" => return TokenSinkResult::RawData(RawKind::Rcdata),
                        "plaintext" => return TokenSinkResult::Plaintext,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}
/// Preserve paragraphs, literal less-than signs and all HTML5 named entities.
/// Input bounds belong to the metadata/import contract; no content is silently
/// truncated by this presentation conversion.
pub fn readable(source: &str) -> String {
    let queue = BufferQueue::default();
    queue.push_back(source.into());
    let tokenizer = Tokenizer::new(Sink::default(), Default::default());
    let _result = tokenizer.feed(&queue);
    tokenizer.end();
    let state = tokenizer.sink.0.into_inner();
    let mut result = String::with_capacity(state.text.len());
    let mut space = false;
    let mut newlines = 0;
    for character in state.text.chars() {
        if character == '\n' {
            space = false;
            if !result.is_empty() && newlines < 2 {
                result.push('\n');
                newlines += 1;
            }
        } else if character.is_whitespace() {
            space = true;
        } else {
            if space && !result.is_empty() && newlines == 0 {
                result.push(' ');
            }
            space = false;
            newlines = 0;
            result.push(character);
        }
    }
    result.trim_end().into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_frozen_paragraph_list_plain_text_and_html5_entities() {
        assert_eq!(
            readable(
                "Un dragon !<br/><br/>Aide <b>Lou</b> &amp; ses amis.<span class=\"SF_splitter\"></span>"
            ),
            "Un dragon !\n\nAide Lou & ses amis."
        );
        assert_eq!(
            readable("<p>First&nbsp;story.</p><p>Second story.</p>"),
            "First story.\n\nSecond story."
        );
        assert_eq!(
            readable("For ages 3 < 5.\nTwo adventures."),
            "For ages 3 < 5.\nTwo adventures."
        );
        assert_eq!(
            readable("<ul><li>Lou</li><li>Dragon</li></ul>"),
            "• Lou\n\n• Dragon"
        );
        assert_eq!(
            readable("&eacute; &NotEqualTilde; &#x1f9da; &unknown;"),
            "é ≂̸ 🧚 &unknown;"
        );
        assert_eq!(readable("<span></span><br/>"), "");
    }
    #[test]
    fn omits_active_hidden_and_nested_markup_and_keeps_anchor_text() {
        assert_eq!(
            readable(
                "<script>if(a<b)alert(1)</script><style>body{display:none}</style><iframe src=https://example.com>hidden</iframe><object><div>hidden</div></object><template>hidden<script>hidden</script></template><noscript>hidden</noscript><img src=missing onerror=alert(1)><a href=javascript:alert(1)>A story</a>"
            ),
            "A story"
        );
        assert_eq!(readable("Visible<script>unterminated <b>hidden"), "Visible");
        assert_eq!(
            readable("<textarea>&lt;b&gt;literal&lt;/b&gt;</textarea>"),
            "<b>literal</b>"
        );
    }
}
