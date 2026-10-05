//! Tokens into a tree.
//!
//! ```text
//! whole      := sum [ `in` target ]
//! sum        := term { (+|-) term }
//! term       := juxtaposed { (*|/) juxtaposed | `of` juxtaposed }
//! juxtaposed := signed { primary-that-starts-a-quantity }     `10 km`, `2 pi`
//! signed     := (+|-) signed | power
//! power      := postfix [ ^ signed ]                          right-associative
//! postfix    := primary { ! | % | ² | ³ }
//! primary    := number | unit | currency | constant | function argument
//!             | ( whole ) | date | time | now | today | roman
//! ```
//!
//! # Juxtaposition binds tighter than `/`
//!
//! So `10 km / 2 h` is five kilometres an hour, and `km/h` is a speed. The one
//! exception is a fraction of two bare numbers before a unit: `1/2 cup` is half
//! a cup, as everybody reads it, not one over two cups.

use crate::lex::{Kind, Token};
use crate::rates::Currency;
use crate::units::{Constant, Function, Means, Radix, UnitId};
use crate::{BigRational, Refusal, Why};

/// A node of the tree, and where its text starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub at: usize,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Number(BigRational, Radix),
    Unit(UnitId),
    Currency(Currency),
    Constant(Constant),
    Negate(Box<Node>),
    Add(Box<Node>, Box<Node>),
    Subtract(Box<Node>, Box<Node>),
    Multiply(Box<Node>, Box<Node>),
    Divide(Box<Node>, Box<Node>),
    Power(Box<Node>, Box<Node>),
    Percent(Box<Node>),
    Factorial(Box<Node>),
    Call(Function, Box<Node>),
    Date(i64, u32, u32),
    Time(u32, u32, u32),
    Now,
    Today,
    Roman(u32),
    Convert(Box<Node>, Target),
}

/// What `in` converts to.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A unit, or an expression of units (`km/h`, `m²`).
    Units(Box<Node>),
    /// A base to show the number in.
    Radix(Radix),
    /// A time zone, by the name typed (`Tokyo`, `America/Sao_Paulo`, `UTC`).
    Zone(String),
}

struct Parser<'a> {
    tokens: &'a [Token],
    text: &'a str,
    i: usize,
}

pub fn parse(tokens: &[Token], text: &str) -> Result<Node, Refusal> {
    let mut p = Parser { tokens, text, i: 0 };
    if tokens.is_empty() {
        return Err(Refusal {
            at: 0,
            why: Why::Empty,
        });
    }
    let node = p.whole()?;
    match p.peek() {
        None => Ok(node),
        Some(t) => Err(p.refused(t)),
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.i)
    }

    fn peek_kind(&self) -> Option<&Kind> {
        self.peek().map(|t| &t.kind)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn is_op(&self, c: char) -> bool {
        self.peek_kind() == Some(&Kind::Op(c))
    }

    fn at_end(&self) -> usize {
        self.text.len()
    }

    /// The refusal for a token that does not belong where it is.
    fn refused(&self, t: &Token) -> Refusal {
        let why = match &t.kind {
            Kind::Word(_) => Why::UnknownWord,
            Kind::Op(')') => Why::Unbalanced,
            _ => Why::Unexpected,
        };
        Refusal { at: t.at, why }
    }

    fn whole(&mut self) -> Result<Node, Refusal> {
        let at = self.peek().map_or(0, |t| t.at);
        let value = self.sum()?;
        if self.peek_kind() == Some(&Kind::Means(Means::Into)) {
            let into = self.next().map_or(0, |t| t.end);
            let target = self.target(into)?;
            return Ok(Node {
                at,
                kind: NodeKind::Convert(Box::new(value), target),
            });
        }
        Ok(value)
    }

    fn target(&mut self, after: usize) -> Result<Target, Refusal> {
        let Some(first) = self.peek().cloned() else {
            return Err(Refusal {
                at: after,
                why: Why::Empty,
            });
        };
        let last = self.tokens.len() == self.i + 1;
        if let Kind::Means(Means::Radix(r)) = first.kind
            && last
        {
            self.i += 1;
            return Ok(Target::Radix(r));
        }
        // `UTC` has the shape of a currency code and is a zone.
        if let Kind::Currency(c) = first.kind
            && last
            && matches!(c.code(), "UTC" | "GMT")
        {
            self.i += 1;
            return Ok(Target::Zone(c.code().to_owned()));
        }
        // A UNIT EXPRESSION when it parses as one to the end; a zone's name
        // otherwise (`New York` is two words the table does not know).
        let start = self.i;
        if !matches!(first.kind, Kind::Word(_))
            && let Ok(node) = self.sum()
            && self.peek().is_none()
        {
            return Ok(Target::Units(Box::new(node)));
        }
        self.i = start;
        let rest = self.text[first.at..].trim();
        let has_word = self.tokens[start..]
            .iter()
            .any(|t| matches!(t.kind, Kind::Word(_)));
        if !has_word {
            // It was meant as units and does not parse: say where.
            self.sum()?;
            return Err(self.peek().map_or(
                Refusal {
                    at: self.at_end(),
                    why: Why::Unexpected,
                },
                |t| self.refused(t),
            ));
        }
        self.i = self.tokens.len();
        Ok(Target::Zone(rest.to_owned()))
    }

    fn sum(&mut self) -> Result<Node, Refusal> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek_kind() {
                Some(Kind::Op('+')) => '+',
                Some(Kind::Op('-')) => '-',
                _ => return Ok(left),
            };
            self.i += 1;
            let right = self.term()?;
            let at = left.at;
            left = Node {
                at,
                kind: if op == '+' {
                    NodeKind::Add(Box::new(left), Box::new(right))
                } else {
                    NodeKind::Subtract(Box::new(left), Box::new(right))
                },
            };
        }
    }

    fn term(&mut self) -> Result<Node, Refusal> {
        let mut left = self.juxtaposed()?;
        loop {
            let at = left.at;
            match self.peek_kind() {
                Some(Kind::Op('*')) => {
                    self.i += 1;
                    let right = self.juxtaposed()?;
                    left = Node {
                        at,
                        kind: NodeKind::Multiply(Box::new(left), Box::new(right)),
                    };
                }
                Some(Kind::Op('/')) => {
                    self.i += 1;
                    // `1/2 cup`: a fraction of two bare numbers, then the unit.
                    let fraction = matches!(left.kind, NodeKind::Number(..))
                        && matches!(self.peek_kind(), Some(Kind::Number(..)));
                    if fraction {
                        let right = self.signed()?;
                        left = Node {
                            at,
                            kind: NodeKind::Divide(Box::new(left), Box::new(right)),
                        };
                        left = self.juxtapose(left)?;
                    } else {
                        let right = self.juxtaposed()?;
                        left = Node {
                            at,
                            kind: NodeKind::Divide(Box::new(left), Box::new(right)),
                        };
                    }
                }
                // `OF` TAKES A PERCENTAGE OF SOMETHING (`20% of 150`), and
                // nothing else: `5 de 10` is "five out of ten" to a
                // Portuguese reader, and a product would answer 50.
                Some(Kind::Means(Means::Of)) => {
                    let percent = |n: &Node| matches!(n.kind, NodeKind::Percent(_));
                    let negated = matches!(&left.kind, NodeKind::Negate(n) if percent(n));
                    if !percent(&left) && !negated {
                        return Err(Refusal {
                            at: self.tokens[self.i].at,
                            why: Why::Unexpected,
                        });
                    }
                    self.i += 1;
                    let right = self.juxtaposed()?;
                    left = Node {
                        at,
                        kind: NodeKind::Multiply(Box::new(left), Box::new(right)),
                    };
                }
                _ => return Ok(left),
            }
        }
    }

    fn juxtaposed(&mut self) -> Result<Node, Refusal> {
        let left = self.signed()?;
        self.juxtapose(left)
    }

    /// What follows a quantity without an operator multiplies it: a unit, a
    /// currency, a constant, a function, a parenthesis.
    fn juxtapose(&mut self, mut left: Node) -> Result<Node, Refusal> {
        while matches!(
            self.peek_kind(),
            Some(
                Kind::Unit(_)
                    | Kind::Currency(_)
                    | Kind::Means(Means::Constant(_) | Means::Function(_))
                    | Kind::Op('(')
            )
        ) {
            let at = left.at;
            let right = self.power()?;
            left = Node {
                at,
                kind: NodeKind::Multiply(Box::new(left), Box::new(right)),
            };
        }
        Ok(left)
    }

    fn signed(&mut self) -> Result<Node, Refusal> {
        let at = self.peek().map_or(self.at_end(), |t| t.at);
        if self.is_op('-') {
            self.i += 1;
            let inner = self.signed()?;
            return Ok(Node {
                at,
                kind: NodeKind::Negate(Box::new(inner)),
            });
        }
        if self.is_op('+') {
            self.i += 1;
            return self.signed();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Node, Refusal> {
        let base = self.postfix()?;
        if self.is_op('^') {
            self.i += 1;
            let exponent = self.signed()?;
            let at = base.at;
            return Ok(Node {
                at,
                kind: NodeKind::Power(Box::new(base), Box::new(exponent)),
            });
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Result<Node, Refusal> {
        let mut node = self.primary()?;
        loop {
            let at = node.at;
            let squared = |n: i64| Node {
                at,
                kind: NodeKind::Number(BigRational::from_integer(n.into()), Radix::Decimal),
            };
            node = match self.peek_kind() {
                Some(Kind::Op('!')) => Node {
                    at,
                    kind: NodeKind::Factorial(Box::new(node)),
                },
                Some(Kind::Op('%')) => Node {
                    at,
                    kind: NodeKind::Percent(Box::new(node)),
                },
                Some(Kind::Op('²')) => Node {
                    at,
                    kind: NodeKind::Power(Box::new(node), Box::new(squared(2))),
                },
                Some(Kind::Op('³')) => Node {
                    at,
                    kind: NodeKind::Power(Box::new(node), Box::new(squared(3))),
                },
                _ => return Ok(node),
            };
            self.i += 1;
        }
    }

    fn primary(&mut self) -> Result<Node, Refusal> {
        let Some(t) = self.next() else {
            return Err(Refusal {
                at: self.at_end(),
                why: Why::Unexpected,
            });
        };
        let at = t.at;
        let kind = match t.kind {
            Kind::Number(n, r) => NodeKind::Number(n, r),
            Kind::Unit(u) => NodeKind::Unit(u),
            Kind::Currency(c) => {
                // `$10`, `R$ 50`: a sign before the amount.
                if matches!(self.peek_kind(), Some(Kind::Number(..))) {
                    let amount = self.postfix()?;
                    NodeKind::Multiply(
                        Box::new(amount),
                        Box::new(Node {
                            at,
                            kind: NodeKind::Currency(c),
                        }),
                    )
                } else {
                    NodeKind::Currency(c)
                }
            }
            Kind::Means(Means::Constant(c)) => NodeKind::Constant(c),
            Kind::Means(Means::Function(f)) => {
                let argument = if self.is_op('(') {
                    self.primary()?
                } else {
                    self.juxtaposed()?
                };
                NodeKind::Call(f, Box::new(argument))
            }
            Kind::Means(Means::Now) => NodeKind::Now,
            Kind::Means(Means::Today) => NodeKind::Today,
            Kind::Op('(') => {
                let inner = self.whole()?;
                if !self.is_op(')') {
                    return Err(Refusal {
                        at,
                        why: Why::Unbalanced,
                    });
                }
                self.i += 1;
                return Ok(Node { at, ..inner });
            }
            Kind::Date(y, m, d) => NodeKind::Date(y, m, d),
            Kind::Time(h, m, s) => NodeKind::Time(h, m, s),
            Kind::Roman(n) => NodeKind::Roman(n),
            _ => return Err(self.refused(&t)),
        };
        Ok(Node { at, kind })
    }
}
