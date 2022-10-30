use std::collections::HashMap;

use crate::lexer::{Token, TokenType};

#[derive(Debug, PartialEq)]
pub enum Json<'a> {
    Null,
    Bool(bool),
    Number(f64),
    String(&'a str),
    Object(Box<HashMap<&'a str, Json<'a>>>),
    Array(Box<Vec<Json<'a>>>),
}

#[derive(Debug, PartialEq)]
pub enum ParseErrorType {
    UnexpectedEnd,
    UnexpectedToken,
    DuplicateKey,
    TrailingComma,
    KeyNotInQuotes,
    MissingColon,
}

#[derive(Debug, PartialEq)]
pub struct ParseError<'a> {
    error_type: ParseErrorType,
    token: Option<&'a Token<'a>>,
    expected: Option<&'a TokenType<'a>>,
}

impl<'a> ParseError<'a> {
    pub fn new(
        error_type: ParseErrorType,
        token: Option<&'a Token<'a>>,
        expected: Option<&'a TokenType<'a>>,
    ) -> ParseError<'a> {
        ParseError {
            error_type,
            token,
            expected,
        }
    }
}

struct ParseContext<'a> {
    key: &'a str,
    value: Json<'a>,
    next: usize,
}

impl<'a> ParseContext<'a> {
    pub fn new(value: Json<'a>, next: usize) -> ParseContext<'a> {
        ParseContext {
            key: "",
            value,
            next,
        }
    }

    pub fn key_value_pair(key: &'a str, value: Json<'a>, next: usize) -> ParseContext<'a> {
        ParseContext { key, value, next }
    }
}

fn expect<'a>(
    token_type: &'a TokenType<'a>,
    error_type: ParseErrorType,
    tokens: &'a Vec<Token>,
    i: usize,
) -> Result<&'a Token<'a>, ParseError<'a>> {
    match tokens.get(i) {
        Some(token) => {
            if token.token_type == *token_type {
                Ok(&token)
            } else {
                Err(ParseError::new(error_type, Some(token), Some(&token_type)))
            }
        }
        None => Err(ParseError::new(ParseErrorType::UnexpectedEnd, None, None)),
    }
}

fn check_trailing_comma<'a>(
    last_comma: Option<&'a Token<'a>>,
    i: usize,
) -> Result<usize, ParseError<'a>> {
    match last_comma {
        Some(token) => {
            if token.col == i + 1 {
                Err(ParseError::new(
                    ParseErrorType::TrailingComma,
                    Some(token),
                    None,
                ))
            } else {
                Ok(i)
            }
        }
        None => Ok(i),
    }
}

fn for_each_comma<'a, G, B>(
    getter: G,
    mut builder: B,
    tokens: &'a Vec<Token>,
    start: usize,
) -> Result<usize, ParseError<'a>>
where
    G: Fn(&'a Vec<Token>, usize) -> Result<ParseContext<'a>, ParseError<'a>>,
    B: FnMut(ParseContext<'a>, Option<&'a Token<'a>>) -> Result<(), ParseError<'a>>,
{
    let mut i = start;
    let mut last_comma: Option<&'a Token<'a>> = None;

    loop {
        match getter(tokens, i) {
            Ok(parse_context) => {
                let next = parse_context.next;

                if let Err(parse_error) = builder(parse_context, tokens.get(i)) {
                    return Err(parse_error);
                };

                i = next;

                match expect(
                    &TokenType::Comma,
                    ParseErrorType::UnexpectedToken,
                    tokens,
                    i,
                ) {
                    Ok(token) => {
                        last_comma = Some(token);
                        i += 1;
                    }
                    Err(_) => break,
                }
            }
            Err(parse_error) => match parse_error.error_type {
                ParseErrorType::UnexpectedToken => break,
                _ => return Err(parse_error),
            },
        }
    }

    check_trailing_comma(last_comma, i)
}

fn expect_key<'a>(tokens: &'a Vec<Token>, i: usize) -> Result<&'a str, ParseError<'a>> {
    match tokens.get(i) {
        Some(token) => match token.token_type {
            TokenType::String(s) => Ok(s),
            _ => Err(ParseError::new(
                ParseErrorType::KeyNotInQuotes,
                Some(token),
                None,
            )),
        },
        None => Err(ParseError::new(ParseErrorType::UnexpectedEnd, None, None)),
    }
}

fn key_value_pair<'a>(
    tokens: &'a Vec<Token>,
    start: usize,
) -> Result<ParseContext<'a>, ParseError<'a>> {
    let key = match expect_key(tokens, start) {
        Ok(k) => k,
        Err(parse_error) => return Err(parse_error),
    };

    expect(
        &TokenType::Colon,
        ParseErrorType::MissingColon,
        tokens,
        start + 1,
    )?;

    let value_parse_context = value(tokens, start + 2)?;

    return Ok(ParseContext::key_value_pair(
        key,
        value_parse_context.value,
        value_parse_context.next,
    ));
}

fn object<'a>(tokens: &'a Vec<Token>, start: usize) -> Result<ParseContext<'a>, ParseError<'a>> {
    let mut object = HashMap::new();
    let builder = |parse_context: ParseContext<'a>, token: Option<&'a Token<'a>>| match object
        .insert(parse_context.key, parse_context.value)
    {
        Some(_) => return Err(ParseError::new(ParseErrorType::DuplicateKey, token, None)),
        None => Ok(()),
    };

    let i = match for_each_comma(key_value_pair, builder, tokens, start + 1) {
        Ok(next) => next,
        Err(parse_error) => return Err(parse_error),
    };

    let value = Json::Object(Box::new(object));

    match expect(
        &TokenType::CloseCurly,
        ParseErrorType::UnexpectedToken,
        tokens,
        i,
    ) {
        Ok(_) => Ok(ParseContext::new(value, i + 1)),
        Err(parse_error) => return Err(parse_error),
    }
}

fn array<'a>(tokens: &'a Vec<Token>, start: usize) -> Result<ParseContext<'a>, ParseError<'a>> {
    let mut array = vec![];
    let builder = |parse_context: ParseContext<'a>, _| Ok(array.push(parse_context.value));

    let i = match for_each_comma(value, builder, tokens, start + 1) {
        Ok(next) => next,
        Err(parse_error) => return Err(parse_error),
    };

    let value = Json::Array(Box::new(array));

    match expect(
        &TokenType::CloseSquare,
        ParseErrorType::UnexpectedToken,
        tokens,
        i,
    ) {
        Ok(_) => Ok(ParseContext::new(value, i + 1)),
        Err(parse_error) => Err(parse_error),
    }
}

fn value<'a>(tokens: &'a Vec<Token>, start: usize) -> Result<ParseContext<'a>, ParseError<'a>> {
    let start_token = match tokens.get(start) {
        Some(token) => token,
        None => return Err(ParseError::new(ParseErrorType::UnexpectedEnd, None, None)),
    };

    match start_token.token_type {
        TokenType::Null => Ok(ParseContext::new(Json::Null, start + 1)),
        TokenType::Bool(x) => Ok(ParseContext::new(Json::Bool(x), start + 1)),
        TokenType::Number(x) => Ok(ParseContext::new(Json::Number(x), start + 1)),
        TokenType::String(x) => Ok(ParseContext::new(Json::String(x), start + 1)),
        TokenType::OpenCurly => object(tokens, start),
        TokenType::OpenSquare => array(tokens, start),
        _ => Err(ParseError::new(
            ParseErrorType::UnexpectedToken,
            Some(start_token),
            None,
        )),
    }
}

pub fn parse<'a>(tokens: &'a Vec<Token>) -> Result<Json<'a>, ParseError<'a>> {
    match value(tokens, 0) {
        Ok(parse_context) => Ok(parse_context.value),
        Err(parse_error) => Err(parse_error),
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::{self};

    use super::*;

    #[test]
    fn test_value_unexpected_token() {
        let cases = vec![":", ",", "}", "]", "hello"];

        for raw in cases {
            let tokens = lexer::lex(raw);
            let value = parse(&tokens);

            let expected = Err(ParseError::new(
                ParseErrorType::UnexpectedToken,
                Some(&tokens[0]),
                None,
            ));

            assert_eq!(value, expected);
        }
    }

    fn assert_case<'a>(
        case: &'static str,
        actual: Result<Json<'a>, ParseError<'a>>,
        expected: Result<Json<'a>, ParseError<'a>>,
    ) {
        if actual != expected {
            panic!(
                "\nFailed test case {}\nactual: {:?}\nexpected: {:?}\n",
                case, actual, expected
            );
        }
    }

    #[test]
    fn test_parse() {
        let cases = vec![
            (
                "",
                Err(ParseError::new(ParseErrorType::UnexpectedEnd, None, None)),
            ),
            (
                "[1  , 2",
                Err(ParseError::new(ParseErrorType::UnexpectedEnd, None, None)),
            ),
            (
                "[1, 2,  ",
                Err(ParseError::new(ParseErrorType::UnexpectedEnd, None, None)),
            ),
            ("null  ", Ok(Json::Null)),
            ("true", Ok(Json::Bool(true))),
            ("  false ", Ok(Json::Bool(false))),
            (" 1234", Ok(Json::Number(1234.0))),
            ("\"foo\"", Ok(Json::String("foo"))),
            (
                "{\"foo\":{   \"bar\":1234}   }",
                Ok(Json::Object(Box::new(HashMap::from([(
                    "foo",
                    Json::Object(Box::new(HashMap::from([("bar", Json::Number(1234.0))]))),
                )])))),
            ),
            (
                "{\"foo\":{   \"bar\":1234},  \"another\": \"testing\" }",
                Ok(Json::Object(Box::new(HashMap::from([
                    (
                        "foo",
                        Json::Object(Box::new(HashMap::from([("bar", Json::Number(1234.0))]))),
                    ),
                    ("another", Json::String("testing")),
                ])))),
            ),
            (
                "[1,   2,3  ,  4]",
                Ok(Json::Array(Box::new(vec![
                    Json::Number(1.0),
                    Json::Number(2.0),
                    Json::Number(3.0),
                    Json::Number(4.0),
                ]))),
            ),
        ];

        for case in cases {
            let (raw, expected) = case;
            let tokens = lexer::lex(raw);
            let value = parse(&tokens);

            assert_case(raw, value, expected)
        }
    }

    #[test]
    fn test_parse_located_error() {
        let cases: Vec<(&str, ParseErrorType, usize, Option<&TokenType>)> = vec![
            (
                "[ 1, 2, 3 4]",
                ParseErrorType::UnexpectedToken,
                6,
                Some(&TokenType::CloseSquare),
            ),
            ("[1, 2, 3,]", ParseErrorType::TrailingComma, 6, None),
            (
                "{\"foo\":123, \"foo\": 432}",
                ParseErrorType::DuplicateKey,
                5,
                None,
            ),
            (
                "{\"foo\":123, foo: 432}",
                ParseErrorType::KeyNotInQuotes,
                5,
                None,
            ),
            (
                "{\"foo\" 123}",
                ParseErrorType::MissingColon,
                2,
                Some(&TokenType::Colon),
            ),
        ];

        for case in cases {
            let (raw, expected_error, token_location, expected_token_type) = case;

            let tokens = lexer::lex(raw);
            let expected = Err(ParseError::new(
                expected_error,
                Some(tokens.get(token_location).unwrap()),
                expected_token_type,
            ));

            let value = parse(&tokens);

            assert_case(raw, value, expected)
        }
    }
}
