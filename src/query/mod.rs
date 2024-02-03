mod expr;
mod functions;
mod parser;
use crate::models;

pub use self::expr::Expr;

use anyhow::{bail, Result};

use sqlx::database::{HasStatement, HasArguments};
use sqlx::encode::Encode;
use sqlx::postgres::{PgArguments, PgConnection, Postgres};
use sqlx::{prelude::*, Arguments, Pool};
use sqlx::{Execute, Type};
use futures::StreamExt;

#[derive(Default)]
pub struct Bindings {
    arguments: PgArguments,
    len: usize,
}

#[derive(Default)]
pub struct QueryBuilder {
    sql: String,
}

impl QueryBuilder {
    pub fn sql(&mut self, sql: impl AsRef<str>) {
        self.sql.push_str(sql.as_ref());
    }

    pub fn append(&mut self, other: &QueryBuilder) {
        self.sql.push_str(&other.sql);
    }

    pub fn binding_id<T>(&mut self, bindings: &mut Bindings, val: T) -> usize
    where
        T: Type<Postgres> + for<'a> Encode<'a, Postgres> + std::marker::Send,
    {
        bindings.len += 1;
        let id = bindings.len;
    
        let mut arguments = std::mem::replace(&mut bindings.arguments, PgArguments::default());
        arguments.add(val);
        bindings.arguments = arguments;
    
        id
    }

    pub fn binding<T>(&mut self, bindings: &mut Bindings, val: T)
    where
        T: Type<Postgres> + for<'a> Encode<'a, Postgres> + std::marker::Send,
    {
        bindings.len += 1;
        let id = bindings.len;
        self.sql.push_str(&format!("${}", id));
        let mut arguments = std::mem::replace(&mut bindings.arguments, PgArguments::default());
        arguments.add(val);
        bindings.arguments = arguments;
    }
}

struct ExecWrapper<'a>(&'a QueryBuilder, Bindings);

impl<'a> Execute<'a, Postgres> for ExecWrapper<'a> {
    fn sql(&self) -> &'a str {
        self.0.sql.as_str()
    }

    fn statement(&self) -> Option<&<Postgres as HasStatement<'a>>::Statement> {
        None
    }

    fn take_arguments(&mut self) -> Option<<Postgres as HasArguments<'a>>::Arguments> {
        Some(std::mem::take(&mut self.1.arguments))
    }

    fn persistent(&self) -> bool {
        false 
    }
}

fn can_separate(expr: &Expr) -> bool {
    match expr {
        Expr::And(exprs) | Expr::Or(exprs) => !exprs.iter().any(|e| {
            let has_phrases = e.has_phrases();
            let has_funcs = e.has_funcs();
            has_phrases && has_funcs
        }),

        _ => true,
    }
}

fn shallow_retain_expr(expr: Expr, p: impl Fn(&Expr) -> bool) -> Expr {
    match expr {
        Expr::And(mut exprs) => {
            exprs.retain(p);
            Expr::And(exprs)
        }

        Expr::Or(mut exprs) => {
            exprs.retain(p);
            Expr::Or(exprs)
        }

        expr => {
            if p(&expr) {
                expr
            } else {
                Expr::True
            }
        }
    }
}

fn leave_tsqueries_only(expr: Expr) -> Expr {
    shallow_retain_expr(expr, |e| !e.has_funcs())
}

fn leave_funcs_only(expr: Expr) -> Expr {
    shallow_retain_expr(expr, |e| !e.has_phrases())
}

fn handle_list(
    query: &mut QueryBuilder,
    bindings: &mut Bindings,
    mut f: impl FnMut(&mut QueryBuilder, &mut Bindings, Expr) -> Result<()>,
    op: &str,
    list: Vec<Expr>,
) -> Result<()> {
    for (i, inner) in list.into_iter().enumerate() {
        if i > 0 {
            query.sql(" ");
            query.sql(op);
            query.sql(" ");
        }
        query.sql("(");
        f(query, bindings, inner)?;
        query.sql(")");
    }

    Ok(())
}

fn build_tsquery(query: &mut QueryBuilder, bindings: &mut Bindings, expr: Expr) -> Result<()> {
    match expr {
        Expr::Phrase(p) => {
            query.sql("phraseto_tsquery('russian', ");
            query.binding(bindings, p);
            query.sql(")");
        }

        Expr::Not(inner) => {
            query.sql("!! (");
            build_tsquery(query, bindings, *inner)?;
            query.sql(")");
        }

        Expr::Then(inner) => handle_list(query, bindings, build_tsquery, "<->", inner)?,
        Expr::And(inner) => handle_list(query, bindings, build_tsquery, "&&", inner)?,
        Expr::Or(inner) => handle_list(query, bindings, build_tsquery, "||", inner)?,
        Expr::Func(..) => bail!("full-message function in phrase context (this is probably a bug)"),
        _ => query.sql("phraseto_tsquery('russian', '')"),
    }

    Ok(())
}

fn build_func_filter(query: &mut QueryBuilder, bindings: &mut Bindings, expr: Expr) -> Result<()> {
    match expr {
        Expr::Func(name, value) => match name.as_str() {
            "bots" | "sort" | "order" => query.sql("TRUE"),
            _ => self::functions::handle(query, bindings, name, value)?,
        },

        Expr::Not(inner) => {
            query.sql("NOT (");
            build_func_filter(query, bindings, *inner)?;
            query.sql(")");
        }

        Expr::And(inner) => handle_list(query, bindings, build_func_filter, "AND", inner)?,
        Expr::Or(inner) => handle_list(query, bindings, build_func_filter, "OR", inner)?,

        Expr::Then(_) => bail!("THEN in functional context (this is probably a bug)"),
        Expr::Phrase(_) => bail!("phrase in functional context (this is probably a bug)"),

        Expr::True => query.sql("TRUE"),
        Expr::False | Expr::Empty => query.sql("FALSE"),
    }

    Ok(())
}

fn build_phrase_filter(
    query: &mut QueryBuilder,
    bindings: &mut Bindings,
    tsqueries: &mut Vec<QueryBuilder>,
    expr: Expr,
) -> Result<()> {
    let mut partial = QueryBuilder::default();
    build_tsquery(&mut partial, bindings, expr)?;
    query.sql("to_tsvector('russian', msg_body) @@ (");
    query.append(&partial);
    query.sql(")");
    tsqueries.push(partial);
    Ok(())
}

pub fn build_filter(
    query: &mut QueryBuilder,
    bindings: &mut Bindings,
    tsqueries: &mut Vec<QueryBuilder>,
    expr: Expr,
) -> Result<()> {
    if can_separate(&expr) {
        let is_or = expr.is_or();

        let f_expr = leave_funcs_only(expr.clone()).reduce();
        let p_expr = leave_tsqueries_only(expr).reduce();

        if is_or {
            if f_expr == Expr::True || p_expr == Expr::True {
                query.sql("TRUE");
            } else if f_expr == Expr::False && p_expr == Expr::False {
                query.sql("FALSE");
            } else if f_expr == Expr::Empty {
                build_phrase_filter(query, bindings, tsqueries, p_expr)?
            } else if p_expr == Expr::Empty {
                build_func_filter(query, bindings, f_expr)?
            } else {
                query.sql("(");
                build_phrase_filter(query, bindings, tsqueries, p_expr)?;
                query.sql(") OR (");
                build_func_filter(query, bindings, f_expr)?;
                query.sql(")");
            }
        } else {
            if f_expr == Expr::False || p_expr == Expr::False {
                query.sql("FALSE");
            } else if f_expr == Expr::True && p_expr == Expr::True {
                query.sql("TRUE");
            } else if f_expr == Expr::Empty || f_expr == Expr::True {
                build_phrase_filter(query, bindings, tsqueries, p_expr)?
            } else if p_expr == Expr::Empty || p_expr == Expr::True {
                build_func_filter(query, bindings, f_expr)?
            } else {
                query.sql("(");
                build_phrase_filter(query, bindings, tsqueries, p_expr)?;
                query.sql(") AND (");
                build_func_filter(query, bindings, f_expr)?;
                query.sql(")");
            }
        }
    } else {
        match expr {
            Expr::And(inner) => handle_list(
                query,
                bindings,
                |q, b, e| build_filter(q, b, tsqueries, e),
                "AND",
                inner,
            )?,
            Expr::Or(inner) => handle_list(
                query,
                bindings,
                |q, b, e| build_filter(q, b, tsqueries, e),
                "OR",
                inner,
            )?,
            _ => unreachable!(),
        }
    }

    Ok(())
}

fn relevance(query: &mut QueryBuilder, tsqueries: &[QueryBuilder]) {
    if tsqueries.is_empty() {
        query.sql("msg_timestamp");
    }

    for (i, q) in tsqueries.iter().enumerate() {
        if i > 0 {
            query.sql(" + ");
        }
        query.sql("ts_rank(to_tsvector('russian', msg_body), ");
        query.append(&q);
        query.sql(")");
    }
}

fn should_exclude_bots(value: &str) -> Result<bool> {
    match value {
        "exclude" => Ok(true),
        "include" => Ok(false),
        _ => {
            bail!("bad 'bots' function argument: either 'exclude' (default) or 'include' expected")
        }
    }
}

pub async fn search(
    db: Pool<Postgres>,
    expr: Expr,
) -> Result<Vec<models::Message>> {
    let mut query = QueryBuilder::default();
    let mut bindings = Bindings::default();

    let sort = expr
        .get_func("sort")
        .unwrap_or_else(|| "relevance")
        .to_owned();

    let order = expr.get_func("order").unwrap_or_else(|| "desc").to_owned();

    query.sql(
        "SELECT msg_id, msg_offset, msg_author, msg_body, msg_timestamp \
         FROM messages \
         LEFT JOIN aliases ON alias_secondary = msg_author \
         WHERE ",
    );

    let mut tsqueries = vec![];
    let mut filter = QueryBuilder::default();
    build_filter(
        &mut filter,
        &mut bindings,
        &mut tsqueries,
        expr.normalize()?,
    )?;

    query.append(&filter);
    query.sql(" ORDER BY ");

    match sort.as_str() {
        "time" => {
            query.sql("msg_timestamp");
        },
        "relevance" => {
            relevance(&mut query, &tsqueries);
        }
        "random" => {
            query.sql("RANDOM()");
        }
        _ => bail!("bad 'sort' function argument: either 'time', 'relevance' or 'random' expected")
    }

    match order.as_str() {
        "asc" => query.sql(" ASC"),
        "desc" => query.sql(" DESC"),
        _ => bail!("bad 'order' function argument: either 'desc' (default) or 'asc' expected"),
    }

    query.sql(" LIMIT 1000");
    println!("{}", query.sql);

    let mut messages = Vec::new();
    let mut rows = db.fetch(ExecWrapper(&query, bindings));
    while let Some(Ok(row)) = rows.next().await {
        messages.push(models::Message {
            id: row.get(0),
            author: row.get(2),
            body: row.get(3),
            time: row.get(4),
            offset: row.get(1)
        })
    }

    Ok(messages)
}

pub async fn count(
    db: &mut PgConnection,
    bot_list: Vec<String>,
    expr: Expr,
) -> Result<CountResult> {
    let exclude_bots = should_exclude_bots(expr.get_func("bots").unwrap_or_else(|| "exclude"))?;

    let mut query = QueryBuilder::default();
    let mut bindings = Bindings::default();

    #[rustfmt::skip]
    query.sql(
        "SELECT count(*), \
                count(distinct msg_author), \
                count(distinct coalesce(alias_primary, msg_author)) \
         FROM messages \
         LEFT JOIN aliases ON alias_secondary = msg_author \
         WHERE ",
    );

    let mut tsqueries = vec![];
    let mut filter = QueryBuilder::default();
    build_filter(
        &mut filter,
        &mut bindings,
        &mut tsqueries,
        expr.normalize()?,
    )?;

    query.append(&filter);

    if exclude_bots {
        query.sql(" AND msg_author != ALL(");
        query.binding(&mut bindings, bot_list);
        query.sql(")");
    }

    println!("{}", query.sql);

    let mut rows = db.fetch(ExecWrapper(&query, bindings));
    let row = rows.next().await.unwrap();
    let row = row?;
    Ok(CountResult {
        total_messages: row.get(0),
        total_users_raw: row.get(1),
        total_users: row.get(2),
    })
}

pub async fn top(db: &mut PgConnection, bot_list: Vec<String>, expr: Expr) -> Result<TopResult> {
    let exclude_bots = should_exclude_bots(expr.get_func("bots").unwrap_or_else(|| "exclude"))?;

    let mut query = QueryBuilder::default();
    let mut bindings = Bindings::default();

    #[rustfmt::skip]
    query.sql(
        "SELECT coalesce(alias_primary, msg_author) as author, \
                count(msg_body) \
         FROM messages \
         LEFT JOIN aliases ON alias_secondary = msg_author \
         WHERE ",
    );

    let mut tsqueries = vec![];
    let mut filter = QueryBuilder::default();
    build_filter(
        &mut filter,
        &mut bindings,
        &mut tsqueries,
        expr.clone().normalize()?,
    )?;

    query.append(&filter);

    if exclude_bots {
        query.sql(" AND msg_author != ALL(");
        query.binding(&mut bindings, bot_list.clone());
        query.sql(")");
    }

    query.sql("  GROUP BY author ORDER BY count(msg_body) DESC LIMIT 6");

    println!("{}", query.sql);

    let count = count(db, bot_list, expr).await?;

    let mut top = Vec::new();
    let mut rows = db.fetch(ExecWrapper(&query, bindings));
    while let Some(Ok(row)) = rows.next().await {
        top.push((row.get(0), row.get(1)));
    }

    Ok(TopResult {
        top,
        total_messages: count.total_messages,
        total_users: count.total_users,
        total_users_raw: count.total_users_raw,
    })
}

#[derive(Clone, Debug)]
pub struct CountResult {
    pub total_messages: i64,
    pub total_users: i64,
    pub total_users_raw: i64,
}

#[derive(Clone, Debug)]
pub struct TopResult {
    pub top: Vec<(String, i64)>,
    pub total_messages: i64,
    pub total_users: i64,
    pub total_users_raw: i64,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub offset: i32,
    pub author: String,
    pub body: String,
    pub timestamp: chrono::NaiveDateTime,
}
