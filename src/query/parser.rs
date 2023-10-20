use unic_ucd_category::GeneralCategory;

use crate::query::expr::Expr;

type Result<T> = std::result::Result<T, ()>;

fn is_special_char(c: char) -> bool {
    match c {
        '(' | ')' | ':' | '\'' | '"' => true,
        _ => false,
    }
}

fn is_word_char(c: char) -> bool {
    !is_whitespace(c) && !is_special_char(c)
}

fn is_whitespace(c: char) -> bool {
    if is_special_char(c) {
        return false;
    }

    use GeneralCategory::*;

    match GeneralCategory::of(c) {
        OpenPunctuation | ClosePunctuation | InitialPunctuation | FinalPunctuation
        | OtherPunctuation | SpaceSeparator | LineSeparator | ParagraphSeparator => true,
        _ => false,
    }
}

fn is_keyword(s: &str) -> bool {
    match s {
        "AND" | "THEN" | "OR" | "NOT" => true,
        _ => false,
    }
}

struct Input<'a> {
    full: &'a str,
    remaining: &'a str,
    position: usize,
    offset: usize,
}

impl Input<'_> {
    fn has_remaining(&self) -> bool {
        !self.remaining.is_empty()
    }

    fn peek(&self) -> Option<char> {
        self.remaining.chars().next()
    }

    fn next(&mut self) -> Option<char> {
        let char = self.peek()?;
        let len = char.len_utf8();

        self.remaining = &self.remaining[len..];
        self.position += 1;
        self.offset += len;

        Some(char)
    }

    fn next_expect(&mut self, pred: impl FnOnce(char) -> bool) -> Result<char> {
        let val = self.next();

        match val {
            Some(c) if pred(c) => Ok(c),
            _ => Err(()),
        }
    }

    fn skip(&mut self, n: usize) {
        for _ in 0..n {
            self.next();
        }
    }
}

fn parse_word(input: &mut Input<'_>) -> String {
    let start_offset = input.offset;
    let mut end_offset = start_offset;

    while input.peek().filter(|&c| is_word_char(c)).is_some() {
        input.next();
        end_offset = input.offset;
    }

    input.full[start_offset..end_offset].to_owned()
}

fn parse_string(input: &mut Input<'_>) -> Result<String> {
    let mut string = String::new();
    let quotation_mark = input.next_expect(|c| c == '\'' || c == '"')?;

    while let Some(c) = input.peek() {
        match c {
            '\\' => {
                input.next();
                let next = input.peek();

                if next == Some('\'') || next == Some('"') || next == Some('\\') {
                    input.next();
                    string.push(next.unwrap());
                } else {
                    string.push('\\');
                }
            }

            c if c == quotation_mark => break,
            c => {
                input.next();
                string.push(c)
            }
        }
    }

    if quotation_mark == '\'' {
        input.next_expect(|c| c == '\'')?;
    } else {
        input.next_expect(|c| c == '"')?;
    }

    Ok(string)
}

fn parse_string_or_word(input: &mut Input<'_>) -> Result<String> {
    let word = parse_word(input);

    if !word.is_empty() {
        Ok(word)
    } else {
        parse_string(input)
    }
}

fn parse_func(input: &mut Input<'_>) -> Result<Expr> {
    let name = parse_word(input);

    if is_keyword(&name) {
        return Err(());
    }

    if name.is_empty() {
        let string = parse_string(input)?;
        return Ok(Expr::Phrase(string));
    }

    if input.peek() != Some(':') {
        return Ok(Expr::Phrase(name));
    }

    input.next();
    let value = parse_string_or_word(input)?;

    Ok(Expr::Func(name, value))
}

fn skip_whitespace(input: &mut Input<'_>) {
    while let Some(c) = input.peek() {
        if !is_whitespace(c) {
            break;
        }

        input.next();
    }
}

fn parse_term(input: &mut Input<'_>) -> Result<Expr> {
    skip_whitespace(input);

    if input.remaining.starts_with("NOT") {
        input.skip(3);
        skip_whitespace(input);

        let inner = parse_term(input)?;
        Ok(Expr::Not(Box::new(inner)))
    } else if input.remaining.starts_with("(") {
        input.next();

        let inner = parse_expr(input)?;
        skip_whitespace(input);

        if !input.remaining.starts_with(")") {
            return Err(());
        }

        input.next();

        Ok(inner)
    } else {
        parse_func(input)
    }
}

fn parse_operator(
    input: &mut Input<'_>,
    keyword: &str,
    term: fn(&mut Input<'_>) -> Result<Expr>,
    wrap: impl FnOnce(Vec<Expr>) -> Expr,
) -> Result<Expr> {
    let mut args = vec![term(input)?];

    loop {
        skip_whitespace(input);

        if input.remaining.starts_with(keyword) {
            input.skip(keyword.len());
            args.push(term(input)?);
        } else {
            break;
        }
    }

    if args.len() == 1 {
        Ok(args.remove(0))
    } else {
        Ok(wrap(args))
    }
}

fn parse_or(input: &mut Input<'_>) -> Result<Expr> {
    parse_operator(input, "OR", parse_term, Expr::Or)
}

fn parse_and(input: &mut Input<'_>) -> Result<Expr> {
    parse_operator(input, "AND", parse_or, Expr::And)
}

fn parse_then(input: &mut Input<'_>) -> Result<Expr> {
    parse_operator(input, "THEN", parse_and, Expr::Then)
}

fn parse_implicit_and(input: &mut Input<'_>) -> Result<Expr> {
    let mut args = vec![parse_then(input)?];

    loop {
        skip_whitespace(input);

        let r = input.remaining;
        if !r.starts_with("OR")
            && !r.starts_with("AND")
            && !r.starts_with("THEN")
            && !r.starts_with(")")
            && !r.is_empty()
        {
            args.push(parse_then(input)?);
        } else {
            break;
        }
    }

    if args.len() == 1 {
        Ok(args.remove(0))
    } else {
        Ok(Expr::And(args))
    }
}

fn parse_expr(input: &mut Input<'_>) -> Result<Expr> {
    parse_implicit_and(input)
}

pub fn parse(input: &str) -> Result<Expr> {
    let mut input = Input {
        full: input,
        remaining: input,
        position: 0,
        offset: 0,
    };

    skip_whitespace(&mut input);

    if input.has_remaining() {
        parse_expr(&mut input)
    } else {
        Ok(Expr::False)
    }
}
