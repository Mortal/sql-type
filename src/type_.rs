// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::RefOrVal;
use alloc::{
    borrow::Cow,
    fmt::{Display, Write},
    vec::Vec,
};
use sql_parse::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseType {
    Any,
    Bool,
    Bytes,
    Date,
    DateTime,
    Float,
    Integer,
    String,
    Time,
    TimeStamp,
}

impl Display for BaseType {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        match self {
            BaseType::Any => f.write_str("any"),
            BaseType::Bool => f.write_str("bool"),
            BaseType::Bytes => f.write_str("bytes"),
            BaseType::Date => f.write_str("date"),
            BaseType::DateTime => f.write_str("datetime"),
            BaseType::Float => f.write_str("float"),
            BaseType::Integer => f.write_str("integer"),
            BaseType::String => f.write_str("string"),
            BaseType::Time => f.write_str("time"),
            BaseType::TimeStamp => f.write_str("timestamp"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<'a> {
    Args(BaseType, Vec<(usize, Span)>),
    Base(BaseType),
    Enum(RefOrVal<'a, Vec<Cow<'a, str>>>),
    F32,
    F64,
    I16,
    I32,
    I64,
    I8,
    Invalid,
    JSON,
    Set(RefOrVal<'a, Vec<Cow<'a, str>>>),
    U16,
    U32,
    U64,
    U8,
    Null,
}

impl<'a> Display for Type<'a> {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        match self {
            Type::Args(t, a) => {
                write!(f, "args({}", t)?;
                for (a, _) in a {
                    write!(f, ", {}", a)?;
                }
                f.write_char(')')
            }
            Type::Base(t) => t.fmt(f),
            Type::F32 => f.write_str("f32"),
            Type::F64 => f.write_str("f64"),
            Type::I16 => f.write_str("i16"),
            Type::I32 => f.write_str("i32"),
            Type::I64 => f.write_str("i64"),
            Type::I8 => f.write_str("i8"),
            Type::Invalid => f.write_str("invalid"),
            Type::JSON => f.write_str("json"),
            Type::U16 => f.write_str("u16"),
            Type::U32 => f.write_str("u32"),
            Type::U64 => f.write_str("u64"),
            Type::U8 => f.write_str("u8"),
            Type::Null => f.write_str("null"),
            Type::Enum(v) => {
                f.write_str("enum(")?;
                for (i, v) in v.iter().enumerate() {
                    if i != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "'{}'", v)?
                }
                f.write_char(')')
            }
            Type::Set(v) => {
                f.write_str("set(")?;
                for (i, v) in v.iter().enumerate() {
                    if i != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "'{}'", v)?
                }
                f.write_char(')')
            }
        }
    }
}

impl<'a> Type<'a> {
    pub fn ref_clone(&'a self) -> Self {
        match self {
            Type::Enum(e) => Type::Enum(e.ref_clone()),
            Type::Set(e) => Type::Set(e.ref_clone()),
            t => t.clone(),
        }
    }

    pub fn base(&self) -> BaseType {
        match self {
            Type::Args(t, _) => *t,
            Type::Base(t) => *t,
            Type::Enum(_) => BaseType::String,
            Type::F32 => BaseType::Float,
            Type::F64 => BaseType::Float,
            Type::I16 => BaseType::Integer,
            Type::I32 => BaseType::Integer,
            Type::I64 => BaseType::Integer,
            Type::I8 => BaseType::Integer,
            Type::Invalid => BaseType::Any,
            Type::JSON => BaseType::Any,
            Type::Null => BaseType::Any,
            Type::Set(_) => BaseType::String,
            Type::U16 => BaseType::Integer,
            Type::U32 => BaseType::Integer,
            Type::U64 => BaseType::Integer,
            Type::U8 => BaseType::Integer,
        }
    }
}

impl<'a> From<BaseType> for Type<'a> {
    fn from(t: BaseType) -> Self {
        Type::Base(t)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullType<'a> {
    pub t: Type<'a>,
    pub not_null: bool,
}

impl<'a> FullType<'a> {
    pub fn ref_clone(&'a self) -> Self {
        FullType {
            t: self.t.ref_clone(),
            not_null: self.not_null,
        }
    }
    pub fn new(t: impl Into<Type<'a>>, not_null: bool) -> Self {
        Self {
            t: t.into(),
            not_null,
        }
    }
    pub fn invalid() -> Self {
        Self {
            t: Type::Invalid,
            not_null: false,
        }
    }
}

impl<'a> core::ops::Deref for FullType<'a> {
    type Target = Type<'a>;

    fn deref(&self) -> &Self::Target {
        &self.t
    }
}

impl<'a> Display for FullType<'a> {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        self.t.fmt(f)?;
        if self.not_null {
            f.write_str(" not null")
        } else {
            Ok(())
        }
    }
}
