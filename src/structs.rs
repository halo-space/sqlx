//! Struct：轻量 ORM（参考 go-sqlbuilder 的 Struct/structfields 实现）。
//!
//! Rust 无运行时反射；在“不新增 proc-macro crate”的约束下，本实现通过 `macro_rules!`
//! 为 struct 生成字段元数据与取值逻辑，从而提供与 go-sqlbuilder 接近的体验。

use crate::delete::DeleteBuilder;
use crate::escape_all;
use crate::field_mapper::{FieldMapperFunc, default_field_mapper};
use crate::flavor::Flavor;
use crate::insert::InsertBuilder;
use crate::select::SelectBuilder;
use crate::select_cols;
use crate::update::UpdateBuilder;
use std::any::Any;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldOpt {
    WithQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMeta {
    /// Rust 字段名（用于生成取值代码）
    pub rust: &'static str,
    /// 用于 FieldMapper 的“原始字段名”（对齐 go 的 reflect.StructField.Name）。
    ///
    /// Rust 无法获得运行时字段名；这里由宏生成，默认等于 `rust`，
    /// 但测试/用户可以显式指定以对齐 go 的 CamelCase 命名。
    pub orig: &'static str,
    /// SQL 列名（db tag / mapper 之后）
    pub db: &'static str,
    /// 可选别名（AS）
    pub as_: Option<&'static str>,
    /// tags
    pub tags: &'static [&'static str],
    /// omitempty tags（包含 "" 表示默认）
    pub omitempty_tags: &'static [&'static str],
    pub with_quote: bool,
}

impl FieldMeta {
    pub fn name_for_select(&self, flavor: Flavor, alias: &str) -> String {
        let base = if self.with_quote {
            flavor.quote(alias)
        } else {
            alias.to_string()
        };
        if let Some(as_) = self.as_ {
            format!("{base} AS {as_}")
        } else {
            base
        }
    }
}

fn is_ignored(fm: &FieldMeta) -> bool {
    // 对齐 go 的 `db:"-"`：忽略该字段
    fm.db == "-"
}

/// 由宏为你的业务 struct 实现的 trait：提供字段元数据与取值/空值判断。
pub trait SqlStruct: Sized {
    const FIELDS: &'static [FieldMeta];

    /// 取字段的值用于 INSERT/UPDATE（按 FIELDS 顺序）。
    fn values(&self) -> Vec<crate::modifiers::Arg>;

    /// 判断某个字段是否“空值”（用于 omitempty）。
    fn is_empty_field(&self, rust_field: &'static str) -> bool;

    /// 返回可写入的扫描目标列表（用于 `Struct::addr*`）。
    ///
    /// 说明：为了避免 Rust 借用检查器对“多次可变借用同一 struct”的限制，
    /// 这里一次性生成全部 `ScanCell`（内部用 raw pointer 持有字段地址）。
    fn addr_cells<'a>(
        &'a mut self,
        rust_fields: &[&'static str],
    ) -> Option<Vec<crate::scan::ScanCell<'a>>>;
}

/// 判断“空值”的 trait（用于实现 go-sqlbuilder 的 omitempty 语义子集）。
pub trait IsEmpty {
    fn is_empty_value(&self) -> bool;
}

impl IsEmpty for String {
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for &str {
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for bool {
    fn is_empty_value(&self) -> bool {
        !*self
    }
}

macro_rules! empty_num {
    ($($t:ty),+ $(,)?) => {
        $(impl IsEmpty for $t {
            fn is_empty_value(&self) -> bool {
                *self == 0 as $t
            }
        })+
    };
}

empty_num!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl IsEmpty for f64 {
    fn is_empty_value(&self) -> bool {
        // 对齐 go：用 bits 判断 0（避免 -0.0 的边界差异）
        self.to_bits() == 0
    }
}

impl<T: IsEmpty> IsEmpty for Option<T> {
    fn is_empty_value(&self) -> bool {
        match self {
            None => true,
            Some(v) => v.is_empty_value(),
        }
    }
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty_value(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for Box<dyn crate::valuer::SqlValuer> {
    fn is_empty_value(&self) -> bool {
        // 对齐 go 的指针语义：非 nil 指针不是 empty。
        false
    }
}

pub struct Struct<T: SqlStruct> {
    pub flavor: Flavor,
    mapper: FieldMapperFunc,
    with_tags: Vec<&'static str>,
    without_tags: Vec<&'static str>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: SqlStruct> Clone for Struct<T> {
    fn clone(&self) -> Self {
        Self {
            flavor: self.flavor,
            mapper: self.mapper.clone(),
            with_tags: self.with_tags.clone(),
            without_tags: self.without_tags.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: SqlStruct> std::fmt::Debug for Struct<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // mapper 无法 Debug；这里只输出关键信息，避免影响使用与测试。
        f.debug_struct("Struct")
            .field("flavor", &self.flavor)
            .field("with_tags", &self.with_tags)
            .field("without_tags", &self.without_tags)
            .finish()
    }
}

impl<T: SqlStruct> Default for Struct<T> {
    fn default() -> Self {
        Self {
            flavor: crate::default_flavor(),
            mapper: default_field_mapper(),
            with_tags: Vec::new(),
            without_tags: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: SqlStruct> Struct<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// WithFieldMapper：返回 shadow copy，并覆盖当前 Struct 的 mapper（对齐 go `Struct.WithFieldMapper`）。
    ///
    /// - 传入 `identity_mapper()` 等价于 go 的 `WithFieldMapper(nil)`。
    pub fn with_field_mapper(&self, mapper: FieldMapperFunc) -> Self {
        let mut c = self.clone();
        c.mapper = mapper;
        c
    }

    fn has_defined_tag(tag: &str) -> bool {
        if tag.is_empty() {
            return false;
        }
        T::FIELDS
            .iter()
            .any(|f| !is_ignored(f) && f.tags.contains(&tag))
    }

    /// ForFlavor：返回 shadow copy（对齐 go `Struct.For`），不修改原对象。
    pub fn for_flavor(&self, flavor: Flavor) -> Self {
        let mut c = self.clone();
        c.flavor = flavor;
        c
    }

    /// WithTag：返回 shadow copy（对齐 go `Struct.WithTag`），不修改原对象。
    pub fn with_tag(&self, tags: impl IntoIterator<Item = &'static str>) -> Self {
        let mut c = self.clone();
        for t in tags {
            if t.is_empty() {
                continue;
            }
            if !c.with_tags.contains(&t) {
                c.with_tags.push(t);
            }
        }
        c.with_tags.sort_unstable();
        c.with_tags.dedup();
        c
    }

    /// WithoutTag：返回 shadow copy（对齐 go `Struct.WithoutTag`），不修改原对象。
    pub fn without_tag(&self, tags: impl IntoIterator<Item = &'static str>) -> Self {
        let mut c = self.clone();
        for t in tags {
            if t.is_empty() {
                continue;
            }
            if !c.without_tags.contains(&t) {
                c.without_tags.push(t);
            }
        }
        c.without_tags.sort_unstable();
        c.without_tags.dedup();
        // 过滤 with_tags
        c.with_tags.retain(|t| !c.without_tags.contains(t));
        c
    }

    fn should_omit_empty(&self, fm: &FieldMeta) -> bool {
        // 对齐 go 的 structField.ShouldOmitEmpty(with...):
        // - 先看默认 tag ""
        // - 再看 with tags
        let omit = fm.omitempty_tags;
        if omit.is_empty() {
            return false;
        }
        if omit.contains(&"") {
            return true;
        }
        self.with_tags.iter().any(|t| omit.contains(t))
    }

    fn excluded_by_without(&self, fm: &FieldMeta) -> bool {
        self.without_tags.iter().any(|t| fm.tags.contains(t))
    }

    fn alias_of(&self, fm: &FieldMeta) -> String {
        if is_ignored(fm) {
            return String::new();
        }

        if !fm.db.is_empty() {
            return fm.db.to_string();
        }

        let mapped = (self.mapper)(fm.orig);
        if mapped.is_empty() {
            fm.orig.to_string()
        } else {
            mapped
        }
    }

    fn read_key_of(&self, fm: &FieldMeta) -> String {
        // 对齐 go structField.Key：优先 As，否则 Alias，否则 Name
        if let Some(as_) = fm.as_ {
            return as_.to_string();
        }
        let a = self.alias_of(fm);
        if a.is_empty() { fm.rust.to_string() } else { a }
    }

    fn write_key_of(&self, fm: &FieldMeta) -> String {
        // 对齐 go ForWrite：按 Alias 去重
        let a = self.alias_of(fm);
        if a.is_empty() { fm.rust.to_string() } else { a }
    }

    fn fields_for_read(&self) -> Vec<&'static FieldMeta> {
        self.fields_filtered(true)
    }

    fn fields_for_write(&self) -> Vec<&'static FieldMeta> {
        self.fields_filtered(false)
    }

    fn fields_filtered(&self, for_read: bool) -> Vec<&'static FieldMeta> {
        let mut out = Vec::new();
        let mut seen = HashSet::<String>::new();

        let push_field = |out: &mut Vec<&'static FieldMeta>,
                          seen: &mut HashSet<String>,
                          fm: &'static FieldMeta,
                          for_read: bool| {
            if is_ignored(fm) {
                return;
            }
            if self.excluded_by_without(fm) {
                return;
            }
            let key = if for_read {
                self.read_key_of(fm)
            } else {
                self.write_key_of(fm)
            };
            if seen.insert(key) {
                out.push(fm);
            }
        };

        if self.with_tags.is_empty() {
            for fm in T::FIELDS {
                push_field(&mut out, &mut seen, fm, for_read);
            }
            return out;
        }

        // 对齐 go FilterTags(with...): 按 with_tags 顺序（这里已排序）逐个 tag 抽取字段并去重
        for tag in &self.with_tags {
            for fm in T::FIELDS {
                if fm.tags.contains(tag) {
                    push_field(&mut out, &mut seen, fm, for_read);
                }
            }
        }

        out
    }

    fn parse_table_alias(table: &str) -> &str {
        // 与 go 实现一致：取最后一个空格后的 token
        table.rsplit_once(' ').map(|(_, a)| a).unwrap_or(table)
    }

    /// Columns：对齐 go-sqlbuilder `Struct.Columns()`（返回 ForWrite 的未 quote 列名）。
    pub fn columns(&self) -> Vec<String> {
        self.fields_for_write()
            .into_iter()
            .map(|f| self.alias_of(f))
            .collect()
    }

    /// ColumnsForTag：对齐 go-sqlbuilder `Struct.ColumnsForTag(tag)`。
    ///
    /// - 如果 tag 不存在，返回 None（对齐 go 返回 nil）
    pub fn columns_for_tag(&self, tag: &str) -> Option<Vec<String>> {
        if !Self::has_defined_tag(tag) {
            return None;
        }
        // API 约束：当前实现需要 &'static str；这里为对齐 go 的便捷接口，做一次泄漏。
        // 后续如果要严格控制内存，可把 tags 改为 Cow<'static, str>。
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        Some(self.with_tag([tag]).columns())
    }

    /// Values：对齐 go-sqlbuilder `Struct.Values()`（返回 ForWrite 的值，顺序与 `columns()` 一致）。
    pub fn values(&self, v: &T) -> Vec<crate::modifiers::Arg> {
        let all = v.values();
        let mut out = Vec::new();
        for (fm, arg) in T::FIELDS.iter().zip(all) {
            if is_ignored(fm) || self.excluded_by_without(fm) {
                continue;
            }
            if self.with_tags.is_empty() || self.with_tags.iter().any(|t| fm.tags.contains(t)) {
                out.push(arg);
            }
        }
        // 注意：上面是“声明顺序”而不是 “tag 分组顺序”；
        // 为与 go 完全一致（多 tag 时按 tag 分组 + 去重），这里用 fields_for_write 再重排。
        let mut map = std::collections::HashMap::<&'static str, crate::modifiers::Arg>::new();
        for (fm, arg) in T::FIELDS.iter().zip(v.values()) {
            map.insert(fm.rust, arg);
        }
        self.fields_for_write()
            .into_iter()
            .filter_map(|fm| map.get(fm.rust).cloned())
            .collect()
    }

    /// ValuesForTag：对齐 go-sqlbuilder `Struct.ValuesForTag(tag, value)`。
    ///
    /// - 如果 tag 不存在，返回 None（对齐 go 返回 nil）
    pub fn values_for_tag(&self, tag: &str, v: &T) -> Option<Vec<crate::modifiers::Arg>> {
        if !Self::has_defined_tag(tag) {
            return None;
        }
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        Some(self.with_tag([tag]).values(v))
    }

    /// ForeachRead：对齐 go-sqlbuilder `Struct.ForeachRead`。
    ///
    /// - `dbtag`：等价 go 的 `sf.DBTag`（可能为空字符串）
    /// - `is_quoted`：等价 go 的 `sf.IsQuoted`
    /// - `field_meta`：Rust 侧提供的字段元信息（替代 go 的 `reflect.StructField`）
    pub fn foreach_read(&self, mut trans: impl FnMut(&str, bool, &FieldMeta)) {
        for fm in self.fields_for_read() {
            trans(fm.db, fm.with_quote, fm);
        }
    }

    /// ForeachWrite：对齐 go-sqlbuilder `Struct.ForeachWrite`。
    pub fn foreach_write(&self, mut trans: impl FnMut(&str, bool, &FieldMeta)) {
        for fm in self.fields_for_write() {
            trans(fm.db, fm.with_quote, fm);
        }
    }

    /// Addr：对齐 go-sqlbuilder `Struct.Addr(st)`（返回 ForRead 的“写入目标”列表）。
    pub fn addr<'a>(&self, st: &'a mut T) -> Vec<crate::scan::ScanCell<'a>> {
        let rust_fields: Vec<&'static str> = self
            .fields_for_read()
            .into_iter()
            .map(|fm| fm.rust)
            .collect();
        st.addr_cells(&rust_fields).unwrap_or_default()
    }

    /// AddrForTag：对齐 go-sqlbuilder `Struct.AddrForTag(tag, st)`。
    /// tag 不存在返回 None（对齐 go 返回 nil）。
    pub fn addr_for_tag<'a>(
        &self,
        tag: &str,
        st: &'a mut T,
    ) -> Option<Vec<crate::scan::ScanCell<'a>>> {
        if !Self::has_defined_tag(tag) {
            return None;
        }
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        Some(self.with_tag([tag]).addr(st))
    }

    /// AddrWithCols：对齐 go-sqlbuilder `Struct.AddrWithCols(cols, st)`。
    /// 如果 cols 中任一列找不到，返回 None（对齐 go 返回 nil）。
    pub fn addr_with_cols<'a>(
        &self,
        cols: &[&str],
        st: &'a mut T,
    ) -> Option<Vec<crate::scan::ScanCell<'a>>> {
        let fields = self.fields_for_read();
        let mut map = std::collections::HashMap::<String, &'static str>::new();
        for fm in fields {
            let key = self.read_key_of(fm);
            map.insert(key, fm.rust);
        }

        let mut rust_fields = Vec::with_capacity(cols.len());
        for &c in cols {
            rust_fields.push(*map.get(c)?);
        }
        st.addr_cells(&rust_fields)
    }

    pub fn select_from(&self, table: &str) -> SelectBuilder {
        let mut sb = SelectBuilder::new();
        sb.set_flavor(self.flavor);
        sb.from([table.to_string()]);

        let alias = Self::parse_table_alias(table);
        let cols: Vec<String> = self
            .fields_for_read()
            .into_iter()
            .map(|f| {
                let field_alias = self.alias_of(f);
                let mut c = String::new();
                // 对齐 go：只检查 sf.Alias（db）是否包含 '.'
                if self.flavor != Flavor::CQL && !field_alias.contains('.') {
                    c.push_str(alias);
                    c.push('.');
                }
                c.push_str(&f.name_for_select(self.flavor, &field_alias));
                c
            })
            .collect();

        if cols.is_empty() {
            select_cols!(sb, "*");
        } else {
            sb.select(cols);
        }
        sb
    }

    /// SelectFromForTag：对齐 go-sqlbuilder `SelectFromForTag(table, tag)`（deprecated）。
    pub fn select_from_for_tag(&self, table: &str, tag: &str) -> SelectBuilder {
        // go：如果 tag 不存在，则 SELECT *；这里复用现有行为：with_tag 后 cols 为空 => select "*"
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        self.with_tag([tag]).select_from(table)
    }

    pub fn update(&self, table: &str, value: &T) -> UpdateBuilder {
        let mut ub = UpdateBuilder::new();
        ub.set_flavor(self.flavor);
        ub.update([table.to_string()]);

        let mut assigns = Vec::new();

        let mut map = std::collections::HashMap::<&'static str, crate::modifiers::Arg>::new();
        for (fm, arg) in T::FIELDS.iter().zip(value.values()) {
            map.insert(fm.rust, arg);
        }

        for fm in self.fields_for_write() {
            if self.should_omit_empty(fm) && value.is_empty_field(fm.rust) {
                continue;
            }
            // 对齐 go 的 withquote：写入时也需要 quote 列名。
            let field_alias = self.alias_of(fm);
            let col = if fm.with_quote {
                self.flavor.quote(&field_alias)
            } else {
                field_alias
            };
            if let Some(v) = map.get(fm.rust).cloned() {
                assigns.push(ub.assign(&col, v));
            }
        }

        ub.set(assigns);
        ub
    }

    /// UpdateForTag：对齐 go-sqlbuilder `UpdateForTag(table, tag, value)`（deprecated）。
    pub fn update_for_tag(&self, table: &str, tag: &str, value: &T) -> UpdateBuilder {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        self.with_tag([tag]).update(table, value)
    }

    pub fn delete_from(&self, table: &str) -> DeleteBuilder {
        let mut db = DeleteBuilder::new();
        db.set_flavor(self.flavor);
        db.delete_from([table.to_string()]);
        db
    }

    pub fn insert_into<'a>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = &'a T>,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        self.insert_internal(table, rows, InsertVerb::Insert)
    }

    /// InsertIntoForTag：对齐 go-sqlbuilder `InsertIntoForTag(table, tag, value...)`（deprecated）。
    pub fn insert_into_for_tag<'a>(
        &self,
        table: &str,
        tag: &str,
        rows: impl IntoIterator<Item = &'a T>,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        self.with_tag([tag]).insert_into(table, rows)
    }

    pub fn insert_ignore_into_for_tag<'a>(
        &self,
        table: &str,
        tag: &str,
        rows: impl IntoIterator<Item = &'a T>,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        self.with_tag([tag]).insert_ignore_into(table, rows)
    }

    pub fn replace_into_for_tag<'a>(
        &self,
        table: &str,
        tag: &str,
        rows: impl IntoIterator<Item = &'a T>,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        self.with_tag([tag]).replace_into(table, rows)
    }

    fn filter_rows_any<'a>(values: impl IntoIterator<Item = &'a dyn Any>) -> Vec<&'a T>
    where
        T: 'static,
    {
        values
            .into_iter()
            .filter_map(|v| v.downcast_ref::<T>())
            .collect()
    }

    /// InsertIntoAny：对齐 go `InsertInto(table, value ...interface{})` 的“忽略非预期类型”语义。
    pub fn insert_into_any<'a>(
        &self,
        table: &str,
        values: impl IntoIterator<Item = &'a dyn Any>,
    ) -> InsertBuilder
    where
        T: 'static,
    {
        let rows = Self::filter_rows_any(values);
        self.insert_into(table, rows)
    }

    pub fn insert_ignore_into_any<'a>(
        &self,
        table: &str,
        values: impl IntoIterator<Item = &'a dyn Any>,
    ) -> InsertBuilder
    where
        T: 'static,
    {
        let rows = Self::filter_rows_any(values);
        self.insert_ignore_into(table, rows)
    }

    pub fn replace_into_any<'a>(
        &self,
        table: &str,
        values: impl IntoIterator<Item = &'a dyn Any>,
    ) -> InsertBuilder
    where
        T: 'static,
    {
        let rows = Self::filter_rows_any(values);
        self.replace_into(table, rows)
    }

    pub fn insert_into_for_tag_any<'a>(
        &self,
        table: &str,
        tag: &str,
        values: impl IntoIterator<Item = &'a dyn Any>,
    ) -> InsertBuilder
    where
        T: 'static,
    {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        let rows = Self::filter_rows_any(values);
        self.with_tag([tag]).insert_into(table, rows)
    }

    pub fn insert_ignore_into_for_tag_any<'a>(
        &self,
        table: &str,
        tag: &str,
        values: impl IntoIterator<Item = &'a dyn Any>,
    ) -> InsertBuilder
    where
        T: 'static,
    {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        let rows = Self::filter_rows_any(values);
        self.with_tag([tag]).insert_ignore_into(table, rows)
    }

    pub fn replace_into_for_tag_any<'a>(
        &self,
        table: &str,
        tag: &str,
        values: impl IntoIterator<Item = &'a dyn Any>,
    ) -> InsertBuilder
    where
        T: 'static,
    {
        let tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        let rows = Self::filter_rows_any(values);
        self.with_tag([tag]).replace_into(table, rows)
    }

    pub fn insert_ignore_into<'a>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = &'a T>,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        self.insert_internal(table, rows, InsertVerb::InsertIgnore)
    }

    pub fn replace_into<'a>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = &'a T>,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        self.insert_internal(table, rows, InsertVerb::Replace)
    }

    fn insert_internal<'a>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = &'a T>,
        verb: InsertVerb,
    ) -> InsertBuilder
    where
        T: 'a,
    {
        let mut ib = InsertBuilder::new();
        ib.set_flavor(self.flavor);
        match verb {
            InsertVerb::Insert => {
                ib.insert_into(table);
            }
            InsertVerb::InsertIgnore => {
                ib.insert_ignore_into(table);
            }
            InsertVerb::Replace => {
                ib.replace_into(table);
            }
        }

        let rows: Vec<&T> = rows.into_iter().collect();
        if rows.is_empty() {
            // 对齐 go：空 value slice 不会调用 Cols/Values
            return ib;
        }

        let fields = self.fields_for_write();

        // 计算列是否应被整体过滤（omitempty 且所有行均为空）
        let mut nil_cnt = vec![0_usize; fields.len()];
        for (fi, fm) in fields.iter().enumerate() {
            let should_omit = self.should_omit_empty(fm);
            if !should_omit {
                continue;
            }
            for r in &rows {
                if r.is_empty_field(fm.rust) {
                    nil_cnt[fi] += 1;
                }
            }
        }

        let mut kept = Vec::<usize>::new();
        for (i, cnt) in nil_cnt.into_iter().enumerate() {
            if cnt == rows.len() {
                continue;
            }
            kept.push(i);
        }

        let cols: Vec<String> = kept
            .iter()
            .map(|&i| {
                let fm = fields[i];
                let field_alias = self.alias_of(fm);
                if fm.with_quote {
                    self.flavor.quote(&field_alias)
                } else {
                    field_alias
                }
            })
            .collect();
        ib.cols(escape_all(cols));

        for r in rows {
            let mut map = std::collections::HashMap::<&'static str, crate::modifiers::Arg>::new();
            for (fm, arg) in T::FIELDS.iter().zip(r.values()) {
                map.insert(fm.rust, arg);
            }
            let mut row_args = Vec::new();
            for &i in &kept {
                let fm = fields[i];
                row_args.push(
                    map.get(fm.rust)
                        .cloned()
                        .unwrap_or_else(|| crate::SqlValue::Null.into()),
                );
            }
            ib.values(row_args);
        }

        ib
    }
}

#[derive(Debug, Clone, Copy)]
enum InsertVerb {
    Insert,
    InsertIgnore,
    Replace,
}

/// 声明一个可用于 `Struct<T>` 的业务 struct 元数据与取值逻辑。
///
/// 用法示例：
///
/// ```ignore
/// #[derive(Default)]
/// struct User { id: i64, name: String }
///
/// halo_space::sqlbuilder::sql_struct! {
///   impl User {
///     id:  { db: "id", tags: ["pk"], omitempty: [], quote: false, as: None },
///     name:{ db: "name", tags: [],     omitempty: [""], quote: true,  as: None },
///   }
/// }
/// ```
#[macro_export]
macro_rules! sql_struct {
    (
        impl $ty:ty {
            $(
                $field:ident : { db: $db:literal, $(orig: $orig:literal,)? tags: [ $($tag:literal),* $(,)? ], omitempty: [ $($omit:literal),* $(,)? ], quote: $quote:literal, as: $as:expr }
            ),* $(,)?
        }
    ) => {
        impl $crate::structs::SqlStruct for $ty {
            const FIELDS: &'static [$crate::structs::FieldMeta] = &[
                $(
                    $crate::structs::FieldMeta{
                        rust: stringify!($field),
                        orig: $crate::__sql_struct_orig!(stringify!($field) $(, $orig)?),
                        db: $db,
                        as_: $as,
                        tags: &[ $($tag),* ],
                        omitempty_tags: &[ $($omit),* ],
                        with_quote: $quote,
                    }
                ),*
            ];

            fn values(&self) -> Vec<$crate::modifiers::Arg> {
                vec![
                    $(
                        $crate::modifiers::Arg::from(self.$field.clone())
                    ),*
                ]
            }

            fn is_empty_field(&self, rust_field: &'static str) -> bool {
                match rust_field {
                    $(
                        stringify!($field) => $crate::structs::IsEmpty::is_empty_value(&self.$field),
                    )*
                    _ => false,
                }
            }

            fn addr_cells<'a>(
                &'a mut self,
                rust_fields: &[&'static str],
            ) -> Option<Vec<$crate::scan::ScanCell<'a>>> {
                let mut out = Vec::with_capacity(rust_fields.len());
                for &rf in rust_fields {
                    match rf {
                        $(
                            stringify!($field) => {
                                out.push($crate::scan::ScanCell::from_ptr(std::ptr::addr_of_mut!(self.$field)));
                            }
                        )*
                        _ => return None,
                    }
                }
                Some(out)
            }
        }
    };
}

/// 宏内部 helper：支持 `orig:` 的可选参数。
#[doc(hidden)]
#[macro_export]
macro_rules! __sql_struct_orig {
    ($default:expr) => {
        $default
    };
    ($default:expr, $custom:expr) => {
        $custom
    };
}
