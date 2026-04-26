use std::collections::HashSet;

use crate::grep::{pattern_get, pattern_get_include};
use crate::task::Task;
use serde::Serialize;

#[derive(Debug, PartialEq, Clone)]
enum Token {
    TagPrefix,
    PropertyPrefix,
    TitlePrefix,
    StatePrefix,
    And,
    Or,
    Lparen,
    Rparen,
    Minus,
    Equals,
    Ident(String),
    QuotedString(String),
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum FilterExpr {
    Tag(String),
    Property(String, Option<String>),
    Title(String),
    State(String),
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),
}

#[derive(Debug, PartialEq)]
pub struct QueryError {
    pub message: String,
    pub pos: usize,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "query error at position {}: {}", self.pos, self.message)
    }
}

const PREFIX_TAG: &str = "tag";
const PREFIX_PROPERTY: &str = "property";
const PREFIX_TITLE: &str = "title";
const PREFIX_STATE: &str = "state";

fn is_known_prefix(s: &str) -> bool {
    s == PREFIX_TAG || s == PREFIX_PROPERTY || s == PREFIX_TITLE || s == PREFIX_STATE
}

fn prefix_token(prefix: &str) -> Token {
    match prefix {
        PREFIX_TAG => Token::TagPrefix,
        PREFIX_PROPERTY => Token::PropertyPrefix,
        PREFIX_TITLE => Token::TitlePrefix,
        PREFIX_STATE => Token::StatePrefix,
        _ => unreachable!(),
    }
}

fn tokenize_word(word: &str) -> Result<Vec<Token>, QueryError> {
    let mut result = Vec::new();

    if let Some(rest) = word.strip_prefix('-') {
        if rest.is_empty() {
            result.push(Token::Ident("-".to_string()));
            return Ok(result);
        }
        let colon_pos = rest.find(':');
        if let Some(cp) = colon_pos {
            let prefix = &rest[..cp];
            if is_known_prefix(prefix) {
                result.push(Token::Minus);
                result.push(prefix_token(prefix));
                let remainder = &rest[cp + 1..];
                if !remainder.is_empty() {
                    result.push(Token::Ident(remainder.to_string()));
                }
                return Ok(result);
            }
        }
        result.push(Token::Ident(word.to_string()));
        return Ok(result);
    }

    if word == "AND" {
        result.push(Token::And);
        return Ok(result);
    }
    if word == "OR" {
        result.push(Token::Or);
        return Ok(result);
    }

    if let Some(colon_pos) = word.find(':') {
        let prefix = &word[..colon_pos];
        if is_known_prefix(prefix) {
            result.push(prefix_token(prefix));
            let remainder = &word[colon_pos + 1..];
            if !remainder.is_empty() {
                result.push(Token::Ident(remainder.to_string()));
            }
            return Ok(result);
        }
    }

    if word.ends_with(':') {
        let prefix = &word[..word.len() - 1];
        if is_known_prefix(prefix) {
            result.push(prefix_token(prefix));
            return Ok(result);
        }
    }

    result.push(Token::Ident(word.to_string()));
    Ok(result)
}

fn tokenize(input: &str) -> Result<Vec<Token>, QueryError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        let ch = chars[pos];

        if ch.is_whitespace() {
            pos += 1;
            continue;
        }

        match ch {
            '(' => {
                tokens.push(Token::Lparen);
                pos += 1;
            }
            ')' => {
                tokens.push(Token::Rparen);
                pos += 1;
            }
            '=' => {
                tokens.push(Token::Equals);
                pos += 1;
            }
            '"' => {
                pos += 1;
                let start = pos;
                while pos < chars.len() && chars[pos] != '"' {
                    pos += 1;
                }
                if pos >= chars.len() {
                    return Err(QueryError {
                        message: "unterminated string".to_string(),
                        pos: start.saturating_sub(1),
                    });
                }
                let s: String = chars[start..pos].iter().collect();
                tokens.push(Token::QuotedString(s));
                pos += 1;
            }
            _ if ch == '-' || ch.is_alphabetic() || ch == '_' || ch.is_ascii_digit() => {
                if ch == '-' && pos + 1 < chars.len() && chars[pos + 1] == '(' {
                    tokens.push(Token::Minus);
                    pos += 1;
                    continue;
                }
                let start = pos;
                while pos < chars.len() {
                    let c = chars[pos];
                    if c == '"' {
                        break;
                    }
                    if c.is_alphanumeric() || c == '_' || c == '-' || c == ':' {
                        pos += 1;
                    } else {
                        break;
                    }
                }
                let word: String = chars[start..pos].iter().collect();
                for t in tokenize_word(&word)? {
                    tokens.push(t);
                }
            }
            _ => {
                return Err(QueryError {
                    message: format!("unexpected character '{}'", ch),
                    pos,
                });
            }
        }
    }

    Ok(tokens)
}

struct ParserState<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> ParserState<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn expect_ident(&mut self, context: &str) -> Result<String, QueryError> {
        match self.peek().cloned() {
            Some(Token::Ident(s)) => {
                self.advance();
                Ok(s)
            }
            Some(Token::QuotedString(s)) => {
                self.advance();
                Ok(s)
            }
            Some(tok) => Err(QueryError {
                message: format!(
                    "expected value for {} but got {:?}",
                    context,
                    token_name(&tok)
                ),
                pos: self.pos,
            }),
            None => Err(QueryError {
                message: format!("expected value for {} but reached end of input", context),
                pos: self.pos,
            }),
        }
    }

    fn parse_expr(&mut self) -> Result<FilterExpr, QueryError> {
        let expr = self.parse_or()?;
        if self.pos < self.tokens.len() {
            return Err(QueryError {
                message: format!(
                    "unexpected token {:?} after expression",
                    token_name(self.peek().unwrap())
                ),
                pos: self.pos,
            });
        }
        Ok(expr)
    }

    fn parse_or(&mut self) -> Result<FilterExpr, QueryError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = FilterExpr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<FilterExpr, QueryError> {
        let mut left = self.parse_not()?;
        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_not()?;
            left = FilterExpr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<FilterExpr, QueryError> {
        if self.peek() == Some(&Token::Minus) {
            self.advance();
            let expr = self.parse_atom()?;
            Ok(FilterExpr::Not(Box::new(expr)))
        } else {
            self.parse_atom()
        }
    }

    fn parse_atom(&mut self) -> Result<FilterExpr, QueryError> {
        match self.peek().cloned() {
            Some(Token::Lparen) => {
                self.advance();
                let expr = self.parse_or()?;
                match self.peek() {
                    Some(Token::Rparen) => {
                        self.advance();
                        Ok(expr)
                    }
                    Some(tok) => Err(QueryError {
                        message: format!("expected ')' but got {:?}", token_name(&tok)),
                        pos: self.pos,
                    }),
                    None => Err(QueryError {
                        message: "expected ')' but reached end of input".to_string(),
                        pos: self.pos,
                    }),
                }
            }
            Some(Token::TagPrefix) => {
                self.advance();
                let value = self.expect_ident("tag")?;
                Ok(FilterExpr::Tag(value))
            }
            Some(Token::PropertyPrefix) => {
                self.advance();
                let key = self.expect_ident("property key")?;
                if self.peek() == Some(&Token::Equals) {
                    self.advance();
                    let value = self.expect_ident("property value")?;
                    Ok(FilterExpr::Property(key, Some(value)))
                } else {
                    Ok(FilterExpr::Property(key, None))
                }
            }
            Some(Token::TitlePrefix) => {
                self.advance();
                let query = self.expect_ident("title query")?;
                Ok(FilterExpr::Title(query))
            }
            Some(Token::StatePrefix) => {
                self.advance();
                let state = self.expect_ident("state")?;
                Ok(FilterExpr::State(state))
            }
            Some(tok) => Err(QueryError {
                message: format!("unexpected token {:?}", token_name(&tok)),
                pos: self.pos,
            }),
            None => Err(QueryError {
                message: "unexpected end of input".to_string(),
                pos: self.pos,
            }),
        }
    }
}

fn token_name(tok: &Token) -> &'static str {
    match tok {
        Token::TagPrefix => "tag:",
        Token::PropertyPrefix => "property:",
        Token::TitlePrefix => "title:",
        Token::StatePrefix => "state:",
        Token::And => "AND",
        Token::Or => "OR",
        Token::Lparen => "(",
        Token::Rparen => ")",
        Token::Minus => "-",
        Token::Equals => "=",
        Token::Ident(_) => "ident",
        Token::QuotedString(_) => "quoted string",
    }
}

pub fn parse_query(input: &str) -> Result<FilterExpr, QueryError> {
    if input.trim().is_empty() {
        return Err(QueryError {
            message: "empty query".to_string(),
            pos: 0,
        });
    }
    let tokens = tokenize(input)?;
    let mut parser = ParserState {
        tokens: &tokens,
        pos: 0,
    };
    parser.parse_expr()
}

pub fn evaluate(expr: &FilterExpr, task: &Task) -> bool {
    match expr {
        FilterExpr::Tag(value) => task.tags.contains(value),
        FilterExpr::Property(key, Some(value)) => task
            .properties
            .get(key)
            .map(|pv| {
                if value.contains('T') {
                    pv == value
                } else {
                    pv.split('T').next().unwrap_or(pv.as_str()) == value.as_str()
                }
            })
            .unwrap_or(false),
        FilterExpr::Property(key, None) => task.properties.contains_key(key),
        FilterExpr::Title(query) => task
            .title
            .to_lowercase()
            .contains(&query.to_lowercase()),
        FilterExpr::State(state) => task.state == *state,
        FilterExpr::And(left, right) => evaluate(left, task) && evaluate(right, task),
        FilterExpr::Or(left, right) => evaluate(left, task) || evaluate(right, task),
        FilterExpr::Not(inner) => !evaluate(inner, task),
    }
}

pub fn filter_tasks(expr: &FilterExpr, tasks: &[Task]) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| evaluate(expr, t))
        .cloned()
        .collect()
}

pub fn parse_and_filter(query: &str, tasks: &[Task]) -> Result<Vec<Task>, QueryError> {
    let expr = parse_query(query)?;
    Ok(filter_tasks(&expr, tasks))
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum GrepStrategy {
    All,
    Tag { tag: String },
    MultiTag { tags: Vec<String> },
    IncludeState { states: HashSet<String> },
    ExcludeState { exclude: HashSet<String> },
    Property { property: String },
}

pub fn analyze_query(expr: &FilterExpr) -> GrepStrategy {
    match expr {
        FilterExpr::Tag(tag) => GrepStrategy::Tag {
            tag: tag.clone(),
        },
        FilterExpr::State(state) => GrepStrategy::IncludeState {
            states: {
                let mut s = HashSet::new();
                s.insert(state.clone());
                s
            },
        },
        FilterExpr::Not(inner) => {
            if let FilterExpr::State(st) = inner.as_ref() {
                GrepStrategy::ExcludeState {
                    exclude: {
                        let mut s = HashSet::new();
                        s.insert(st.clone());
                        s
                    },
                }
            } else {
                best_and_leaf(expr)
            }
        }
        FilterExpr::Property(key, value) => GrepStrategy::Property {
            property: match value {
                Some(v) => format!("{}={}", key, v),
                None => key.clone(),
            },
        },
        FilterExpr::Title(_) => GrepStrategy::All,
        FilterExpr::Or(left, right) => {
            let tags = collect_or_tags(expr);
            if !tags.is_empty() {
                return GrepStrategy::MultiTag { tags };
            }
            let states = collect_or_states(expr);
            if !states.is_empty() {
                return GrepStrategy::IncludeState {
                    states: states.into_iter().collect(),
                };
            }
            let left_strategy = best_and_leaf(left);
            let right_strategy = best_and_leaf(right);
            pick_better(left_strategy, right_strategy)
        }
        FilterExpr::And(_, _) => best_and_leaf(expr),
    }
}

fn collect_or_tags(expr: &FilterExpr) -> Vec<String> {
    match expr {
        FilterExpr::Tag(tag) => vec![tag.clone()],
        FilterExpr::Or(left, right) => {
            let mut tags = collect_or_tags(left);
            tags.extend(collect_or_tags(right));
            tags
        }
        _ => vec![],
    }
}

fn collect_or_states(expr: &FilterExpr) -> Vec<String> {
    match expr {
        FilterExpr::State(state) => vec![state.clone()],
        FilterExpr::Or(left, right) => {
            let mut states = collect_or_states(left);
            states.extend(collect_or_states(right));
            states
        }
        _ => vec![],
    }
}

fn best_and_leaf(expr: &FilterExpr) -> GrepStrategy {
    match expr {
        FilterExpr::Tag(tag) => GrepStrategy::Tag {
            tag: tag.clone(),
        },
        FilterExpr::State(state) => GrepStrategy::IncludeState {
            states: {
                let mut s = HashSet::new();
                s.insert(state.clone());
                s
            },
        },
        FilterExpr::Property(key, value) => GrepStrategy::Property {
            property: match value {
                Some(v) => format!("{}={}", key, v),
                None => key.clone(),
            },
        },
        FilterExpr::Not(inner) => {
            if let FilterExpr::State(st) = inner.as_ref() {
                GrepStrategy::ExcludeState {
                    exclude: {
                        let mut s = HashSet::new();
                        s.insert(st.clone());
                        s
                    },
                }
            } else {
                GrepStrategy::All
            }
        }
        FilterExpr::Or(left, right) => {
            let tags = collect_or_tags(expr);
            if !tags.is_empty() {
                return GrepStrategy::MultiTag { tags };
            }
            let states = collect_or_states(expr);
            if !states.is_empty() {
                return GrepStrategy::IncludeState {
                    states: states.into_iter().collect(),
                };
            }
            let left_strategy = best_and_leaf(left);
            let right_strategy = best_and_leaf(right);
            pick_better(left_strategy, right_strategy)
        }
        FilterExpr::And(left, right) => {
            let left_strategy = best_and_leaf(left);
            let right_strategy = best_and_leaf(right);
            pick_better(left_strategy, right_strategy)
        }
        _ => GrepStrategy::All,
    }
}

fn strategy_priority(s: &GrepStrategy) -> u8 {
    match s {
        GrepStrategy::All => 0,
        GrepStrategy::ExcludeState { .. } => 1,
        GrepStrategy::IncludeState { .. } => 2,
        GrepStrategy::Property { .. } => 3,
        GrepStrategy::Tag { .. } => 4,
        GrepStrategy::MultiTag { .. } => 5,
    }
}

fn pick_better(a: GrepStrategy, b: GrepStrategy) -> GrepStrategy {
    if strategy_priority(&a) >= strategy_priority(&b) {
        a
    } else {
        b
    }
}

pub fn strategy_regex(strategy: &GrepStrategy) -> String {
    match strategy {
        GrepStrategy::All => {
            let excluded: HashSet<String> = HashSet::new();
            pattern_get(Some(&excluded))
        }
        GrepStrategy::Tag { tag } => {
            let excluded: HashSet<String> = HashSet::new();
            let state_pattern = pattern_get(Some(&excluded));
            let tag_pattern = format!("#{}", tag);
            format!("{}|{}", state_pattern, tag_pattern)
        }
        GrepStrategy::MultiTag { tags } => {
            let excluded: HashSet<String> = HashSet::new();
            let state_pattern = pattern_get(Some(&excluded));
            let tag_patterns: Vec<String> = tags.iter().map(|t| format!("#{}", t)).collect();
            format!("{}|{}", state_pattern, tag_patterns.join("|"))
        }
        GrepStrategy::IncludeState { states } => pattern_get_include(states),
        GrepStrategy::ExcludeState { exclude } => pattern_get(Some(exclude)),
        GrepStrategy::Property { property } => {
            if property.contains('=') {
                let parts: Vec<&str> = property.splitn(2, '=').collect();
                let key = parts[0];
                let value = parts[1];
                format!(
                    r"@{}\({}(?:T[^)]*)?\)",
                    regex::escape(key),
                    regex::escape(value)
                )
            } else {
                format!("@{}\\([^)]+\\)", regex::escape(property))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use std::collections::HashSet;

use crate::task::Task;
    use indexmap::IndexMap;

    fn make_task(id: &str, title: &str, state: &str, tags: Vec<&str>, properties: Vec<(&str, &str)>) -> Task {
        Task {
            title: title.to_string(),
            state: state.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            raw: String::new(),
            body: String::new(),
            id: id.to_string(),
            parent: None,
            children: vec![],
            properties: {
                let mut map = IndexMap::new();
                for (k, v) in properties {
                    map.insert(k.to_string(), v.to_string());
                }
                map
            },
        }
    }

    #[test]
    fn test_tokenize_simple_tag() {
        let tokens = tokenize("tag:foo").unwrap();
        assert_eq!(
            tokens,
            vec![Token::TagPrefix, Token::Ident("foo".to_string())]
        );
    }

    #[test]
    fn test_tokenize_negated_tag() {
        let tokens = tokenize("-tag:foo").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Minus,
                Token::TagPrefix,
                Token::Ident("foo".to_string())
            ]
        );
    }

    #[test]
    fn test_tokenize_tag_with_underscore() {
        let tokens = tokenize("tag:foo_bar").unwrap();
        assert_eq!(
            tokens,
            vec![Token::TagPrefix, Token::Ident("foo_bar".to_string())]
        );
    }

    #[test]
    fn test_tokenize_negated_tag_with_underscore() {
        let tokens = tokenize("-tag:foo_bar").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Minus,
                Token::TagPrefix,
                Token::Ident("foo_bar".to_string())
            ]
        );
    }

    #[test]
    fn test_tokenize_property() {
        let tokens = tokenize("property:bar=baz").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::PropertyPrefix,
                Token::Ident("bar".to_string()),
                Token::Equals,
                Token::Ident("baz".to_string())
            ]
        );
    }

    #[test]
    fn test_tokenize_property_exists() {
        let tokens = tokenize("property:bar").unwrap();
        assert_eq!(
            tokens,
            vec![Token::PropertyPrefix, Token::Ident("bar".to_string())]
        );
    }

    #[test]
    fn test_tokenize_title() {
        let tokens = tokenize("title:meeting").unwrap();
        assert_eq!(
            tokens,
            vec![Token::TitlePrefix, Token::Ident("meeting".to_string())]
        );
    }

    #[test]
    fn test_tokenize_title_quoted() {
        let tokens = tokenize(r#"title:"hello world""#).unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::TitlePrefix,
                Token::QuotedString("hello world".to_string())
            ]
        );
    }

    #[test]
    fn test_tokenize_quoted_with_space() {
        let tokens = tokenize(r#"tag:"foo bar""#).unwrap();
        assert_eq!(
            tokens,
            vec![Token::TagPrefix, Token::QuotedString("foo bar".to_string())]
        );
    }

    #[test]
    fn test_tokenize_state() {
        let tokens = tokenize("state:TODO").unwrap();
        assert_eq!(
            tokens,
            vec![Token::StatePrefix, Token::Ident("TODO".to_string())]
        );
    }

    #[test]
    fn test_tokenize_and_or() {
        let tokens = tokenize("tag:foo AND tag:bar OR tag:baz").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::TagPrefix,
                Token::Ident("foo".to_string()),
                Token::And,
                Token::TagPrefix,
                Token::Ident("bar".to_string()),
                Token::Or,
                Token::TagPrefix,
                Token::Ident("baz".to_string())
            ]
        );
    }

    #[test]
    fn test_tokenize_parentheses() {
        let tokens = tokenize("(tag:foo OR tag:bar) AND property:prio=high").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Lparen,
                Token::TagPrefix,
                Token::Ident("foo".to_string()),
                Token::Or,
                Token::TagPrefix,
                Token::Ident("bar".to_string()),
                Token::Rparen,
                Token::And,
                Token::PropertyPrefix,
                Token::Ident("prio".to_string()),
                Token::Equals,
                Token::Ident("high".to_string())
            ]
        );
    }

    #[test]
    fn test_tokenize_negated_group() {
        let tokens = tokenize("-(tag:foo AND tag:bar)").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Minus,
                Token::Lparen,
                Token::TagPrefix,
                Token::Ident("foo".to_string()),
                Token::And,
                Token::TagPrefix,
                Token::Ident("bar".to_string()),
                Token::Rparen
            ]
        );
    }

    #[test]
    fn test_tokenize_unterminated_string() {
        let result = tokenize(r#"tag:"foo"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_simple_tag() {
        let expr = parse_query("tag:foo").unwrap();
        assert_eq!(expr, FilterExpr::Tag("foo".to_string()));
    }

    #[test]
    fn test_parse_negated_tag() {
        let expr = parse_query("-tag:foo").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Not(Box::new(FilterExpr::Tag("foo".to_string())))
        );
    }

    #[test]
    fn test_parse_property() {
        let expr = parse_query("property:bar=baz").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Property("bar".to_string(), Some("baz".to_string()))
        );
    }

    #[test]
    fn test_parse_property_exists() {
        let expr = parse_query("property:bar").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Property("bar".to_string(), None)
        );
    }

    #[test]
    fn test_parse_title() {
        let expr = parse_query("title:meeting").unwrap();
        assert_eq!(expr, FilterExpr::Title("meeting".to_string()));
    }

    #[test]
    fn test_parse_state() {
        let expr = parse_query("state:TODO").unwrap();
        assert_eq!(expr, FilterExpr::State("TODO".to_string()));
    }

    #[test]
    fn test_parse_and() {
        let expr = parse_query("tag:foo AND tag:bar").unwrap();
        assert_eq!(
            expr,
            FilterExpr::And(
                Box::new(FilterExpr::Tag("foo".to_string())),
                Box::new(FilterExpr::Tag("bar".to_string()))
            )
        );
    }

    #[test]
    fn test_parse_or() {
        let expr = parse_query("tag:foo OR tag:bar").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Or(
                Box::new(FilterExpr::Tag("foo".to_string())),
                Box::new(FilterExpr::Tag("bar".to_string()))
            )
        );
    }

    #[test]
    fn test_parse_nested() {
        let expr = parse_query("(tag:foo OR tag:bar) AND property:prio=high").unwrap();
        assert_eq!(
            expr,
            FilterExpr::And(
                Box::new(FilterExpr::Or(
                    Box::new(FilterExpr::Tag("foo".to_string())),
                    Box::new(FilterExpr::Tag("bar".to_string()))
                )),
                Box::new(FilterExpr::Property(
                    "prio".to_string(),
                    Some("high".to_string())
                ))
            )
        );
    }

    #[test]
    fn test_parse_and_precedence_over_or() {
        let expr = parse_query("tag:a AND tag:b OR tag:c").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Or(
                Box::new(FilterExpr::And(
                    Box::new(FilterExpr::Tag("a".to_string())),
                    Box::new(FilterExpr::Tag("b".to_string()))
                )),
                Box::new(FilterExpr::Tag("c".to_string()))
            )
        );
    }

    #[test]
    fn test_parse_negated_group() {
        let expr = parse_query("-(tag:foo AND tag:bar)").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Not(Box::new(FilterExpr::And(
                Box::new(FilterExpr::Tag("foo".to_string())),
                Box::new(FilterExpr::Tag("bar".to_string()))
            )))
        );
    }

    #[test]
    fn test_parse_negated_title() {
        let expr = parse_query("-title:meeting").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Not(Box::new(FilterExpr::Title("meeting".to_string())))
        );
    }

    #[test]
    fn test_parse_negated_state() {
        let expr = parse_query("-state:DONE").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Not(Box::new(FilterExpr::State("DONE".to_string())))
        );
    }

    #[test]
    fn test_parse_three_way_and() {
        let expr = parse_query("tag:foo AND property:bar=baz AND -tag:foo_bar").unwrap();
        assert_eq!(
            expr,
            FilterExpr::And(
                Box::new(FilterExpr::And(
                    Box::new(FilterExpr::Tag("foo".to_string())),
                    Box::new(FilterExpr::Property(
                        "bar".to_string(),
                        Some("baz".to_string())
                    ))
                )),
                Box::new(FilterExpr::Not(Box::new(FilterExpr::Tag(
                    "foo_bar".to_string()
                ))))
            )
        );
    }

    #[test]
    fn test_parse_empty() {
        let result = parse_query("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_whitespace() {
        let result = parse_query("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_prefix() {
        let result = parse_query("foo:bar");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_rparen() {
        let result = parse_query("(tag:foo");
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_tag() {
        let task = make_task("t:1", "task one", "TODO", vec!["foo", "bar"], vec![]);
        assert!(evaluate(&FilterExpr::Tag("foo".to_string()), &task));
        assert!(evaluate(&FilterExpr::Tag("bar".to_string()), &task));
        assert!(!evaluate(&FilterExpr::Tag("baz".to_string()), &task));
    }

    #[test]
    fn test_evaluate_not_tag() {
        let task = make_task("t:1", "task one", "TODO", vec!["foo"], vec![]);
        let expr = FilterExpr::Not(Box::new(FilterExpr::Tag("foo".to_string())));
        assert!(!evaluate(&expr, &task));

        let expr2 = FilterExpr::Not(Box::new(FilterExpr::Tag("baz".to_string())));
        assert!(evaluate(&expr2, &task));
    }

    #[test]
    fn test_evaluate_property() {
        let task = make_task("t:1", "task", "TODO", vec![], vec![("priority", "high")]);
        assert!(evaluate(
            &FilterExpr::Property("priority".to_string(), Some("high".to_string())),
            &task
        ));
        assert!(!evaluate(
            &FilterExpr::Property("priority".to_string(), Some("low".to_string())),
            &task
        ));
        assert!(evaluate(
            &FilterExpr::Property("priority".to_string(), None),
            &task
        ));
        assert!(!evaluate(
            &FilterExpr::Property("scheduled".to_string(), None),
            &task
        ));
    }

    #[test]
    fn test_evaluate_property_date_strips_time() {
        let task = make_task(
            "t:1",
            "task",
            "TODO",
            vec![],
            vec![("scheduled", "2024-01-15T10:00")],
        );
        assert!(evaluate(
            &FilterExpr::Property("scheduled".to_string(), Some("2024-01-15".to_string())),
            &task
        ));
    }

    #[test]
    fn test_evaluate_title() {
        let task = make_task("t:1", "buy milk", "TODO", vec![], vec![]);
        assert!(evaluate(
            &FilterExpr::Title("milk".to_string()),
            &task
        ));
        assert!(evaluate(
            &FilterExpr::Title("MILK".to_string()),
            &task
        ));
        assert!(!evaluate(
            &FilterExpr::Title("eggs".to_string()),
            &task
        ));
    }

    #[test]
    fn test_evaluate_state() {
        let task = make_task("t:1", "task", "TODO", vec![], vec![]);
        assert!(evaluate(&FilterExpr::State("TODO".to_string()), &task));
        assert!(!evaluate(&FilterExpr::State("DONE".to_string()), &task));
    }

    #[test]
    fn test_evaluate_and() {
        let task = make_task(
            "t:1",
            "task",
            "TODO",
            vec!["foo", "bar"],
            vec![("prio", "high")],
        );
        let expr = FilterExpr::And(
            Box::new(FilterExpr::Tag("foo".to_string())),
            Box::new(FilterExpr::Tag("bar".to_string())),
        );
        assert!(evaluate(&expr, &task));

        let expr2 = FilterExpr::And(
            Box::new(FilterExpr::Tag("foo".to_string())),
            Box::new(FilterExpr::Tag("baz".to_string())),
        );
        assert!(!evaluate(&expr2, &task));
    }

    #[test]
    fn test_evaluate_or() {
        let task = make_task("t:1", "task", "TODO", vec!["foo"], vec![]);
        let expr = FilterExpr::Or(
            Box::new(FilterExpr::Tag("foo".to_string())),
            Box::new(FilterExpr::Tag("bar".to_string())),
        );
        assert!(evaluate(&expr, &task));

        let expr2 = FilterExpr::Or(
            Box::new(FilterExpr::Tag("baz".to_string())),
            Box::new(FilterExpr::Tag("qux".to_string())),
        );
        assert!(!evaluate(&expr2, &task));
    }

    #[test]
    fn test_evaluate_not() {
        let task = make_task("t:1", "task", "TODO", vec!["foo"], vec![]);
        let expr = FilterExpr::Not(Box::new(FilterExpr::Tag("foo".to_string())));
        assert!(!evaluate(&expr, &task));

        let expr2 = FilterExpr::Not(Box::new(FilterExpr::Tag("bar".to_string())));
        assert!(evaluate(&expr2, &task));
    }

    #[test]
    fn test_filter_tasks_empty_list() {
        let expr = parse_query("tag:foo").unwrap();
        let tasks: Vec<Task> = vec![];
        let result = filter_tasks(&expr, &tasks);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_tasks_multiple() {
        let t1 = make_task("t:1", "one", "TODO", vec!["foo"], vec![]);
        let t2 = make_task("t:2", "two", "DONE", vec!["bar"], vec![]);
        let t3 = make_task("t:3", "three", "IN_PROGRESS", vec!["foo", "bar"], vec![]);

        let tasks = vec![t1.clone(), t2.clone(), t3.clone()];

        let expr = parse_query("tag:foo").unwrap();
        let result = filter_tasks(&expr, &tasks);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&t1));
        assert!(result.contains(&t3));
        assert!(!result.contains(&t2));
    }

    #[test]
    fn test_parse_and_filter() {
        let t1 = make_task(
            "t:1",
            "one",
            "TODO",
            vec!["foo"],
            vec![("bar", "baz")],
        );
        let t2 = make_task(
            "t:2",
            "two",
            "TODO",
            vec!["foo", "foo_bar"],
            vec![("bar", "baz")],
        );
        let t3 = make_task(
            "t:3",
            "three",
            "TODO",
            vec!["foo"],
            vec![("bar", "other")],
        );

        let tasks = vec![t1.clone(), t2.clone(), t3.clone()];

        let result = parse_and_filter(
            "tag:foo AND property:bar=baz AND -tag:foo_bar",
            &tasks,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&t1));
    }

    #[test]
    fn test_parse_and_filter_quoted_title() {
        let t1 = make_task("t:1", "buy milk", "TODO", vec![], vec![]);
        let t2 = make_task("t:2", "buy eggs", "TODO", vec![], vec![]);

        let tasks = vec![t1.clone(), t2.clone()];

        let result = parse_and_filter(r#"title:"buy milk""#, &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&t1));
    }

    #[test]
    fn test_parse_and_filter_nested() {
        let t1 = make_task("t:1", "one", "TODO", vec!["bug"], vec![]);
        let t2 = make_task("t:2", "two", "TODO", vec!["feature"], vec![("prio", "high")]);
        let t3 = make_task("t:3", "three", "TODO", vec!["bug"], vec![("prio", "high")]);
        let t4 = make_task("t:4", "four", "TODO", vec!["feature"], vec![]);

        let tasks = vec![t1.clone(), t2.clone(), t3.clone(), t4.clone()];

        let result =
            parse_and_filter("(tag:bug OR tag:feature) AND property:prio=high", &tasks).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&t2));
        assert!(result.contains(&t3));
    }

    #[test]
    fn test_analyze_simple_tag() {
        let expr = parse_query("tag:foo").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::Tag { tag: "foo".to_string() });
    }

    #[test]
    fn test_analyze_simple_state() {
        let expr = parse_query("state:TODO").unwrap();
        let strategy = analyze_query(&expr);
        let mut expected = HashSet::new();
        expected.insert("TODO".to_string());
        assert_eq!(strategy, GrepStrategy::IncludeState { states: expected });
    }

    #[test]
    fn test_analyze_negated_state() {
        let expr = parse_query("-state:DONE").unwrap();
        let strategy = analyze_query(&expr);
        let mut expected = HashSet::new();
        expected.insert("DONE".to_string());
        assert_eq!(strategy, GrepStrategy::ExcludeState { exclude: expected });
    }

    #[test]
    fn test_analyze_simple_property() {
        let expr = parse_query("property:prio").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::Property { property: "prio".to_string() });
    }

    #[test]
    fn test_analyze_property_with_value() {
        let expr = parse_query("property:prio=high").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::Property { property: "prio=high".to_string() });
    }

    #[test]
    fn test_analyze_or_tags() {
        let expr = parse_query("tag:foo OR tag:bar").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::MultiTag { tags: vec!["foo".to_string(), "bar".to_string()] });
    }

    #[test]
    fn test_analyze_or_states() {
        let expr = parse_query("state:TODO OR state:DONE").unwrap();
        let strategy = analyze_query(&expr);
        let mut expected = HashSet::new();
        expected.insert("TODO".to_string());
        expected.insert("DONE".to_string());
        assert_eq!(strategy, GrepStrategy::IncludeState { states: expected });
    }

    #[test]
    fn test_analyze_and_picks_tag_over_property() {
        let expr = parse_query("tag:foo AND property:prio=high").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::Tag { tag: "foo".to_string() });
    }

    #[test]
    fn test_analyze_title_returns_all() {
        let expr = parse_query("title:meeting").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::All);
    }

    #[test]
    fn test_analyze_negated_tag_returns_all() {
        let expr = parse_query("-tag:foo").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::All);
    }

    #[test]
    fn test_analyze_mixed_or_returns_tag() {
        let expr = parse_query("tag:foo OR state:TODO").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::MultiTag { tags: vec!["foo".to_string()] });
    }

    #[test]
    fn test_analyze_complex_and_or() {
        let expr = parse_query("(tag:foo OR tag:bar) AND property:prio=high").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::MultiTag { tags: vec!["foo".to_string(), "bar".to_string()] });
    }

    #[test]
    fn test_evaluate_property_datetime_strips_time() {
        let task = make_task(
            "t:1",
            "task",
            "TODO",
            vec![],
            vec![("scheduled", "2024-01-15T10:30")],
        );
        let expr = parse_query("property:scheduled=2024-01-15").unwrap();
        let result = filter_tasks(&expr, &[task.clone()]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_property_datetime_value() {
        let expr = parse_query("property:scheduled=2024-01-15T10:30").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Property(
                "scheduled".to_string(),
                Some("2024-01-15T10:30".to_string())
            )
        );
    }

    #[test]
    fn test_parse_property_date_value() {
        let expr = parse_query("property:scheduled=2024-01-15").unwrap();
        assert_eq!(
            expr,
            FilterExpr::Property(
                "scheduled".to_string(),
                Some("2024-01-15".to_string())
            )
        );
    }
}
