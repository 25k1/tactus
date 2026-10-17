use crate::numbers::parse_number;
use crate::text::{close, normalize};

const NUMBER_SPAN: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Word(String),
    Choice(Vec<Vec<Node>>),
    Optional(Vec<Vec<Node>>),
    Number,
    Name,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Captures {
    pub number: Option<u32>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub captures: Captures,
    pub literals: usize,
    pub fuzzy: usize,
}

impl Match {
    pub fn better_than(&self, other: &Match) -> bool {
        (self.literals, std::cmp::Reverse(self.fuzzy)) > (other.literals, std::cmp::Reverse(other.fuzzy))
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Open(char),
    Close(char),
    Bar,
    Number,
    Name,
    Word(String),
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' | '[' => tokens.push(Token::Open(c)),
            ')' | ']' => tokens.push(Token::Close(c)),
            '|' => tokens.push(Token::Bar),
            '{' => {
                let slot: String = chars.by_ref().take_while(|c| *c != '}').collect();
                tokens.push(match slot.trim() {
                    "number" => Token::Number,
                    "name" => Token::Name,
                    other => return Err(format!("unknown slot {{{other}}}")),
                });
            }
            c if c.is_whitespace() => {}
            c => {
                let mut word = c.to_string();
                while let Some(next) = chars.peek().filter(|next| !"()[]|{ \t".contains(**next)) {
                    word.push(*next);
                    chars.next();
                }
                tokens.extend(normalize(&word).split_whitespace().map(|part| Token::Word(part.to_owned())));
            }
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn alternatives(&mut self) -> Result<Vec<Vec<Node>>, String> {
        let mut alternatives = vec![self.sequence()?];
        while self.tokens.get(self.position) == Some(&Token::Bar) {
            self.position += 1;
            alternatives.push(self.sequence()?);
        }
        Ok(alternatives)
    }

    fn sequence(&mut self) -> Result<Vec<Node>, String> {
        let mut nodes = Vec::new();
        while let Some(token) = self.tokens.get(self.position).cloned() {
            self.position += 1;
            nodes.push(match token {
                Token::Word(word) => Node::Word(word),
                Token::Number => Node::Number,
                Token::Name => Node::Name,
                Token::Open(open) => {
                    let inner = self.alternatives()?;
                    let close = if open == '(' { ')' } else { ']' };
                    if self.tokens.get(self.position) != Some(&Token::Close(close)) {
                        return Err(format!("missing {close}"));
                    }
                    self.position += 1;
                    if open == '(' { Node::Choice(inner) } else { Node::Optional(inner) }
                }
                Token::Close(_) | Token::Bar => {
                    self.position -= 1;
                    break;
                }
            });
        }
        if nodes.is_empty() { Err("empty phrase".into()) } else { Ok(nodes) }
    }
}

pub fn parse(text: &str) -> Result<Vec<Node>, String> {
    let mut parser = Parser { tokens: tokenize(text)?, position: 0 };
    let nodes = parser.sequence()?;
    match parser.tokens.get(parser.position) {
        None => Ok(nodes),
        Some(token) => Err(format!("unexpected {token:?}")),
    }
}

pub fn split_alternatives(text: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '|' if depth == 0 => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(part) = parts.last_mut() {
            part.push(c);
        }
    }
    parts.into_iter().map(|part| part.trim().to_owned()).collect()
}

struct Search<'a> {
    tokens: &'a [&'a str],
    fuzzy: bool,
    best: Option<Match>,
}

impl Search<'_> {
    fn run(&mut self, work: &[&Node], position: usize, captures: Captures, literals: usize, fuzzy: usize) {
        let Some((node, rest)) = work.split_first() else {
            if position == self.tokens.len() {
                let found = Match { captures, literals, fuzzy };
                if self.best.as_ref().is_none_or(|best| found.better_than(best)) {
                    self.best = Some(found);
                }
            }
            return;
        };
        let remaining = self.tokens.len() - position;
        match node {
            Node::Word(word) => {
                let Some(heard) = self.tokens.get(position) else { return };
                if heard == word {
                    self.run(rest, position + 1, captures, literals + 1, fuzzy);
                } else if self.fuzzy && close(word, heard) {
                    self.run(rest, position + 1, captures, literals + 1, fuzzy + 1);
                }
            }
            Node::Choice(alternatives) | Node::Optional(alternatives) => {
                for alternative in alternatives {
                    let next: Vec<&Node> = alternative.iter().chain(rest.iter().copied()).collect();
                    self.run(&next, position, captures.clone(), literals, fuzzy);
                }
                if matches!(node, Node::Optional(_)) {
                    self.run(rest, position, captures, literals, fuzzy);
                }
            }
            Node::Number => {
                for span in (1..=remaining.min(NUMBER_SPAN)).rev() {
                    if let Some(number) = parse_number(&self.tokens[position..position + span]) {
                        let captures = Captures { number: Some(number), ..captures.clone() };
                        self.run(rest, position + span, captures, literals, fuzzy);
                    }
                }
            }
            Node::Name => {
                for span in 1..=remaining {
                    let name = self.tokens[position..position + span].join(" ");
                    let captures = Captures { name: Some(name), ..captures.clone() };
                    self.run(rest, position + span, captures, literals, fuzzy);
                }
            }
        }
    }
}

pub fn matches(nodes: &[Node], tokens: &[&str], fuzzy: bool) -> Option<Match> {
    let work: Vec<&Node> = nodes.iter().collect();
    let mut search = Search { tokens, fuzzy, best: None };
    search.run(&work, 0, Captures::default(), 0, 0);
    search.best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn try_match(pattern: &str, heard: &str) -> Option<Match> {
        let tokens: Vec<&str> = heard.split_whitespace().collect();
        matches(&parse(pattern).unwrap(), &tokens, false)
    }

    #[test]
    fn parses_groups_options_and_slots() {
        let nodes = parse("(сделай|поставь) громкость [на] {number}").unwrap();
        assert_eq!(nodes.len(), 4);
        assert!(matches!(&nodes[0], Node::Choice(alternatives) if alternatives.len() == 2));
        assert!(matches!(&nodes[2], Node::Optional(_)));
        assert_eq!(nodes[3], Node::Number);
        assert!(parse("(a|b").is_err());
        assert!(parse("{volume}").is_err());
        assert!(parse("a ]").is_err());
        assert_eq!(split_alternatives("a (b|c) | d [e|f]"), ["a (b|c)", "d [e|f]"]);
    }

    #[test]
    fn matches_numbers_names_and_optional_words() {
        let found =
            try_match("(сделай|поставь) громкость [на] {number}", "поставь громкость на тридцать пять").unwrap();
        assert_eq!((found.captures.number, found.literals), (Some(35), 3));
        let found = try_match("включи альбом {name}", "включи альбом master of puppets").unwrap();
        assert_eq!(found.captures.name.as_deref(), Some("master of puppets"));
        assert!(try_match("включи альбом {name}", "включи альбом").is_none());
        assert!(try_match("громче", "громче громче").is_none());
    }

    #[test]
    fn fuzzy_matching_is_opt_in() {
        let nodes = parse("следующий трек").unwrap();
        assert!(matches(&nodes, &["следущий", "трек"], false).is_none());
        assert_eq!(matches(&nodes, &["следущий", "трек"], true).unwrap().fuzzy, 1);
    }
}
