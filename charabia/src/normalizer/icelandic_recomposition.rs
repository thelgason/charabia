use std::borrow::Cow;
use std::sync::LazyLock;

use aho_corasick::AhoCorasick;

use super::Normalizer;
use crate::normalizer::NormalizerOption;
use crate::{Language, Token};

static MATCHING_STR: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "A\u{301}", "a\u{301}", // á, Á (COMBINING ACUTE ACCENT)
        "E\u{301}", "e\u{301}", // é, É
        "I\u{301}", "i\u{301}", // í, Í
        "O\u{301}", "o\u{301}", // ó, Ó
        "U\u{301}", "u\u{301}", // ú, Ú
        "Y\u{301}", "y\u{301}", // ý, Ý
        "O\u{308}", "o\u{308}", // ö, Ö (COMBINING DIAERESIS)
        "Þ", "þ",               // Þ, þ (thorn)
        "Ð", "ð",               // Ð, ð (eth)
        "Æ", "æ",               // Æ, æ (ash)
    ])
    .unwrap()
});

/// Icelandic specialized [`Normalizer`].
///
/// This Normalizer recompose Icelandic characters containing diacritics.
///
/// This avoids the diacritic removal from the letter and preserves expected Icelandic character ordering.
pub struct IcelandicRecompositionNormalizer;

impl Normalizer for IcelandicRecompositionNormalizer {
    fn normalize<'o>(&self, mut token: Token<'o>, options: &NormalizerOption) -> Token<'o> {
        match token.char_map.take() {
            Some(mut char_map) => {
                // if a char_map already exists, iterate over it to reconstruct sub-strings.
                let mut lemma = String::new();
                let mut tail = token.lemma.as_ref();
                let mut normalized = String::new();
                for (_, normalized_len) in char_map.iter_mut() {
                    let (head, t) = tail.split_at(*normalized_len as usize);
                    tail = t;
                    normalized.clear();
                    // then normalize each sub-strings recomputing the size in the char_map.
                    let mut peekable = head.chars().peekable();
                    while let Some(c) = peekable.next() {
                        let (c, peek_consumed) = recompose_icelandic(c, peekable.peek());
                        if peek_consumed {
                            peekable.next();
                        }

                        normalized.push(c);
                    }

                    *normalized_len = normalized.len() as u8;
                    lemma.push_str(normalized.as_ref());
                }

                token.lemma = Cow::Owned(lemma);
                token.char_map = Some(char_map);
            }
            None => {
                // if no char_map exists, iterate over the lemma recomposing characters.
                let mut char_map = Vec::new();
                let mut lemma = String::new();
                let mut peekable = token.lemma.chars().peekable();
                while let Some(c) = peekable.next() {
                    let (normalized, peek_consumed) = recompose_icelandic(c, peekable.peek());
                    if peek_consumed {
                        peekable.next();
                    }

                    if options.create_char_map {
                        char_map.push((c.len_utf8() as u8, normalized.len_utf8() as u8));
                    }
                    lemma.push(normalized);
                }
                token.lemma = Cow::Owned(lemma);
                if options.create_char_map {
                    token.char_map = Some(char_map);
                }
            }
        }

        token
    }

    // Returns `true` if the Normalizer should be used.
    fn should_normalize(&self, token: &Token) -> bool {
        token.language == Some(Language::Isl) && MATCHING_STR.is_match(token.lemma())
    }
}

fn recompose_icelandic(current: char, next: Option<&char>) -> (char, bool) {
    match (current, next) {
        // Acute accent combinations
        ('A', Some('\u{301}')) => ('Á', true),
        ('a', Some('\u{301}')) => ('á', true),
        ('E', Some('\u{301}')) => ('É', true),
        ('e', Some('\u{301}')) => ('é', true),
        ('I', Some('\u{301}')) => ('Í', true),
        ('i', Some('\u{301}')) => ('í', true),
        ('O', Some('\u{301}')) => ('Ó', true),
        ('o', Some('\u{301}')) => ('ó', true),
        ('U', Some('\u{301}')) => ('Ú', true),
        ('u', Some('\u{301}')) => ('ú', true),
        ('Y', Some('\u{301}')) => ('Ý', true),
        ('y', Some('\u{301}')) => ('ý', true),
        // Diaeresis (umlaut)
        ('O', Some('\u{308}')) => ('Ö', true),
        ('o', Some('\u{308}')) => ('ö', true),
        // Special Icelandic letters are already composed
        (c, _) => (c, false),
    }
}

// Test the normalizer:
#[cfg(test)]
mod test {
    use std::borrow::Cow::Owned;

    use crate::normalizer::test::test_normalizer;
    use crate::normalizer::Normalizer;
    use crate::token::TokenKind;
    use crate::Script;

    use super::*;

    // base tokens to normalize.
    fn tokens() -> Vec<Token<'static>> {
        vec![Token {
            lemma: Owned("ÁaéíóúýöæþðÆÞÐ".to_string()),
            char_end: 14,
            byte_end: 27,
            script: Script::Latin,
            language: Some(Language::Isl),
            ..Default::default()
        }]
    }

    // expected result of the current Normalizer.
    fn normalizer_result() -> Vec<Token<'static>> {
        vec![Token {
            lemma: Owned("ÁaéíóúýöæþðÆÞÐ".to_string()),
            char_end: 14,
            byte_end: 27,
            script: Script::Latin,
            language: Some(Language::Isl),
            ..Default::default()
        }]
    }

    // expected result of the complete Normalizer pipeline.
    fn normalized_tokens() -> Vec<Token<'static>> {
        vec![Token {
            lemma: Owned("áaéíóúýöæþðæþð".to_string()),
            char_end: 14,
            byte_end: 27,
            char_map: Some(vec![
                (2, 2), // Á -> á
                (1, 1), // a -> a
                (2, 2), // é -> é
                (2, 2), // í -> í
                (2, 2), // ó -> ó
                (2, 2), // ú -> ú
                (2, 2), // ý -> ý
                (2, 2), // ö -> ö
                (2, 2), // æ -> æ
                (2, 2), // þ -> þ
                (2, 2), // ð -> ð
                (2, 2), // Æ -> æ
                (2, 2), // Þ -> þ
                (2, 2), // Ð -> ð
            ]),
            script: Script::Latin,
            kind: TokenKind::Word,
            language: Some(Language::Isl),
            ..Default::default()
        }]
    }

    test_normalizer!(
        IcelandicRecompositionNormalizer,
        tokens(),
        normalizer_result(),
        normalized_tokens()
    );
}