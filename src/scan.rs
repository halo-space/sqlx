//! Scan：为 `Struct::addr*` 提供可写入的“扫描目标”（对齐 go-sqlbuilder 的 Addr/Scan 体验）。
//!
//! go 中 `database/sql` 通过 `Scan(dest...)` 写入指针；Rust 没有统一反射式 Scan API。
//! 本实现提供一个最小子集：把“字符串 token”写入到字段（用于对齐 go 的单测与示例）。

use crate::valuer::SqlValuer;
use std::marker::PhantomData;

/// 扫描/解析错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScanError {
    #[error("builder not enough tokens")]
    NotEnoughTokens,
    #[error("builder failed to parse int: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
    #[error("builder failed to parse float")]
    ParseFloat,
    #[error("builder failed to parse bool")]
    ParseBool,
    #[error("builder scan into Option<T> is not supported")]
    UnsupportedOption,
    #[error("builder scan into this type is not supported")]
    UnsupportedType,
}

/// 从字符串 token 写入自身（最小子集）。
pub trait ScanFromStr {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError>;
}

impl ScanFromStr for String {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        self.clear();
        self.push_str(s);
        Ok(())
    }
}

impl ScanFromStr for i64 {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        *self = s.parse::<i64>()?;
        Ok(())
    }
}

impl ScanFromStr for i32 {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        *self = s.parse::<i32>()?;
        Ok(())
    }
}

impl ScanFromStr for u64 {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        *self = s.parse::<u64>()?;
        Ok(())
    }
}

impl ScanFromStr for u16 {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        *self = s.parse::<u16>()?;
        Ok(())
    }
}

impl ScanFromStr for f64 {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        *self = s.parse::<f64>().map_err(|_| ScanError::ParseFloat)?;
        Ok(())
    }
}

impl ScanFromStr for bool {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        match s {
            "true" | "TRUE" | "1" => {
                *self = true;
                Ok(())
            }
            "false" | "FALSE" | "0" => {
                *self = false;
                Ok(())
            }
            _ => Err(ScanError::ParseBool),
        }
    }
}

impl<T: ScanFromStr> ScanFromStr for Option<T> {
    fn scan_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        if s.eq_ignore_ascii_case("null") {
            *self = None;
            return Ok(());
        }
        let _ = s;
        Err(ScanError::UnsupportedOption)
    }
}

impl ScanFromStr for Box<dyn SqlValuer> {
    fn scan_from_str(&mut self, _s: &str) -> Result<(), ScanError> {
        Err(ScanError::UnsupportedType)
    }
}

type Setter = fn(*mut (), &str) -> Result<(), ScanError>;

fn set_impl<T: ScanFromStr>(ptr: *mut (), s: &str) -> Result<(), ScanError> {
    // SAFETY: ptr 由宏从真实字段地址构造，且 lifetime 由 ScanCell 约束。
    let r = unsafe { &mut *(ptr as *mut T) };
    r.scan_from_str(s)
}

/// 一个可写入的扫描目标（类似 go 的指针 dest）。
#[derive(Debug)]
pub struct ScanCell<'a> {
    ptr: *mut (),
    set: Setter,
    _pd: PhantomData<&'a mut ()>,
}

impl<'a> ScanCell<'a> {
    pub fn from_ptr<T: ScanFromStr>(ptr: *mut T) -> Self {
        Self {
            ptr: ptr as *mut (),
            set: set_impl::<T>,
            _pd: PhantomData,
        }
    }

    pub fn set_from_str(&mut self, s: &str) -> Result<(), ScanError> {
        (self.set)(self.ptr, s)
    }
}

/// 按空白分割输入，把每个 token 写入对应的 dest。
pub fn scan_tokens(input: &str, mut dests: Vec<ScanCell<'_>>) -> Result<(), ScanError> {
    let mut it = input.split_whitespace();
    for d in dests.iter_mut() {
        let token = it.next().ok_or(ScanError::NotEnoughTokens)?;
        d.set_from_str(token)?;
    }
    Ok(())
}
