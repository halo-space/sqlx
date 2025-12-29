//! Build / BuildNamed / Buildf 等自由拼接能力（对齐 go-sqlbuilder `builder.go`）。

use crate::args::Args;
use crate::flavor::Flavor;
use crate::modifiers::{Arg, Builder, escape, named};

#[derive(Debug, Clone)]
struct CompiledBuilder {
    args: Args,
    format: String,
}

impl CompiledBuilder {
    fn new(args: Args, format: String) -> Self {
        Self { args, format }
    }
}

impl Builder for CompiledBuilder {
    fn build_with_flavor(&self, flavor: Flavor, initial_arg: &[Arg]) -> (String, Vec<Arg>) {
        self.args
            .compile_with_flavor(&self.format, flavor, initial_arg)
    }

    fn flavor(&self) -> Flavor {
        self.args.flavor
    }
}

#[derive(Clone)]
struct FlavoredBuilder {
    inner: Box<dyn Builder>,
    flavor: Flavor,
}

impl Builder for FlavoredBuilder {
    fn build_with_flavor(&self, flavor: Flavor, initial_arg: &[Arg]) -> (String, Vec<Arg>) {
        self.inner.build_with_flavor(flavor, initial_arg)
    }

    fn flavor(&self) -> Flavor {
        self.flavor
    }
}

/// WithFlavor：给 builder 绑定默认 flavor。
pub fn with_flavor(builder: impl Builder + 'static, flavor: Flavor) -> Box<dyn Builder> {
    Box::new(FlavoredBuilder {
        inner: Box::new(builder),
        flavor,
    })
}

/// Build：使用 `$` 特殊语法构建 builder。
pub fn build(
    format: impl Into<String>,
    args_in: impl IntoIterator<Item = impl Into<Arg>>,
) -> Box<dyn Builder> {
    let mut args = Args::default();
    for a in args_in {
        args.add(a);
    }
    Box::new(CompiledBuilder::new(args, format.into()))
}

/// BuildNamed：只启用 `${name}` 与 `$$`，从 map 中引用参数。
pub fn build_named(
    format: impl Into<String>,
    named_map: impl IntoIterator<Item = (String, Arg)>,
) -> Box<dyn Builder> {
    let mut args = Args {
        only_named: true,
        ..Args::default()
    };

    for (k, v) in named_map {
        args.add(named(k, v));
    }

    Box::new(CompiledBuilder::new(args, format.into()))
}

/// Buildf：类似 fmt.Sprintf 的自由拼接（仅支持 `%v`/`%s`）。
pub fn buildf(format: &str, args_in: impl IntoIterator<Item = impl Into<Arg>>) -> Box<dyn Builder> {
    let mut args = Args::default();
    let escaped = escape(format);
    let mut out = String::new();

    let mut it = args_in.into_iter();
    let mut chars = escaped.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek().copied() {
                Some('v') | Some('s') => {
                    chars.next();
                    if let Some(a) = it.next() {
                        let ph = args.add(a.into());
                        out.push_str(&ph);
                    } else {
                        // 没有足够参数：按字面输出，保持行为可见
                        out.push('%');
                        out.push('v');
                    }
                }
                Some('%') => {
                    chars.next();
                    out.push('%');
                }
                _ => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }

    // 忽略多余参数：对齐 go 的 fmt 行为（多余的不会出现在 format 中）
    Box::new(CompiledBuilder::new(args, out))
}
