# halo-sqlx 

它是一个对齐 [huandu/go-sqlbuilder](https://github.com/huandu/go-sqlbuilder) 设计的 Rust crate，提供：

- `Args` + `Flavor`：支持 `?`、`$1`、`@p1`、`:1` 等多种占位符策略，并且允许通过 `Flavor` 跟随不同 SQL 方言；
- 完整的各类 Builder：`SelectBuilder`、`InsertBuilder`、`UpdateBuilder`、`DeleteBuilder`、`UnionBuilder`、`CTEBuilder`、`CTEQueryBuilder`、`CreateTableBuilder`，同时内建查、插、改、删、聚合、CTE、Union 和 clone 重用模式；
- `Build`/`Buildf`/`BuildNamed`：支持 `${name}`、`$0`、`$?`、`$$`、`Raw`、`List`、`Tuple` 等语法，并支持嵌套 builder、named arg 重用、literal `$` 等特殊行为；
- `Struct` + `field_mapper`：通过 `macro_rules!` 生成 `FieldMeta`，支持 `db`/`fieldtag`/`fieldopt`/`fieldas`、`with_tag`/`without_tag`、自定义 field mapper（如 snake_case/kebab_case/prefix/suffix）并兼容 `SqlValuer`；
- `Scan` + `ScanCell`：仿照 Go 的 `Addr` 实现数据扫描；
- `interpolate`：为不支持参数化的驱动提供 SQL 插值，涵盖多 flavor 的字符串/数字/日期/布尔等转义；
- `SqlValuer`：支持延迟计算参数，兼容自定义数据源；
- 全部示例/单测对齐 Go：138 条单测 + doc-test，覆盖 README 中的 builder、Struct、CTE、Union、field mapper、命名参数等场景。

## 典型用法

### 创建 SELECT

```rust
use halo_space::sqlx::{from_tables, select_cols, where_exprs, select::SelectBuilder};

let mut sb = SelectBuilder::new();
select_cols!(sb, "id");
from_tables!(sb, "user");
where_exprs!(sb, sb.in_("status", [1_i64, 2, 3]));

let (sql, args) = sb.build();
assert_eq!(sql, "SELECT id FROM user WHERE status IN (?, ?, ?)");
assert_eq!(args.len(), 3);
```

### 直接使用 builder API

```rust
use halo_space::sqlx::select::SelectBuilder;

let mut sb = SelectBuilder::new();
sb.select("id") // 单列
    .select_more(["name", "email"]) // 支持数组
    .select_more(vec!["score"]); // 支持 Vec
sb.from(["users", "users_detail"]);
sb.order_by(["name", "score"])
    .where_(["score >= 100", "status = 'active'"]);

let (sql, args) = sb.build();
assert!(sql.contains("SELECT id, name, email, score"));
assert!(sql.contains("FROM users, users_detail"));
```

`select` / `select_more` / `from` / `where_` 等函数现在接受任何实现了 `IntoStrings` 的输入（`&str`、`String`、数组、`Vec`），也可直接用宏调用。

`SelectBuilder`/`UpdateBuilder`/`DeleteBuilder`/`InsertBuilder` 都自带 `build()` 方法，内部直接调用对应的 `build_with_flavor`，免去再显式导入 `modifiers::Builder`（普通 `insert_into(...).values(...).build()` 也能直接使用）。

### Condition / Chain 查询

```rust
use halo_space::sqlx::condition::{
    build_select_with_flavor, Chain, ChainOptions, Condition, ConditionValue, Operator,
};
use halo_space::sqlx::select::SelectBuilder;
use halo_space::sqlx::Flavor;

// 直接用 Condition 组合 OR
let conditions = vec![
    Condition::new("name", Operator::Equal, "jzero"),
    Condition {
        or: true,
        or_operators: vec![Operator::Between, Operator::Between],
        or_fields: vec!["age".into(), "height".into()],
        or_values: vec![
            ConditionValue::from([24_i64, 48]),
            ConditionValue::from([170_i64, 175]),
        ],
        skip: false,
        skip_fn: None,
        or_values_fn: None,
        field: String::new(),
        operator: Operator::Between,
        value: ConditionValue::default(),
        value_fn: None,
        join: None,
        where_clause: None,
    },
];

let mut sb = SelectBuilder::new();
sb.select(vec!["name", "age", "height"]).from(vec!["user"]);
let (sql, args) = build_select_with_flavor(Flavor::MySQL, sb, conditions);
assert_eq!(
    sql,
    "SELECT name, age, height FROM user WHERE `name` = ? AND (`age` BETWEEN ? AND ? OR `height` BETWEEN ? AND ?)"
);
assert_eq!(args, vec!["jzero".into(), 24_i64.into(), 48_i64.into(), 170_i64.into(), 175_i64.into()]);

// 链式 Chain，可叠加 join / 分页 / group by / order
let chain = Chain::new()
    .equal_opts("status", "active", ChainOptions::default().skip(false))
    .join(
        halo_space::sqlx::JoinOption::InnerJoin,
        "user_ext",
        ["user.id = user_ext.uid"],
    )
    .group_by("status")
    .page(2, 10)
    .order_by_desc("created_at");

let mut sb2 = SelectBuilder::new();
sb2.select(vec!["user.id", "user.name"]).from(vec!["user"]);
let (sql2, _args2) = build_select_with_flavor(Flavor::MySQL, sb2, chain.build());
assert!(sql2.contains("INNER JOIN user_ext ON user.id = user_ext.uid"));
assert!(sql2.contains("LIMIT"));

// 更贴近 Go WithValueFunc/WithSkip/WithSkipFunc 的“后置修饰”写法
// 既可传闭包，也可把已有函数名当回调传进去
fn compute_name() -> &'static str {
    "jzero"
}

let chain2 = Chain::new()
    .equal("name", "placeholder")
    .value_fn(|| compute_name().into()) // 传函数；也可以写 .value_fn(|| "jzero".into())
    .skip(false)                 // 可直接写 skip
    .skip_fn(|| false);          // 高于 skip
let mut sb3 = SelectBuilder::new();
sb3.select(vec!["id", "name"]).from(vec!["user"]);
let (sql3, args3) = build_select_with_flavor(Flavor::MySQL, sb3, chain2.build());
assert_eq!(sql3, "SELECT id, name FROM user WHERE `name` = ?");
assert_eq!(args3, vec!["jzero".into()]);
```

### 变长参数宏

宏（`select_cols!`、`from_tables!`、`where_exprs!`、`returning_cols!` 等）可直接从根导入：`use halo_space::sqlx::{select_cols, from_tables, where_exprs};`，自动把多个字符串/列名展开为 `Vec<String>`，无需手动构造切片。

```rust
use halo_space::sqlx::{from_tables, order_by_cols, select_cols, where_exprs, select::SelectBuilder};

let mut sb = SelectBuilder::new();
select_cols!(sb, "id", "name");
from_tables!(sb, "users");
where_exprs!(sb, "status = 'active'", "type <> 'guest'");
order_by_cols!(sb, "name");

let (sql, _) = sb.build();
assert!(sql.contains("WHERE"));
```

宏还覆盖了 `insert_cols!` / `insert_select_cols!` / `delete_from_tables!` / `update_set!` / `create_table_define!` / `struct_with_tag!` 等常见接受字符串 varargs 的接口。

### INSERT / RETURNING

```rust
use halo_space::sqlx::{insert::InsertBuilder, insert_cols, returning_cols};

let mut ib = InsertBuilder::new();
ib.insert_into("users");
insert_cols!(ib, "name", "age").values(["alice", 18_i64]);
returning_cols!(ib, "id");

let (sql, args) = ib.build();
assert_eq!(sql, "INSERT INTO users (name, age) VALUES (?, ?) RETURNING id");
assert_eq!(args.len(), 2);
```

### UPDATE / WHERE / ORDER BY

```rust
use halo_space::sqlx::{update::UpdateBuilder, update_set, where_exprs, update_tables};

let mut ub = UpdateBuilder::new();
update_tables!(ub, "users");
update_set!(ub, "score = score + 1");
where_exprs!(ub, "status = 'active'");
ub.order_by_desc("score");

let (sql, _) = ub.build();
assert!(sql.contains("UPDATE users SET score = score + 1 WHERE status = 'active' ORDER BY score DESC"));
```

### Condition / Chain 更新

```rust
use halo_space::sqlx::condition::{
    build_update_with_flavor, Chain, ConditionValue, Operator, UpdateFieldChain, UpdateFieldOptions,
};
use halo_space::sqlx::update::UpdateBuilder;
use halo_space::sqlx::Flavor;

let updates = UpdateFieldChain::new()
    .assign("name", "alice", UpdateFieldOptions::default())
    .incr("version", UpdateFieldOptions::default())
    .add("score", 5_i64, UpdateFieldOptions::default());

let chain = Chain::new().equal("id", 1_i64);

let mut ub = UpdateBuilder::new();
ub.update(vec!["users"]);
let (sql, _args) = build_update_with_flavor(Flavor::MySQL, ub, updates.build(), chain.build());
assert!(sql.starts_with("UPDATE users SET"));
assert!(sql.contains("WHERE `id` = ?"));
```

### DELETE / LIMIT

```rust
use halo_space::sqlx::{delete::DeleteBuilder, delete_from_tables, where_exprs};

let mut db = DeleteBuilder::new();
delete_from_tables!(db, "sessions");
where_exprs!(db, "expired_at < NOW()");
db.limit(100);

let (sql, _) = db.build();
assert!(sql.contains("DELETE FROM sessions WHERE expired_at < NOW() LIMIT ?"));
```

### 嵌套 Builder / Buildf

```rust
use halo_space::sqlx::{builder::buildf, from_tables, select_cols, select::SelectBuilder};

let mut sb = SelectBuilder::new();
select_cols!(sb, "id");
from_tables!(sb, "user");

let explain = buildf(
    "EXPLAIN %v LEFT JOIN SELECT * FROM banned WHERE state IN (%v, %v)",
    [sb.into(), 1_i64, 2_i64],
);
let (sql, _) = explain.build();
assert!(sql.contains("EXPLAIN SELECT id FROM user"));
```

### named 参数

```rust
use halo_space::sqlx::{
    builder::build_named,
    modifiers::{SqlNamedArg, raw, list},
};

let mut named = std::collections::HashMap::new();
named.insert("table".to_string(), raw("user"));
named.insert("status".to_string(), list([1_i64, 2, 3]));
named.insert("time".to_string(), SqlNamedArg::new("start", 1_514_458_225_i64).into());

let (sql, args) = build_named(
    "SELECT * FROM ${table} WHERE status IN (${status}) AND created_at > ${time}",
    named,
)
.build();
assert!(sql.contains("@start"));
```

## 致谢

- [huandu/go-sqlbuilder](https://github.com/huandu/go-sqlbuilder)：Rust 版本的设计和行为对齐自该项目。
- [jzero](https://github.com/jzero-io/jzero)：链式条件与模板思路来源，并在示例中保持一致的使用体验。

### Struct ORM + field mapper

```rust
use halo_space::sqlx::{field_mapper::snake_case_mapper, Struct};

// 启用 snake_case 映射
let _guard = halo_space::sqlx::field_mapper::set_default_field_mapper_scoped(
    std::sync::Arc::new(snake_case_mapper),
);

#[derive(Default, Clone)]
struct User {
    id: i64,
    user_name: String,
}

// 使用 sql_struct! 生成字段元数据与取值逻辑
halo_space::sqlx::sql_struct! {
    impl User {
        id:        { db: "id",  tags: [], omitempty: [], quote: false, as: None },
        user_name: { db: "",    tags: [], omitempty: [], quote: false, as: None },
    }
}

let s = Struct::<User>::new();
let (sql, _) = s.select_from("user").build();
assert!(sql.contains("user.user_name"));
```

### CTE 与 Union

```rust
use halo_space::sqlx::{
    cte::with,
    cte_query::CTEQueryBuilder,
    from_tables, select_cols, where_exprs,
    select::SelectBuilder,
};

let mut users_cte = CTEQueryBuilder::new();
let mut query = SelectBuilder::new();
select_cols!(query, "id");
from_tables!(query, "users");
where_exprs!(query, "name IS NOT NULL");
users_cte.table("users", ["id"]).as_(query);

let cte = with([users_cte]);
let mut sb = cte.select(Vec::<String>::new());
select_cols!(sb, "users.id");
from_tables!(sb, "users");
let (sql, _) = sb.build();
assert!(sql.contains("WITH users"));
```

### UNION / UNION ALL

```rust
use halo_space::sqlx::{union::UnionBuilder, select::SelectBuilder, select_cols, from_tables};

let mut sb1 = SelectBuilder::new();
select_cols!(sb1, "id");
from_tables!(sb1, "t1");

let mut sb2 = SelectBuilder::new();
select_cols!(sb2, "id");
from_tables!(sb2, "t2");

let mut ub = UnionBuilder::new();
ub.union_all([sb1, sb2]).order_by(["id"]).limit(10);
let (sql, _) = ub.build();
assert!(sql.contains("UNION ALL"));
```

### CREATE TABLE

```rust
use halo_space::sqlx::{
    create_table::CreateTableBuilder, create_table_define, create_table_option,
};

let mut ct = CreateTableBuilder::new();
ct.create_table("users").if_not_exists();
create_table_define!(ct, "id INT", "name TEXT");
create_table_option!(ct, "ENGINE=InnoDB");

let (sql, _) = ct.build();
assert!(sql.contains("CREATE TABLE"));
```

### Flavor 切换

```rust
use halo_space::sqlx::{Flavor, select::SelectBuilder, select_cols, from_tables};

let mut sb = SelectBuilder::new();
select_cols!(sb, "id");
from_tables!(sb, "user");

// 默认使用全局 Flavor；也可临时切换
let (pg_sql, _) = sb.build_with_flavor(Flavor::PostgreSQL, &[]);
let (mysql_sql, _) = sb.build_with_flavor(Flavor::MySQL, &[]);
assert!(pg_sql.contains("$1") || pg_sql.contains("$2")); // PostgreSQL 占位符
assert!(mysql_sql.contains("?")); // MySQL 占位符
```

### Args/占位符与命名参数

```rust
use halo_space::sqlx::{builder::build_named, modifiers::{SqlNamedArg, list, raw}};

let mut named = std::collections::HashMap::new();
named.insert("table".to_string(), raw("user"));
named.insert("ids".to_string(), list([1_i64, 2, 3]));
named.insert("now".to_string(), SqlNamedArg::new("t", 1_700_000_000_i64).into());

let (sql, args) = build_named(
    "SELECT * FROM ${table} WHERE id IN (${ids}) AND created_at > ${now}",
    named,
).build();
assert!(sql.contains("@t")); // 命名占位符
assert_eq!(args.len(), 0);   // named 参数不进入 args
```

### Build/Buildf 快速包装

```rust
use halo_space::sqlx::builder::{build, buildf};

let b1 = build("SELECT 1", ());
assert_eq!(b1.build().0, "SELECT 1");

let b2 = buildf("SELECT * FROM banned WHERE state IN (%v, %v)", [1, 2]);
assert_eq!(b2.build().1.len(), 2);
```

### Struct/field mapper/with_tag

```rust
use halo_space::sqlx::{field_mapper::snake_case_mapper, Struct};
let _guard = halo_space::sqlx::field_mapper::set_default_field_mapper_scoped(
    std::sync::Arc::new(snake_case_mapper),
);

halo_space::sqlx::sql_struct! {
    impl User {
        id: { db: "id", tags: [], omitempty: [], quote: false, as: None },
        user_name: { db: "", tags: [], omitempty: [], quote: false, as: None }
    }
}

let s = Struct::<User>::new().with_tag(["json"]); // 只选择带 json tag 的字段
let sb = s.select_from("user");
let (sql, _) = sb.build();
assert!(sql.contains("user.user_name"));
```

### Scan/Addr 拿到结果

```rust
use halo_space::sqlx::scan::{ScanCell, scan_tokens};

let tokens = vec!["id", "name", "42", "alice"];
let mut cells = [ScanCell::default(); 2];
let mut id: i64 = 0;
let mut name = String::new();
cells[0].addr(&mut id);
cells[1].addr(&mut name);

scan_tokens(&tokens, &mut cells).unwrap();
assert_eq!(id, 42);
assert_eq!(name, "alice");
```

### interpolate（非参数化场景）

```rust
use halo_space::sqlx::interpolate::interpolate_with_flavor;
use halo_space::sqlx::Flavor;

let (sql, _args) = interpolate_with_flavor(
    "SELECT * FROM user WHERE name = ? AND score >= ?",
    ["alice", 90],
    Flavor::MySQL,
).unwrap();
assert!(sql.contains("'alice'"));
assert!(sql.contains("90"));
```

### SqlValuer 延迟取值

```rust
use halo_space::sqlx::{valuer::SqlValuer, value::SqlValue};

struct Now;
impl SqlValuer for Now {
    fn to_sql_value(&self) -> Result<SqlValue, halo_space::sqlx::valuer::ValuerError> {
        Ok(SqlValue::I64(1_700_000_000)) // 示例：返回当前时间戳
    }
}

let now = Now;
let v: SqlValue = now.to_sql_value().unwrap();
assert_eq!(v, SqlValue::I64(1_700_000_000));
```

## 维护与测试

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## 许可证

MIT


