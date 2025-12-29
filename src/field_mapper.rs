//! Field mapper：把 Rust 字段名映射为列名（对齐 go-sqlbuilder `fieldmapper.go`）。

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// 字段名映射函数类型（对齐 go 的 `FieldMapperFunc`）。
pub type FieldMapperFunc = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;

fn identity_impl(s: &str) -> String {
    s.to_string()
}

static IDENTITY_MAPPER: OnceLock<FieldMapperFunc> = OnceLock::new();

/// 恒等 mapper（等价于 go 的 `DefaultFieldMapper == nil`）。
pub fn identity_mapper() -> FieldMapperFunc {
    IDENTITY_MAPPER
        .get_or_init(|| Arc::new(identity_impl))
        .clone()
}

static DEFAULT_FIELD_MAPPER: OnceLock<Mutex<FieldMapperFunc>> = OnceLock::new();
static DEFAULT_FIELD_MAPPER_LOCK: Mutex<()> = Mutex::new(());

fn mapper_cell() -> &'static Mutex<FieldMapperFunc> {
    DEFAULT_FIELD_MAPPER.get_or_init(|| Mutex::new(identity_mapper()))
}

/// 获取当前全局默认 FieldMapper（对齐 go 的 `DefaultFieldMapper`）。
pub fn default_field_mapper() -> FieldMapperFunc {
    mapper_cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// 设置全局默认 FieldMapper，返回旧值。
pub fn set_default_field_mapper(mapper: FieldMapperFunc) -> FieldMapperFunc {
    let mut g = mapper_cell().lock().unwrap_or_else(|e| e.into_inner());
    std::mem::replace(&mut *g, mapper)
}

/// 修改全局默认 FieldMapper 的 RAII guard（会持有一个全局锁，避免并行测试互相干扰）。
pub struct DefaultFieldMapperGuard {
    _lock: MutexGuard<'static, ()>,
    old: FieldMapperFunc,
}

impl Drop for DefaultFieldMapperGuard {
    fn drop(&mut self) {
        let _ = set_default_field_mapper(self.old.clone());
    }
}

/// 在一个作用域内临时设置默认 FieldMapper，并保证退出作用域后自动恢复。
pub fn set_default_field_mapper_scoped(mapper: FieldMapperFunc) -> DefaultFieldMapperGuard {
    let lock = DEFAULT_FIELD_MAPPER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let old = set_default_field_mapper(mapper);
    DefaultFieldMapperGuard { _lock: lock, old }
}

fn convert_with_separator(s: &str, sep: char) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut prev: Option<char> = None;
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied();
        let is_upper = c.is_ascii_uppercase();

        if is_upper {
            if let Some(p) = prev {
                let prev_is_lower_or_digit = p.is_ascii_lowercase() || p.is_ascii_digit();
                let prev_is_upper = p.is_ascii_uppercase();
                let next_is_lower = next.map(|n| n.is_ascii_lowercase()).unwrap_or(false);

                if prev_is_lower_or_digit || (prev_is_upper && next_is_lower) {
                    out.push(sep);
                }
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }

        prev = Some(c);
    }

    out
}

/// SnakeCaseMapper：将 `CamelCase` 转为 `snake_case`（对齐 go 的 `SnakeCaseMapper`）。
///
/// 注意：go-sqlbuilder 依赖 `xstrings.ToSnakeCase`。这里实现的是足以覆盖本仓库测试的规则子集：
/// - 大写转小写
/// - 单词边界插入 `_`（`aB`/`a1B`/`ABc` 等）
pub fn snake_case_mapper(s: &str) -> String {
    convert_with_separator(s, '_')
}

/// KebabcaseMapper：将 `CamelCase` 转为 `kebab-case`（用于测试更多转换策略）。
pub fn kebab_case_mapper(s: &str) -> String {
    convert_with_separator(s, '-')
}

/// UpperCaseMapper：将字段名转为全部大写。
pub fn upper_case_mapper(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// PrefixMapper：返回一个在字段名前添加固定前缀的 mapper。
pub fn prefix_mapper(prefix: &'static str) -> FieldMapperFunc {
    Arc::new(move |name| format!("{prefix}{name}"))
}

/// SuffixMapper：返回一个在字段名后添加固定后缀的 mapper。
pub fn suffix_mapper(suffix: &'static str) -> FieldMapperFunc {
    Arc::new(move |name| format!("{name}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_helpers_work() {
        assert_eq!(snake_case_mapper("FieldName"), "field_name");
        assert_eq!(kebab_case_mapper("FieldName"), "field-name");
    }

    #[test]
    fn upper_case_mapper_changes_case() {
        assert_eq!(upper_case_mapper("FieldName"), "FIELDNAME");
    }

    #[test]
    fn prefix_suffix_mappers_apply() {
        let prefix = prefix_mapper("db_");
        let suffix = suffix_mapper("_col");
        assert_eq!(prefix("FieldName"), "db_FieldName");
        assert_eq!(suffix("FieldName"), "FieldName_col");
    }
}
