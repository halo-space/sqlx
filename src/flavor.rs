//! SQL Flavor（方言）：控制占位符、Quote、Interpolate 等行为。

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard};

/// 与 go-sqlbuilder `Flavor` 对齐的方言枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Flavor {
    #[default]
    MySQL,
    PostgreSQL,
    SQLite,
    SQLServer,
    CQL,
    ClickHouse,
    Presto,
    Oracle,
    Informix,
    Doris,
}

static DEFAULT_FLAVOR: AtomicU8 = AtomicU8::new(Flavor::MySQL as u8);
static DEFAULT_FLAVOR_LOCK: Mutex<()> = Mutex::new(());

impl Flavor {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::MySQL,
            1 => Self::PostgreSQL,
            2 => Self::SQLite,
            3 => Self::SQLServer,
            4 => Self::CQL,
            5 => Self::ClickHouse,
            6 => Self::Presto,
            7 => Self::Oracle,
            8 => Self::Informix,
            9 => Self::Doris,
            _ => Self::MySQL,
        }
    }

    fn to_u8(self) -> u8 {
        self as u8
    }
}

/// 获取当前全局默认 Flavor（对齐 go-sqlbuilder `DefaultFlavor`）。
pub fn default_flavor() -> Flavor {
    Flavor::from_u8(DEFAULT_FLAVOR.load(Ordering::Relaxed))
}

/// 设置全局默认 Flavor，返回旧值（对齐 go-sqlbuilder 的用法习惯）。
pub fn set_default_flavor(flavor: Flavor) -> Flavor {
    let old = DEFAULT_FLAVOR.swap(flavor.to_u8(), Ordering::Relaxed);
    Flavor::from_u8(old)
}

/// 修改全局默认 Flavor 的 RAII guard（会持有一个全局锁，避免并行测试互相干扰）。
pub struct DefaultFlavorGuard {
    _lock: MutexGuard<'static, ()>,
    old: Flavor,
}

impl Drop for DefaultFlavorGuard {
    fn drop(&mut self) {
        set_default_flavor(self.old);
    }
}

/// 在一个作用域内临时设置 DefaultFlavor，并保证退出作用域后自动恢复。
pub fn set_default_flavor_scoped(flavor: Flavor) -> DefaultFlavorGuard {
    let lock = DEFAULT_FLAVOR_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let old = set_default_flavor(flavor);
    DefaultFlavorGuard { _lock: lock, old }
}

impl fmt::Display for Flavor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::MySQL => "MySQL",
            Self::PostgreSQL => "PostgreSQL",
            Self::SQLite => "SQLite",
            Self::SQLServer => "SQLServer",
            Self::CQL => "CQL",
            Self::ClickHouse => "ClickHouse",
            Self::Presto => "Presto",
            Self::Oracle => "Oracle",
            Self::Informix => "Informix",
            Self::Doris => "Doris",
        };
        f.write_str(s)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InterpolateError {
    #[error("builder interpolation for this flavor is not implemented")]
    NotImplemented,
    #[error("builder not enough args when interpolating")]
    MissingArgs,
    #[error("builder unsupported args when interpolating")]
    UnsupportedArgs,
    #[error("{0}")]
    ValuerError(#[from] crate::valuer::ValuerError),
}

impl Flavor {
    /// 对齐 go-sqlbuilder `Flavor#Quote`：为标识符加引号。
    pub fn quote(self, name: &str) -> String {
        match self {
            Self::MySQL | Self::ClickHouse | Self::Doris => format!("`{name}`"),
            Self::PostgreSQL
            | Self::SQLServer
            | Self::SQLite
            | Self::Presto
            | Self::Oracle
            | Self::Informix => {
                format!("\"{name}\"")
            }
            Self::CQL => format!("'{name}'"),
        }
    }

    /// 对齐 go-sqlbuilder `Flavor.PrepareInsertIgnore` 的核心逻辑。
    pub fn prepare_insert_ignore(self) -> &'static str {
        match self {
            Flavor::MySQL | Flavor::Oracle => "INSERT IGNORE",
            Flavor::PostgreSQL => "INSERT",
            Flavor::SQLite => "INSERT OR IGNORE",
            _ => "INSERT",
        }
    }
}
