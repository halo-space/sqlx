//! SqlValuer：对齐 go-sqlbuilder 对 `database/sql/driver.Valuer` 的支持（最小子集）。
//!
//! 在 go 里，`driver.Valuer` 可以在插值阶段被调用，得到最终可序列化值。
//! Rust 没有统一的标准 trait；这里提供一个 crate 内 trait，供用户/测试实现。

use crate::value::SqlValue;

/// Valuer 错误（对齐 go 的 `Value() (driver.Value, error)`）。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("builder sql valuer error: {0}")]
pub struct ValuerError(pub String);

/// 可在插值阶段动态计算实际值的 trait。
pub trait SqlValuer: dyn_clone::DynClone + std::fmt::Debug {
    fn value(&self) -> Result<SqlValue, ValuerError>;
}

dyn_clone::clone_trait_object!(SqlValuer);
