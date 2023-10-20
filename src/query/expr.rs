use std::fmt::{self, Debug, Formatter};

use anyhow::{anyhow, Result};

use crate::query::parser::parse;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum Expr {
    Not(Box<Expr>),
    Then(Vec<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Func(String, String),
    Phrase(String),
    True,
    False,
    Empty,
}

impl Debug for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fn fmt_list(list: &[Expr], op: &str, f: &mut Formatter<'_>) -> fmt::Result {
            for (i, inner) in list.iter().enumerate() {
                if i > 0 {
                    write!(f, " {} ", op)?;
                }

                if inner.is_compound() {
                    write!(f, "({:?})", inner)?;
                } else {
                    write!(f, "{:?}", inner)?;
                }
            }

            Ok(())
        }

        match self {
            Expr::Not(inner) => {
                if inner.is_compound() {
                    write!(f, "NOT ({:?})", inner)
                } else {
                    write!(f, "NOT {:?}", inner)
                }
            }

            Expr::Then(exprs) => fmt_list(exprs, "THEN", f),
            Expr::And(exprs) => fmt_list(exprs, "AND", f),
            Expr::Or(exprs) => fmt_list(exprs, "OR", f),

            Expr::Func(a, b) => write!(f, "{}:{:?}", a, b),
            Expr::Phrase(a) => write!(f, "{:?}", a),

            Expr::True => write!(f, "TRUE"),
            Expr::False => write!(f, "FALSE"),
            Expr::Empty => write!(f, "EMPTY"),
        }
    }
}

impl Expr {
    pub fn parse(input: &str) -> Result<Expr> {
        parse(input).map_err(|_| anyhow!("malformed query"))
    }

    pub fn is_compound(&self) -> bool {
        match self {
            Expr::Then(..) => true,
            Expr::And(..) => true,
            Expr::Or(..) => true,
            Expr::Not(..) => true,
            _ => false,
        }
    }

    pub fn is_and(&self) -> bool {
        match self {
            Expr::And(..) => true,
            _ => false,
        }
    }

    pub fn is_or(&self) -> bool {
        match self {
            Expr::Or(..) => true,
            _ => false,
        }
    }

    pub fn has_phrases(&self) -> bool {
        match self {
            Expr::Phrase(..) => true,
            Expr::Not(inner) => inner.has_phrases(),
            Expr::Then(inner) => inner.iter().any(|e| e.has_phrases()),
            Expr::And(inner) => inner.iter().any(|e| e.has_phrases()),
            Expr::Or(inner) => inner.iter().any(|e| e.has_phrases()),
            _ => false,
        }
    }

    pub fn has_funcs(&self) -> bool {
        match self {
            Expr::Func(..) => true,
            Expr::Not(inner) => inner.has_funcs(),
            Expr::Then(inner) => inner.iter().any(|e| e.has_funcs()),
            Expr::And(inner) => inner.iter().any(|e| e.has_funcs()),
            Expr::Or(inner) => inner.iter().any(|e| e.has_funcs()),
            _ => false,
        }
    }

    pub fn get_func(&self, key: &str) -> Option<&str> {
        match self {
            Expr::Func(k, v) if k == key => Some(&v),
            Expr::And(inner) => inner.iter().flat_map(|e| e.get_func(key)).next(),
            _ => None,
        }
    }

    fn map_inplace(&mut self, f: impl FnOnce(Expr) -> Expr) {
        let orig = std::mem::replace(self, Expr::True);
        *self = f(orig);
    }

    fn map_inplace_result(&mut self, f: impl FnOnce(Expr) -> Result<Expr>) -> Result<()> {
        let orig = std::mem::replace(self, Expr::True);
        *self = f(orig)?;
        Ok(())
    }

    pub fn to_nnf(self) -> Expr {
        let is_or = self.is_or();
        let is_and = self.is_and();

        match self {
            Expr::Not(inner) => match *inner {
                Expr::Not(expr) => expr.to_nnf(),

                Expr::And(mut exprs) => {
                    for expr in &mut exprs {
                        expr.map_inplace(|e| Expr::Not(Box::new(e.to_nnf())));
                    }

                    Expr::Or(exprs)
                }

                Expr::Or(mut exprs) => {
                    for expr in &mut exprs {
                        expr.map_inplace(|e| Expr::Not(Box::new(e.to_nnf())));
                    }

                    Expr::And(exprs)
                }

                _ => Expr::Not(inner),
            },

            Expr::Then(mut inner) | Expr::And(mut inner) | Expr::Or(mut inner) => {
                for expr in inner.iter_mut() {
                    expr.map_inplace(Expr::to_nnf);
                }

                if is_or {
                    Expr::Or(inner)
                } else if is_and {
                    Expr::And(inner)
                } else {
                    Expr::Then(inner)
                }
            }

            e => e,
        }
    }

    pub fn reduce(self) -> Expr {
        fn reduce_list(list: &mut Vec<Expr>) {
            list.sort();
            list.dedup();
            list.retain(|e| e != &Expr::Empty);

            for expr in list {
                expr.map_inplace(Expr::reduce);
            }
        }

        fn has_conflict(list: &[Expr]) -> bool {
            for expr in list {
                if list.iter().any(|other| match other {
                    Expr::Not(inner) if &**inner == expr => true,
                    _ => false,
                }) {
                    return true;
                }
            }
            false
        }

        match self {
            Expr::And(mut exprs) => {
                let mut to_concat = vec![];

                for el in exprs.iter_mut().filter(|e| e.is_and()) {
                    if let Expr::And(mut inner) = std::mem::replace(el, Expr::True) {
                        to_concat.append(&mut inner);
                    }
                }

                exprs.append(&mut to_concat);
                exprs.retain(|e| e != &Expr::True);
                reduce_list(&mut exprs);

                let is_false = exprs.contains(&Expr::False) || has_conflict(&exprs);

                if is_false {
                    Expr::False
                } else if exprs.is_empty() {
                    Expr::Empty
                } else {
                    Expr::And(exprs)
                }
            }

            Expr::Or(mut exprs) => {
                let mut to_concat = vec![];

                for el in exprs.iter_mut().filter(|e| e.is_or()) {
                    if let Expr::Or(mut inner) = std::mem::replace(el, Expr::True) {
                        to_concat.append(&mut inner);
                    }
                }

                exprs.append(&mut to_concat);
                exprs.retain(|e| e != &Expr::False);
                reduce_list(&mut exprs);

                let is_true = exprs.contains(&Expr::True) || has_conflict(&exprs);

                if is_true {
                    Expr::True
                } else if exprs.is_empty() {
                    Expr::Empty
                } else {
                    Expr::Or(exprs)
                }
            }

            Expr::Not(mut inner) => {
                inner.map_inplace(Expr::reduce);
                Expr::Not(inner)
            }

            e => e,
        }
    }

    fn _expand(self, limit: &mut usize) -> Result<Expr> {
        fn should_expand(exprs: &[Expr]) -> bool {
            let has_funcs = exprs.iter().any(|e| e.has_funcs());
            let has_phrases = exprs.iter().any(Expr::has_phrases);
            has_funcs && has_phrases
        }

        fn expand_vec(exprs: &mut Vec<Expr>, limit: &mut usize) -> Result<bool> {
            let left_pos = exprs.iter().position(|e| match e {
                Expr::Or(inner) => should_expand(&inner),
                _ => false,
            });

            let left = match left_pos {
                Some(pos) => match exprs.remove(pos) {
                    Expr::Or(inner) => inner,
                    _ => return Ok(false),
                },
                None => return Ok(false),
            };

            if *limit == 0 {
                return Err(anyhow!("query is too complex"));
            } else {
                *limit -= 1;
            }

            let right = std::mem::replace(exprs, Vec::with_capacity(left.len()));

            for l in left {
                let mut inner = Vec::with_capacity(right.len() + 1);
                inner.push(l);

                for r in &right {
                    inner.push(r.clone());
                }

                if expand_vec(&mut inner, limit)? {
                    for e in inner.drain(..) {
                        exprs.push(e);
                    }
                } else {
                    exprs.push(Expr::And(inner));
                }
            }

            Ok(true)
        }

        let res = match self {
            Expr::And(mut exprs) => {
                for expr in exprs.iter_mut() {
                    expr.map_inplace_result(|e| e._expand(limit))?;
                }

                if expand_vec(&mut exprs, limit)? {
                    Expr::Or(exprs)
                } else {
                    Expr::And(exprs)
                }
            }

            Expr::Or(mut exprs) => {
                for expr in exprs.iter_mut() {
                    expr.map_inplace_result(|e| e._expand(limit))?;
                }

                Expr::Or(exprs)
            }

            Expr::Then(mut exprs) => {
                for expr in exprs.iter_mut() {
                    expr.map_inplace_result(|e| e._expand(limit))?;
                }

                Expr::Then(exprs)
            }

            e => e,
        };

        Ok(res)
    }

    pub fn expand(self) -> Result<Expr> {
        self._expand(&mut 8192).map(|e| e.reduce())
    }

    pub fn normalize(self) -> Result<Expr> {
        self.validate()?.to_nnf().reduce().expand()
    }

    fn _validate(self, level: usize) -> Result<Expr> {
        match self {
            Expr::Not(mut inner) => {
                inner.map_inplace_result(|e| e._validate(level + 1))?;
                Ok(Expr::Not(inner))
            }

            Expr::Then(mut exprs) => {
                for expr in &mut exprs {
                    if expr.has_funcs() {
                        return Err(anyhow!("THEN operands cannot contain functions"));
                    }

                    expr.map_inplace_result(|e| e._validate(level + 1))?
                }

                Ok(Expr::Then(exprs))
            }

            Expr::And(mut exprs) => {
                for expr in &mut exprs {
                    expr.map_inplace_result(|e| e._validate(level + 1))?
                }
                Ok(Expr::And(exprs))
            }

            Expr::Or(mut exprs) => {
                for expr in &mut exprs {
                    if expr.get_func("sort").is_some() || expr.get_func("order").is_some() {
                        return Err(anyhow!(
                            "sorting functions inside OR operands are disallowed"
                        ));
                    }

                    expr.map_inplace_result(|e| e._validate(level + 1))?
                }

                Ok(Expr::Or(exprs))
            }

            Expr::Func(key, value) => match key.as_str() {
                "sort" | "order" | "bots" if level > 1 => {
                    Err(anyhow!("`{}` function should be at the top level", key))
                }
                _ => Ok(Expr::Func(key, value)),
            },

            expr => Ok(expr),
        }
    }

    pub fn validate(self) -> Result<Expr> {
        self._validate(0)
    }
}
