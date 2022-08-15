extern crate slab;

use std::{fmt, io, net, result, thread};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::Arc;

use log::{debug, info, trace};
use mysql_async::{
    Conn, from_row_opt, from_value, from_value_opt, QueryResult, Row, TextProtocol, Value,
};
use mysql_common::constants::{ColumnFlags, ColumnType};
use mysql_common::misc::raw::{Const, RawInt, Skip};
use mysql_common::misc::raw::int::{LeU16, LeU32};
use mysql_common::packets as mycp;
use mysql_common::packets::{ColumnDefinitionCatalog, FixedLengthFieldsLen};
use mysql_common::proto::MySerialize;
use mysql_common::row::new_row;
use redis::{Commands, RedisError};
use serde::{Serialize, Serializer};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, SerializeStruct};
use slab::Slab;
use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::{Parser, ParserError};
use opensrv_mysql::Column;

use crate::mysql_protocol::MySQLResult;

#[derive(Serialize,serde::Deserialize,Debug)]
#[serde(remote="Value")]
pub enum ValueRef{
    NULL,
    Bytes(Vec<u8>),
    Int(i64),
    UInt(u64),
    Float(f32),
    Double(f64),
    /// year, month, day, hour, minutes, seconds, micro seconds
    Date(u16, u8, u8, u8, u8, u8, u32),
    /// is negative, days, hours, minutes, seconds, micro seconds
    Time(bool, u32, u8, u8, u8, u32),
}

#[derive(Serialize,serde::Deserialize,Debug)]
pub struct ValueWrapper{
    #[serde(with="ValueRef")]
    pub v:Value,
}

#[derive(Serialize,serde::Deserialize,Debug)]
pub struct RowWrapper{
    values: Vec<Option<ValueWrapper>>,
    // columns: Arc<[ColumnRef]>,
}


#[derive(Debug, serde::Deserialize)]
enum ColumnTypeRef {
    MYSQL_TYPE_DECIMAL = 0,
    MYSQL_TYPE_TINY,
    MYSQL_TYPE_SHORT,
    MYSQL_TYPE_LONG,
    MYSQL_TYPE_FLOAT,
    MYSQL_TYPE_DOUBLE,
    MYSQL_TYPE_NULL,
    MYSQL_TYPE_TIMESTAMP,
    MYSQL_TYPE_LONGLONG,
    MYSQL_TYPE_INT24,
    MYSQL_TYPE_DATE,
    MYSQL_TYPE_TIME,
    MYSQL_TYPE_DATETIME,
    MYSQL_TYPE_YEAR,
    MYSQL_TYPE_NEWDATE,
    // Internal to MySql
    MYSQL_TYPE_VARCHAR,
    MYSQL_TYPE_BIT,
    MYSQL_TYPE_TIMESTAMP2,
    MYSQL_TYPE_DATETIME2,
    MYSQL_TYPE_TIME2,
    MYSQL_TYPE_TYPED_ARRAY,
    // Used for replication only
    MYSQL_TYPE_UNKNOWN = 243,
    MYSQL_TYPE_JSON = 245,
    MYSQL_TYPE_NEWDECIMAL = 246,
    MYSQL_TYPE_ENUM = 247,
    MYSQL_TYPE_SET = 248,
    MYSQL_TYPE_TINY_BLOB = 249,
    MYSQL_TYPE_MEDIUM_BLOB = 250,
    MYSQL_TYPE_LONG_BLOB = 251,
    MYSQL_TYPE_BLOB = 252,
    MYSQL_TYPE_VAR_STRING = 253,
    MYSQL_TYPE_STRING = 254,
    MYSQL_TYPE_GEOMETRY = 255,
}

impl ColumnTypeRef {
    fn from(col_type: &ColumnType) -> ColumnTypeRef {
        match col_type {
            ColumnType::MYSQL_TYPE_DECIMAL => ColumnTypeRef::MYSQL_TYPE_DECIMAL,
            ColumnType::MYSQL_TYPE_TINY => ColumnTypeRef::MYSQL_TYPE_TINY,
            ColumnType::MYSQL_TYPE_SHORT => ColumnTypeRef::MYSQL_TYPE_SHORT,
            ColumnType::MYSQL_TYPE_LONG => ColumnTypeRef::MYSQL_TYPE_LONG,
            ColumnType::MYSQL_TYPE_FLOAT => ColumnTypeRef::MYSQL_TYPE_FLOAT,
            ColumnType::MYSQL_TYPE_DOUBLE => ColumnTypeRef::MYSQL_TYPE_DOUBLE,
            ColumnType::MYSQL_TYPE_NULL => ColumnTypeRef::MYSQL_TYPE_NULL,
            ColumnType::MYSQL_TYPE_TIMESTAMP => ColumnTypeRef::MYSQL_TYPE_TIMESTAMP,
            ColumnType::MYSQL_TYPE_LONGLONG => ColumnTypeRef::MYSQL_TYPE_LONGLONG,
            ColumnType::MYSQL_TYPE_INT24 => ColumnTypeRef::MYSQL_TYPE_INT24,
            ColumnType::MYSQL_TYPE_DATE => ColumnTypeRef::MYSQL_TYPE_DATE,
            ColumnType::MYSQL_TYPE_TIME => ColumnTypeRef::MYSQL_TYPE_TIME,
            ColumnType::MYSQL_TYPE_DATETIME => ColumnTypeRef::MYSQL_TYPE_DATETIME,
            ColumnType::MYSQL_TYPE_YEAR => ColumnTypeRef::MYSQL_TYPE_YEAR,
            ColumnType::MYSQL_TYPE_NEWDATE => ColumnTypeRef::MYSQL_TYPE_NEWDATE,
            ColumnType::MYSQL_TYPE_VARCHAR => ColumnTypeRef::MYSQL_TYPE_VARCHAR,
            ColumnType::MYSQL_TYPE_BIT => ColumnTypeRef::MYSQL_TYPE_BIT,
            ColumnType::MYSQL_TYPE_TIMESTAMP2 => ColumnTypeRef::MYSQL_TYPE_TIMESTAMP2,
            ColumnType::MYSQL_TYPE_DATETIME2 => ColumnTypeRef::MYSQL_TYPE_DATETIME2,
            ColumnType::MYSQL_TYPE_TIME2 => ColumnTypeRef::MYSQL_TYPE_TIME2,
            ColumnType::MYSQL_TYPE_TYPED_ARRAY => ColumnTypeRef::MYSQL_TYPE_TYPED_ARRAY,
            ColumnType::MYSQL_TYPE_UNKNOWN => ColumnTypeRef::MYSQL_TYPE_UNKNOWN,
            ColumnType::MYSQL_TYPE_JSON => ColumnTypeRef::MYSQL_TYPE_JSON,
            ColumnType::MYSQL_TYPE_NEWDECIMAL => ColumnTypeRef::MYSQL_TYPE_NEWDECIMAL,
            ColumnType::MYSQL_TYPE_ENUM => ColumnTypeRef::MYSQL_TYPE_ENUM,
            ColumnType::MYSQL_TYPE_SET => ColumnTypeRef::MYSQL_TYPE_SET,
            ColumnType::MYSQL_TYPE_TINY_BLOB => ColumnTypeRef::MYSQL_TYPE_TINY_BLOB,
            ColumnType::MYSQL_TYPE_MEDIUM_BLOB => ColumnTypeRef::MYSQL_TYPE_MEDIUM_BLOB,
            ColumnType::MYSQL_TYPE_LONG_BLOB => ColumnTypeRef::MYSQL_TYPE_LONG_BLOB,
            ColumnType::MYSQL_TYPE_BLOB => ColumnTypeRef::MYSQL_TYPE_BLOB,
            ColumnType::MYSQL_TYPE_VAR_STRING => ColumnTypeRef::MYSQL_TYPE_VAR_STRING,
            ColumnType::MYSQL_TYPE_STRING => ColumnTypeRef::MYSQL_TYPE_STRING,
            ColumnType::MYSQL_TYPE_GEOMETRY => ColumnTypeRef::MYSQL_TYPE_GEOMETRY,
        }
    }
}

impl Serialize for ColumnTypeRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&*format!("{:?}", self))
    }
}

#[derive(Debug)]
struct ColumnRef {
    pub table: String,
    pub column: String,
    pub coltype: ColumnType,
    pub colflags: ColumnFlags,
}

impl ColumnRef {
    pub fn from_column(col:Column) -> ColumnRef {
        return ColumnRef{
            table: col.table,
            column: col.column,
            coltype: col.coltype,
            colflags: col.colflags,
        };
    }
}

// This is what #[derive(Serialize)] would generate.
impl Serialize for ColumnRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let mut s = serializer.serialize_struct("Person", 3)?;
        s.serialize_field("table", &self.table)?;
        s.serialize_field("column", &self.column)?;
        s.serialize_field("coltype", &ColumnTypeRef::from(&self.coltype))?;
        s.serialize_field("colflags", &*format!("{:?}", self.colflags))?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for ColumnRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        enum Field {
            TABLE,
            COLUMN,
            COLTYPE,
            COLFLAGS,
        }

        // This part could also be generated independently by:
        //
        //    #[derive(Deserialize)]
        //    #[serde(field_identifier, rename_all = "lowercase")]
        //    enum Field { Secs, Nanos }
        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
                where
                    D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("One of `TABLE,COLUMN,COLTYPE,COLFLAGS`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                        where
                            E: de::Error,
                    {
                        match value {
                            "table" => Ok(Field::TABLE),
                            "column" => Ok(Field::COLUMN),
                            "coltype" => Ok(Field::COLTYPE),
                            "colflags" => Ok(Field::COLFLAGS),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ColumnRefVisitor;

        impl<'de> Visitor<'de> for ColumnRefVisitor {
            type Value = ColumnRef;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct ColumnRef")
            }

            // fn visit_seq<V>(self, mut seq: V) -> Result<ColumnRef, V::Error>
            //     where
            //         V: SeqAccess<'de>,
            // {
            //     let secs = seq.next_element()?
            //         .ok_or_else(|| de::Error::invalid_length(0, &self))?;
            //     let nanos = seq.next_element()?
            //         .ok_or_else(|| de::Error::invalid_length(1, &self))?;
            //     Ok(ColumnRef::new(secs, nanos))
            // }

            fn visit_map<V>(self, mut map: V) -> Result<ColumnRef, V::Error>
                where
                    V: MapAccess<'de>,
            {
                let mut table = String::from("");
                let mut column = String::from("");
                let mut coltype = ColumnType::MYSQL_TYPE_VARCHAR;
                let mut colflags = ColumnFlags::default();
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::TABLE => {
                            table = map.next_value()?;
                        }
                        Field::COLUMN => {
                            column = map.next_value()?;
                        }
                        Field::COLTYPE => {
                            let col_type_ref: ColumnTypeRef = map.next_value()?;
                            coltype = match col_type_ref {
                                MYSQL_TYPE_DECIMAL => ColumnType::MYSQL_TYPE_DECIMAL,
                                MYSQL_TYPE_TINY => ColumnType::MYSQL_TYPE_TINY,
                                MYSQL_TYPE_SHORT => ColumnType::MYSQL_TYPE_SHORT,
                                MYSQL_TYPE_LONG => ColumnType::MYSQL_TYPE_LONG,
                                MYSQL_TYPE_FLOAT => ColumnType::MYSQL_TYPE_FLOAT,
                                MYSQL_TYPE_DOUBLE => ColumnType::MYSQL_TYPE_DOUBLE,
                                MYSQL_TYPE_NULL => ColumnType::MYSQL_TYPE_NULL,
                                MYSQL_TYPE_TIMESTAMP => ColumnType::MYSQL_TYPE_TIMESTAMP,
                                MYSQL_TYPE_LONGLONG => ColumnType::MYSQL_TYPE_LONGLONG,
                                MYSQL_TYPE_INT24 => ColumnType::MYSQL_TYPE_INT24,
                                MYSQL_TYPE_DATE => ColumnType::MYSQL_TYPE_DATE,
                                MYSQL_TYPE_TIME => ColumnType::MYSQL_TYPE_TIME,
                                MYSQL_TYPE_DATETIME => ColumnType::MYSQL_TYPE_DATETIME,
                                MYSQL_TYPE_YEAR => ColumnType::MYSQL_TYPE_YEAR,
                                MYSQL_TYPE_NEWDATE => ColumnType::MYSQL_TYPE_NEWDATE,
                                MYSQL_TYPE_VARCHAR => ColumnType::MYSQL_TYPE_VARCHAR,
                                MYSQL_TYPE_BIT => ColumnType::MYSQL_TYPE_BIT,
                                MYSQL_TYPE_TIMESTAMP2 => ColumnType::MYSQL_TYPE_TIMESTAMP2,
                                MYSQL_TYPE_DATETIME2 => ColumnType::MYSQL_TYPE_DATETIME2,
                                MYSQL_TYPE_TIME2 => ColumnType::MYSQL_TYPE_TIME2,
                                MYSQL_TYPE_TYPED_ARRAY => ColumnType::MYSQL_TYPE_TYPED_ARRAY,
                                MYSQL_TYPE_UNKNOWN => ColumnType::MYSQL_TYPE_UNKNOWN,
                                MYSQL_TYPE_JSON => ColumnType::MYSQL_TYPE_JSON,
                                MYSQL_TYPE_NEWDECIMAL => ColumnType::MYSQL_TYPE_NEWDECIMAL,
                                MYSQL_TYPE_ENUM => ColumnType::MYSQL_TYPE_ENUM,
                                MYSQL_TYPE_SET => ColumnType::MYSQL_TYPE_SET,
                                MYSQL_TYPE_TINY_BLOB => ColumnType::MYSQL_TYPE_TINY_BLOB,
                                MYSQL_TYPE_MEDIUM_BLOB => ColumnType::MYSQL_TYPE_MEDIUM_BLOB,
                                MYSQL_TYPE_LONG_BLOB => ColumnType::MYSQL_TYPE_LONG_BLOB,
                                MYSQL_TYPE_BLOB => ColumnType::MYSQL_TYPE_BLOB,
                                MYSQL_TYPE_VAR_STRING => ColumnType::MYSQL_TYPE_VAR_STRING,
                                MYSQL_TYPE_STRING => ColumnType::MYSQL_TYPE_STRING,
                                MYSQL_TYPE_GEOMETRY => ColumnType::MYSQL_TYPE_GEOMETRY,
                            }
                        }
                        Field::COLFLAGS => {
                            let colflags_str: String = map.next_value()?;
                            colflags = match colflags_str.as_str() {
                                "NOT_NULL_FLAG" => ColumnFlags::NOT_NULL_FLAG,
                                "PRI_KEY_FLAG" => ColumnFlags::PRI_KEY_FLAG,
                                "UNIQUE_KEY_FLAG" => ColumnFlags::UNIQUE_KEY_FLAG,
                                "MULTIPLE_KEY_FLAG" => ColumnFlags::MULTIPLE_KEY_FLAG,
                                "BLOB_FLAG" => ColumnFlags::BLOB_FLAG,
                                "UNSIGNED_FLAG" => ColumnFlags::UNSIGNED_FLAG,
                                "ZEROFILL_FLAG" => ColumnFlags::ZEROFILL_FLAG,
                                "BINARY_FLAG" => ColumnFlags::BINARY_FLAG,
                                "ENUM_FLAG" => ColumnFlags::ENUM_FLAG,
                                "AUTO_INCREMENT_FLAG" => ColumnFlags::AUTO_INCREMENT_FLAG,
                                "TIMESTAMP_FLAG" => ColumnFlags::TIMESTAMP_FLAG,
                                "SET_FLAG" => ColumnFlags::SET_FLAG,
                                "NO_DEFAULT_VALUE_FLAG" => ColumnFlags::NO_DEFAULT_VALUE_FLAG,
                                "ON_UPDATE_NOW_FLAG" => ColumnFlags::ON_UPDATE_NOW_FLAG,
                                "PART_KEY_FLAG" => ColumnFlags::PART_KEY_FLAG,
                                "NUM_FLAG" => ColumnFlags::NUM_FLAG,
                                _ => ColumnFlags::default(),
                            };
                        }
                    }
                }
                Ok(ColumnRef {
                    table,
                    column,
                    coltype,
                    colflags,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["TABLE", "COLUMN", "COLTYPE", "COLFLAGS"];
        deserializer.deserialize_struct("ColumnRef", FIELDS, ColumnRefVisitor)
    }
}

impl Serialize for MySQLResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let mut rows_v: Vec<RowWrapper> = vec![];
        for row in &self.rows {
            let mut row_v = vec![];
            for col in &self.cols {
                // trace!("col:{:?}", col);
                let column_value = &row[col.column.as_ref()];
                let v = ValueWrapper{
                    v: column_value.clone(),
                };
                // let col_json_v = serde_json::to_string(&v)?;
                // trace!("col_json_v:{:?}",col_json_v);
                row_v.push(Some(v));
            }
            let row_wrapper = RowWrapper{
                values: row_v,
            };
            rows_v.push(row_wrapper);
        }
        let cols_v: Vec<ColumnRef> = self
            .cols
            .iter()
            .map(|c| ColumnRef {
                column: c.column.clone(),
                table: c.table.clone(),
                coltype: c.coltype.clone(),
                colflags: c.colflags,
            })
            .collect();

        let mut struct_ser = serializer.serialize_struct("MySQLResult", 2)?;
        struct_ser.serialize_field("cols", &cols_v);
        struct_ser.serialize_field("rows", &rows_v);
        return struct_ser.end();
    }
}

impl<'de> Deserialize<'de> for MySQLResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Rows,
            Cols,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
                where
                    D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`rows` or `cols`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                        where
                            E: de::Error,
                    {
                        match value {
                            "cols" => Ok(Field::Cols),
                            "rows" => Ok(Field::Rows),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct MySQLResultVisitor;

        impl<'de> Visitor<'de> for MySQLResultVisitor {
            type Value = MySQLResult;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct MySQLResult")
            }

            // fn visit_seq<V>(self, mut seq: V) -> Result<MySQLResult, V::Error>
            //     where
            //         V: SeqAccess<'de>,
            // {
            //     let rows = seq.next_element()?
            //         .ok_or_else(|| de::Error::invalid_length(0, &self))?;
            //     let cols = seq.next_element()?
            //         .ok_or_else(|| de::Error::invalid_length(1, &self))?;
            //     Ok(MySQLResult {
            //         rows: rows,
            //         cols: cols,
            //     })
            // }

            fn visit_map<V>(self, mut map: V) -> Result<MySQLResult, V::Error>
                where
                    V: MapAccess<'de>,
            {
                let mut cols = vec![];
                let mut rows = vec![];
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Cols => {
                            trace!("key:{:?}",key);
                            let cols_refs: Vec<ColumnRef> = map.next_value()?;
                            for cr in cols_refs {
                                cols.push(Column {
                                    table: cr.table,
                                    column: cr.column,
                                    coltype: cr.coltype,
                                    colflags: cr.colflags,
                                });
                            }
                        },
                        Field::Rows => {
                            trace!("key:{:?}",key);
                            let row: Vec<RowWrapper> = map.next_value().unwrap();
                            for row in row {
                                let mut row_v = vec![];
                                for col in row.values {
                                    row_v.push(col.unwrap().v);
                                }

                                // let col_slice = &cols[..];
                                let mycp_cols: Vec<mycp::Column> = cols.iter()
                                    .map(|col| {
                                        let r_col = mycp::Column::new(col.coltype)
                                            .with_table(col.table.as_ref())
                                            .with_name(col.column.as_ref())
                                            .with_flags(col.colflags);
                                        r_col
                                        // catalog: ColumnDefinitionCatalog,
                                        // schema: SmallVec<[u8; 16]>,
                                        // table: SmallVec<[u8; 16]>,
                                        // org_table: SmallVec<[u8; 16]>,
                                        // name: SmallVec<[u8; 16]>,
                                        // org_name: SmallVec<[u8; 16]>,
                                        // fixed_length_fields_len: FixedLengthFieldsLen,
                                        // column_length: RawInt<LeU32>,
                                        // character_set: RawInt<LeU16>,
                                        // column_type: Const<ColumnType, u8>,
                                        // flags: Const<ColumnFlags, LeU16>,
                                        // decimals: RawInt<u8>,
                                        // __filler: Skip<2>,
                                    })
                                    .collect();
                                let real_row = new_row(row_v, Arc::from(mycp_cols));
                                rows.push(real_row);
                            }
                        }
                    };
                }
                Ok(MySQLResult { rows, cols })
            }
        }

        const FIELDS: &'static [&'static str] = &["rows", "cols"];
        deserializer.deserialize_struct("MySQLResult", FIELDS, MySQLResultVisitor)
    }
}

fn column_vec_ser<S: Serializer>(vec: &Vec<Column>, serializer: S) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(vec.len()))?;
    for c in vec {
        let col_ref = ColumnRef {
            column: c.column.clone(),
            table: c.table.clone(),
            coltype: c.coltype.clone(),
            colflags: c.colflags,
        };
        seq.serialize_element(&col_ref)?;
    }
    seq.end()
}
