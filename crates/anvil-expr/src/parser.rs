//! Small recursive-descent parser for arithmetic expressions.
//!
//! Grammar:
//!   expr   := term (('+' | '-') term)*
//!   term   := unary (('*' | '/') unary)*
//!   unary  := '-' unary | power
//!   power  := atom ('^' unary)?
//!   atom   := number | name | name '(' args ')' | '(' expr ')'

use crate::ExprError;

#[derive(Clone, Debug, PartialEq)]
pub enum Ast {
    Num(f64),
    Name(String),
    Neg(Box<Ast>),
    Add(Box<Ast>, Box<Ast>),
    Sub(Box<Ast>, Box<Ast>),
    Mul(Box<Ast>, Box<Ast>),
    Div(Box<Ast>, Box<Ast>),
    Pow(Box<Ast>, Box<Ast>),
    Call(String, Vec<Ast>),
}

impl Ast {
    /// Collect every parameter name referenced by this expression.
    pub fn names(&self, out: &mut Vec<String>) {
        match self {
            Ast::Num(_) => {}
            Ast::Name(n) => out.push(n.clone()),
            Ast::Neg(a) => a.names(out),
            Ast::Add(a, b) | Ast::Sub(a, b) | Ast::Mul(a, b) | Ast::Div(a, b) | Ast::Pow(a, b) => {
                a.names(out);
                b.names(out);
            }
            Ast::Call(_, args) => args.iter().for_each(|a| a.names(out)),
        }
    }
}

pub fn parse(src: &str) -> Result<Ast, ExprError> {
    let mut p = Parser { src: src.as_bytes(), pos: 0 };
    let ast = p.expr()?;
    p.skip_ws();
    if p.pos != p.src.len() {
        return Err(p.err("unexpected trailing input"));
    }
    Ok(ast)
}

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> ExprError {
        ExprError::Parse { pos: self.pos, msg: msg.to_string() }
    }
    fn skip_ws(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.src.get(self.pos).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<Ast, ExprError> {
        let mut lhs = self.term()?;
        loop {
            if self.eat(b'+') {
                lhs = Ast::Add(Box::new(lhs), Box::new(self.term()?));
            } else if self.eat(b'-') {
                lhs = Ast::Sub(Box::new(lhs), Box::new(self.term()?));
            } else {
                return Ok(lhs);
            }
        }
    }
    fn term(&mut self) -> Result<Ast, ExprError> {
        let mut lhs = self.unary()?;
        loop {
            if self.eat(b'*') {
                lhs = Ast::Mul(Box::new(lhs), Box::new(self.unary()?));
            } else if self.eat(b'/') {
                lhs = Ast::Div(Box::new(lhs), Box::new(self.unary()?));
            } else {
                return Ok(lhs);
            }
        }
    }
    fn unary(&mut self) -> Result<Ast, ExprError> {
        if self.eat(b'-') {
            return Ok(Ast::Neg(Box::new(self.unary()?)));
        }
        self.power()
    }
    fn power(&mut self) -> Result<Ast, ExprError> {
        let base = self.atom()?;
        if self.eat(b'^') {
            let exp = self.unary()?;
            return Ok(Ast::Pow(Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }
    fn atom(&mut self) -> Result<Ast, ExprError> {
        match self.peek() {
            Some(b'(') => {
                self.pos += 1;
                let e = self.expr()?;
                if !self.eat(b')') {
                    return Err(self.err("expected ')'"));
                }
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => self.number(),
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.pos;
                while self.pos < self.src.len()
                    && (self.src[self.pos].is_ascii_alphanumeric() || self.src[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                let name = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
                if self.eat(b'(') {
                    let mut args = Vec::new();
                    if !self.eat(b')') {
                        loop {
                            args.push(self.expr()?);
                            if self.eat(b')') {
                                break;
                            }
                            if !self.eat(b',') {
                                return Err(self.err("expected ',' or ')'"));
                            }
                        }
                    }
                    return Ok(Ast::Call(name, args));
                }
                Ok(Ast::Name(name))
            }
            _ => Err(self.err("expected number, name or '('")),
        }
    }
    fn number(&mut self) -> Result<Ast, ExprError> {
        let start = self.pos;
        while self.pos < self.src.len()
            && (self.src[self.pos].is_ascii_digit() || matches!(self.src[self.pos], b'.' | b'e' | b'E'))
        {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        text.parse::<f64>().map(Ast::Num).map_err(|_| self.err("bad number"))
    }
}
