use std::{
    fmt::{self, Write},
    fs::File,
    io::Read,
};

use ariadne::{Color, Source};
use chumsky::{
    pratt::{infix, left},
    prelude::*,
    text::{inline_whitespace, newline, whitespace},
};
use rust_decimal::Decimal;

#[derive(Debug)]
enum Line {
    Operation {
        operation: Operation,
        comment: String,
    },
    Subtotal {
        value: Option<Value>,
        comment: String,
    },
}

#[derive(Debug, Clone, Copy)]
enum Value {
    Number(Decimal),
    Interval(Decimal, Decimal),
}

#[derive(Debug)]
enum Operation {
    Mul(Box<Operation>, Box<Operation>),
    Div(Box<Operation>, Box<Operation>),
    Value(Value),
}

impl Value {
    fn sub(self, value: Value) -> Value {
        match (self, value) {
            (Value::Number(n), Value::Number(m)) => Value::Number(n - m),
            (Value::Number(n), Value::Interval(a, b)) => Value::Interval(n - b, n - a),
            (Value::Interval(a, b), Value::Number(n)) => Value::Interval(a - n, b - n),
            (Value::Interval(a, b), Value::Interval(c, d)) => Value::Interval(a - d, b - c),
        }
    }

    fn mul(&self, r: Value) -> Value {
        match (self, r) {
            (Value::Number(n), Value::Number(m)) => Value::Number(n * m),
            (Value::Number(n), Value::Interval(a, b)) => Value::Interval(n * a, n * b),
            (Value::Interval(a, b), Value::Number(n)) => Value::Interval(a * n, b * n),
            (Value::Interval(a, b), Value::Interval(c, d)) => {
                if *a >= 0.into() && c >= 0.into() {
                    Value::Interval(a * c, b * d)
                } else {
                    unimplemented!()
                }
            }
        }
    }

    fn div(&self, r: Value) -> Value {
        match (self, r) {
            (Value::Number(n), Value::Number(m)) => Value::Number(n / m),
            (Value::Number(n), Value::Interval(a, b)) => Value::Interval(n / a, n / b),
            (Value::Interval(a, b), Value::Number(n)) => Value::Interval(a / n, b / n),
            (Value::Interval(_, _), Value::Interval(_, _)) => {
                unimplemented!()
            }
        }
    }
}

fn parse_value<'a>() -> impl Parser<'a, &'a str, Value, extra::Err<Rich<'a, char>>> {
    let number = just('-')
        .or_not()
        .then(text::int(10))
        .then(just('.').then(text::digits(10)).or_not())
        .to_slice()
        .map(|s: &str| s.parse().unwrap())
        .boxed();

    let interval = number
        .clone()
        .then_ignore(just(',').padded())
        .then(number.clone())
        .padded_by(inline_whitespace())
        .delimited_by(just('['), just(']'));

    choice((
        number.map(Value::Number).labelled("number"),
        interval.map(|(a, b)| Value::Interval(a, b)).labelled("interval"),
    ))
}

// This can swallow useful error messages so some fix would be needed int the future
fn parse_operation<'a>() -> impl Parser<'a, &'a str, Operation, extra::Err<Rich<'a, char>>> {
    let value = inline_whitespace()
        .ignore_then(parse_value())
        .map(Operation::Value);

    value.pratt((
        infix(
            left(1),
            inline_whitespace().ignore_then(just('*')),
            |l, r| Operation::Mul(Box::new(l), Box::new(r)),
        ),
        infix(
            left(1),
            inline_whitespace().ignore_then(just('/')),
            |l, r| Operation::Div(Box::new(l), Box::new(r)),
        ),
    ))
}

fn parse_subtotal<'a>() -> impl Parser<'a, &'a str, Line, extra::Err<Rich<'a, char>>> {
    let subtotal_line = one_of("-")
        .ignored()
        .repeated()
        .ignore_then(
            inline_whitespace().then(newline()).labelled("result line")
        );

    let comment = none_of("\n")
        .ignored()
        .repeated()
        .to_slice()
        .map(ToString::to_string);

    let value_comment = inline_whitespace().at_least(1).ignore_then(comment.clone());

    let no_value = comment.padded_by(inline_whitespace()).map(|c| (None, c));

    let value = parse_value()
        .map(Some)
        .then(value_comment.or_not().map(|a| a.unwrap_or_default()));

    let result_line = choice((value, no_value));
    subtotal_line
        .ignore_then(result_line)
        .map(|(v, c)| Line::Subtotal {
            value: v,
            comment: c,
        })
}

fn parse_operation_line<'a>() -> impl Parser<'a, &'a str, Line, extra::Err<Rich<'a, char>>> {
    let value = parse_operation();

    let comment = inline_whitespace()
        .at_least(1)
        .labelled("space")
        .ignore_then(none_of("\n").ignored().repeated().to_slice().labelled("comment"))
        .map(ToString::to_string)
        .or_not();

    value.then(comment).map(|(v, comment)| Line::Operation {
        operation: v,
        comment: comment.unwrap_or(String::new()),
    })
}

fn parse_line<'a>() -> impl Parser<'a, &'a str, Line, extra::Err<Rich<'a, char>>> {
    choice((parse_operation_line(), parse_subtotal()))
}

fn pretty_print_value(fmt: &mut impl Write, v: Value) -> fmt::Result {
    match v {
        Value::Number(n) => write!(fmt, "{}", n.round_dp(2).normalize()),
        Value::Interval(a, b) => write!(
            fmt,
            "[{}, {}]",
            a.round_dp(2).normalize(),
            b.round_dp(2).normalize()
        ),
    }
}

fn pretty_print_operation(fmt: &mut impl Write, op: &Operation) -> fmt::Result {
    match op {
        Operation::Mul(l, r) => {
            pretty_print_operation(fmt, l)?;
            write!(fmt, " * ")?;
            pretty_print_operation(fmt, r)
        }
        Operation::Div(l, r) => {
            pretty_print_operation(fmt, l)?;
            write!(fmt, " / ")?;
            pretty_print_operation(fmt, r)
        }
        Operation::Value(v) => pretty_print_value(fmt, *v),
    }
}

fn pretty_print(lines: Vec<Line>) -> Result<String, std::fmt::Error> {
    let lhs: Vec<_> = lines
        .iter()
        .map(|line| match line {
            Line::Operation { operation, .. } => {
                let mut out = String::new();
                pretty_print_operation(&mut out, operation).unwrap();
                Some(out)
            }
            Line::Subtotal { value, .. } => value.map(|value| {
                let mut out = String::new();
                pretty_print_value(&mut out, value).unwrap();
                out
            }),
        })
        .collect();

    let lhs_col = lhs
        .iter()
        .map(|l| l.as_ref().map(|l| l.len()).unwrap_or(0))
        .max()
        .unwrap_or(0);

    let mut s = String::new();
    for (lhs, line) in lhs.into_iter().zip(lines) {
        match line {
            Line::Operation { comment, .. } => {
                writeln!(
                    &mut s,
                    "{:>width$} {}",
                    lhs.unwrap(),
                    comment,
                    width = lhs_col
                )?;
            }
            Line::Subtotal { comment, .. } => {
                writeln!(&mut s, "{:-<width$}", "", width = lhs_col)?;

                let lhs = if let Some(v) = lhs { v } else { String::new() };
                writeln!(&mut s, "{:>width$} {comment}", lhs, width = lhs_col)?;
                writeln!(&mut s)?;
            }
        }
    }
    Ok(s)
}

fn evaluate_operation(op: &Operation) -> Value {
    match op {
        Operation::Mul(l, r) => {
            let l = evaluate_operation(l);
            let r = evaluate_operation(r);

            l.mul(r)
        }
        Operation::Div(l, r) => {
            let l = evaluate_operation(l);
            let r = evaluate_operation(r);

            l.div(r)
        }
        Operation::Value(v) => *v,
    }
}

fn evaluate(lines: &mut [Line]) {
    if lines.is_empty() {
        return;
    }
    if matches!(lines[0], Line::Subtotal { .. }) {
        return;
    };

    let mut accu = match &lines[0] {
        Line::Operation { operation, .. } => evaluate_operation(&operation),
        Line::Subtotal { .. } => return,
    };

    for l in &mut lines[1..] {
        match l {
            Line::Operation { operation, .. } => accu = accu.sub(evaluate_operation(operation)),
            Line::Subtotal { value, .. } => *value = Some(accu),
        }
    }
}

fn main() -> std::io::Result<()> {
    let Some(arg) = std::env::args().nth(1) else {
        return Ok(());
    };

    let mut file = File::open(arg.clone())?;
    let mut buf = String::new();
    File::read_to_string(&mut file, &mut buf)?;

    let parse_result = parse_line()
        .then_ignore(whitespace())
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .parse(&buf)
        .into_result();

    match parse_result {
        Ok(mut file) => {
            evaluate(&mut file);
            let f = pretty_print(file).unwrap();
            println!("{f}")
        }
        Err(errs) => {
            errs.into_iter().for_each(|e| {
                ariadne::Report::build(ariadne::ReportKind::Error, &arg[..], e.span().start)
                    .with_message(e.to_string())
                    .with_label(
                        ariadne::Label::new((&arg[..], e.span().into_range()))
                            .with_message(e.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint((&arg[..], Source::from(&buf)))
                    .unwrap()
            });
        }
    }

    Ok(())
}
