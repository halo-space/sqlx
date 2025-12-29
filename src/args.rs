//! Args：存储参数并把含 `$` 语法的 format 编译成最终 SQL（对齐 go-sqlbuilder `args.go`）。

use crate::flavor::Flavor;
use crate::flavor::default_flavor;
use crate::modifiers::{Arg, Raw, SqlNamedArg};
use crate::string_builder::StringBuilder;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("builder invalid arg reference ${0}")]
    InvalidArgRef(isize),
}

/// Args 存储 SQL 相关参数。
#[derive(Debug, Clone)]
pub struct Args {
    /// 默认 flavor，用于 `compile`。
    pub flavor: Flavor,

    pub(crate) index_base: usize,
    pub(crate) arg_values: Vec<Arg>,
    pub(crate) named_args: HashMap<String, usize>,
    pub(crate) sql_named_args: HashMap<String, usize>,
    pub(crate) only_named: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for Args {
    fn default() -> Self {
        Self {
            flavor: default_flavor(),
            index_base: 0,
            arg_values: Vec::new(),
            named_args: HashMap::new(),
            sql_named_args: HashMap::new(),
            only_named: false,
        }
    }
}

impl Args {
    /// Add：追加一个参数并返回内部占位符（`$0/$1/...`）。
    pub fn add(&mut self, arg: impl Into<Arg>) -> String {
        let idx = self.add_internal(arg.into());
        format!("${idx}")
    }

    /// Replace：用新参数替换某个 `$n` 占位符对应的值（对齐 go-sqlbuilder `Args.Replace`）。
    pub fn replace(&mut self, placeholder: &str, arg: impl Into<Arg>) {
        if !placeholder.starts_with('$') {
            return;
        }
        let digits = &placeholder[1..];
        if digits.is_empty() || !digits.as_bytes().iter().all(|b| b.is_ascii_digit()) {
            return;
        }
        if let Ok(i) = digits.parse::<usize>() {
            let idx = i.saturating_sub(self.index_base);
            if idx < self.arg_values.len() {
                self.arg_values[idx] = arg.into();
            }
        }
    }

    /// Value：按 `$<n>` 前缀解析参数值（对齐 go-sqlbuilder `Args.Value` 的“宽松匹配”）。
    ///
    /// - `placeholder` 可以带后缀（如 `"$0xxx"`），只要以 `$<digits>` 开头就会解析。
    pub fn value(&self, placeholder: &str) -> Option<&Arg> {
        let s = placeholder.strip_prefix('$')?;
        let mut end = 0usize;
        for b in s.as_bytes() {
            if b.is_ascii_digit() {
                end += 1;
            } else {
                break;
            }
        }
        if end == 0 {
            return None;
        }
        let n: usize = s[..end].parse().ok()?;
        let idx = n.saturating_sub(self.index_base);
        self.arg_values.get(idx)
    }

    fn add_internal(&mut self, mut arg: Arg) -> usize {
        let idx = self.arg_values.len() + self.index_base;

        match &mut arg {
            Arg::SqlNamed(SqlNamedArg { name, value: _ }) => {
                if let Some(&p) = self.sql_named_args.get(name) {
                    arg = self.arg_values[p - self.index_base].clone();
                } else {
                    self.sql_named_args.insert(name.clone(), idx);
                }
                // fallthrough: push arg below
            }
            Arg::Named { name, arg: inner } => {
                if let Some(&p) = self.named_args.get(name) {
                    arg = self.arg_values[p - self.index_base].clone();
                } else {
                    // 先把真实参数加入，再记录 name->idx
                    let real_idx = self.add_internal((**inner).clone());
                    self.named_args.insert(name.clone(), real_idx);
                    return real_idx;
                }
            }
            _ => {}
        }

        self.arg_values.push(arg);
        idx
    }

    /// Compile：按默认 flavor 编译 format。
    pub fn compile(&self, format: &str, initial_value: &[Arg]) -> (String, Vec<Arg>) {
        self.compile_with_flavor(format, self.flavor, initial_value)
    }

    /// CompileWithFlavor：编译 format，并用 `flavor` 输出最终占位符。
    pub fn compile_with_flavor(
        &self,
        format: &str,
        flavor: Flavor,
        initial_value: &[Arg],
    ) -> (String, Vec<Arg>) {
        let mut offset = 0usize;
        let mut ctx = CompileContext {
            buf: StringBuilder::new(),
            flavor,
            values: initial_value.to_vec(),
            named_args: Vec::new(),
        };

        let mut rest = format;
        while let Some(pos) = rest.find('$') {
            if pos > 0 {
                ctx.buf.write_str(&rest[..pos]);
            }
            rest = &rest[pos + 1..];

            if rest.is_empty() {
                ctx.buf.write_char('$');
                break;
            }

            let b0 = rest.as_bytes()[0];
            match b0 {
                b'$' => {
                    ctx.buf.write_char('$');
                    rest = &rest[1..];
                }
                b'{' => {
                    rest = self.compile_named(&mut ctx, rest);
                }
                b'0'..=b'9' if !self.only_named => {
                    let (r, off) = self.compile_digits(&mut ctx, rest, offset);
                    rest = r;
                    offset = off;
                }
                b'?' if !self.only_named => {
                    let (r, off) = self.compile_successive(&mut ctx, &rest[1..], offset);
                    rest = r;
                    offset = off;
                }
                _ => {
                    ctx.buf.write_char('$');
                }
            }
        }

        if !rest.is_empty() {
            ctx.buf.write_str(rest);
        }

        let sql = ctx.buf.into_string();
        let values = self.merge_sql_named_args(ctx.values, ctx.named_args);
        (sql, values)
    }

    fn compile_named<'a>(&self, ctx: &mut CompileContext, format: &'a str) -> &'a str {
        // format[0] == '{'
        if let Some(end) = format.find('}') {
            let name = &format[1..end];
            let rest = &format[end + 1..];
            if let Some(&p) = self.named_args.get(name) {
                let (r, _off) = self.compile_successive(ctx, rest, p - self.index_base);
                return r;
            }
            return rest;
        }
        // invalid
        format
    }

    fn compile_digits<'a>(
        &self,
        ctx: &mut CompileContext,
        format: &'a str,
        offset: usize,
    ) -> (&'a str, usize) {
        let mut i = 0usize;
        for b in format.as_bytes() {
            if b.is_ascii_digit() {
                i += 1;
            } else {
                break;
            }
        }
        let digits = &format[..i];
        let rest = &format[i..];
        if let Ok(pointer) = digits.parse::<usize>() {
            return self.compile_successive(ctx, rest, pointer.saturating_sub(self.index_base));
        }
        (rest, offset)
    }

    fn compile_successive<'a>(
        &self,
        ctx: &mut CompileContext,
        format: &'a str,
        offset: usize,
    ) -> (&'a str, usize) {
        if offset >= self.arg_values.len() {
            ctx.buf.write_str("/* INVALID ARG $");
            ctx.buf.write_str(&offset.to_string());
            ctx.buf.write_str(" */");
            return (format, offset);
        }
        let arg = self.arg_values[offset].clone();
        ctx.write_value(&arg);
        (format, offset + 1)
    }

    fn merge_sql_named_args(&self, mut values: Vec<Arg>, named: Vec<SqlNamedArg>) -> Vec<Arg> {
        if self.sql_named_args.is_empty() && named.is_empty() {
            return values;
        }

        // 先追加 ctx 中遇到的 named args，并去重
        let mut seen = HashMap::<String, ()>::new();
        for a in named {
            if seen.insert(a.name.clone(), ()).is_none() {
                values.push(Arg::SqlNamed(a));
            }
        }

        // 再追加 Add() 时出现但 ctx 中未出现的 named args，按位置稳定排序
        let mut idxs: Vec<usize> = self
            .sql_named_args
            .iter()
            .filter_map(|(n, &p)| if seen.contains_key(n) { None } else { Some(p) })
            .collect();
        idxs.sort_unstable();
        for p in idxs {
            values.push(self.arg_values[p - self.index_base].clone());
        }

        values
    }
}

#[derive(Debug)]
struct CompileContext {
    buf: StringBuilder,
    flavor: Flavor,
    values: Vec<Arg>,
    named_args: Vec<SqlNamedArg>,
}

impl CompileContext {
    fn write_value(&mut self, arg: &Arg) {
        match arg {
            Arg::Builder(b) => {
                let (sql, args) = b.build_with_flavor(self.flavor, &self.values);
                self.buf.write_str(&sql);

                let (values, named) = split_named_args(args);
                self.values = values;
                self.named_args.extend(named);
            }
            Arg::SqlNamed(SqlNamedArg { name, value }) => {
                self.buf.write_char('@');
                self.buf.write_str(name);
                self.named_args.push(SqlNamedArg {
                    name: name.clone(),
                    value: value.clone(),
                });
            }
            Arg::Raw(Raw { expr }) => self.buf.write_str(expr),
            Arg::List { args, is_tuple } => {
                if *is_tuple {
                    self.buf.write_char('(');
                }
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.buf.write_str(", ");
                    }
                    self.write_value(a);
                }
                if *is_tuple {
                    self.buf.write_char(')');
                }
            }
            Arg::Named { .. } => {
                // Named 只在 `${name}` 被解析到时才会真正生效；
                // 这里按普通值处理（保持行为可预测）。
                self.write_placeholder_and_push(arg.clone());
            }
            Arg::Valuer(_) => self.write_placeholder_and_push(arg.clone()),
            Arg::Value(_) => self.write_placeholder_and_push(arg.clone()),
        }
    }

    fn write_placeholder_and_push(&mut self, arg: Arg) {
        match self.flavor {
            Flavor::MySQL
            | Flavor::SQLite
            | Flavor::CQL
            | Flavor::ClickHouse
            | Flavor::Presto
            | Flavor::Informix
            | Flavor::Doris => {
                self.buf.write_char('?');
            }
            Flavor::PostgreSQL => {
                let idx = self.values.len() + 1;
                self.buf.write_char('$');
                self.buf.write_str(&idx.to_string());
            }
            Flavor::SQLServer => {
                let idx = self.values.len() + 1;
                self.buf.write_str(&format!("@p{idx}"));
            }
            Flavor::Oracle => {
                let idx = self.values.len() + 1;
                self.buf.write_char(':');
                self.buf.write_str(&idx.to_string());
            }
        }
        self.values.push(arg);
    }
}

fn split_named_args(mut values: Vec<Arg>) -> (Vec<Arg>, Vec<SqlNamedArg>) {
    if values.is_empty() {
        return (values, Vec::new());
    }

    let mut named = Vec::new();
    while let Some(Arg::SqlNamed(a)) = values.last().cloned() {
        values.pop();
        named.push(a);
    }
    named.reverse();
    (values, named)
}
