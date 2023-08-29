#[derive(Debug)]
pub struct Tokenizer {
    pub input: String,
    pub tokens: Vec<String>,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(PartialEq, Eq, Debug)]
pub struct Tokenized<'a> {
    pub tokens: &'a [String],
    pub span: Span,
}

pub fn normalize_diacritics(input: &str) -> String {
    diacritics::remove_diacritics(input)
        .replace(['\'', '-', '(', ')', '*', ',', ';'], " ")
        .to_ascii_lowercase()
}

impl Tokenized<'_> {
    pub fn normalize(&self) -> String {
        self.tokens
            .iter()
            .map(|t| diacritics::remove_diacritics(t))
            .map(|t| t.to_ascii_lowercase())
            .collect::<Vec<String>>()
            .join(" ")
    }
}

impl Tokenizer {
    pub fn parse(input: &str) -> Self {
        let tokens = input
            .replace(['\'', '-', '(', ')', '*', ',', ';'], " ")
            .split(' ')
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .collect();

        Self {
            input: input.to_string(),
            tokens,
        }
    }

    pub fn ngrams(&self, size: usize) -> impl Iterator<Item = Tokenized> {
        (0..self.tokens.len() - size + 1).map(move |idx| Tokenized {
            tokens: &self.tokens[idx..idx + size],
            span: Span {
                start: idx,
                end: idx + size,
            },
        })
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn region(&self, span: Span) -> String {
        self.tokens[span.start..span.end].join(" ")
    }
}

#[cfg(test)]
mod test {
    use crate::tokens::{Span, Tokenized, Tokenizer};

    #[test]
    fn should_get_ngram() {
        let tokenizer = Tokenizer::parse("j'aime les pommes");
        let mut ngrams = tokenizer.ngrams(2);

        assert_eq!(
            ngrams.next(),
            Some(Tokenized {
                tokens: &["j".to_string(), "aime".to_string()],
                span: Span { start: 0, end: 2 },
            })
        );
        assert_eq!(
            ngrams.next(),
            Some(Tokenized {
                tokens: &["aime".to_string(), "les".to_string()],
                span: Span { start: 1, end: 3 },
            })
        );
        assert_eq!(
            ngrams.next(),
            Some(Tokenized {
                tokens: &["les".to_string(), "pommes".to_string()],
                span: Span { start: 2, end: 4 },
            })
        );
    }
}
