extern crate msql_srv;
extern crate slab;

use std::{io, net, result, thread};
use std::borrow::Borrow;
use std::collections::HashMap;

use log::{debug, info, trace};
use mysql::{
    Conn, from_row_opt, from_value, from_value_opt, QueryResult, Row, serde_json, Text, Value,
};
use mysql::consts::ColumnType;
use mysql::prelude::Queryable;
use mysql::serde_json::error;
use redis::{Commands, RedisError};
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeStruct;
use slab::Slab;
use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::{Parser, ParserError};

use msql_srv::{
    Column, ColumnFlags, ErrorKind, InitWriter, MysqlIntermediary, MysqlShim, ParamParser,
    QueryResultWriter, StatementMetaWriter,
};

fn handle_mysql_result(
    query_result_result: Result<QueryResult<Text>, mysql::Error>,
) -> Result<MySQLResult, mysql::Error> {
    match query_result_result {
        Ok(query_result) => {
            trace!("columns:{:?}", query_result.columns());
            let cols: Vec<Column> = query_result
                .columns()
                .as_ref()
                .iter()
                .map(|c| {
                    let t = c.column_type();
                    Column {
                        table: String::from(c.schema_str().to_owned()),
                        column: String::from(c.name_str().to_owned()),
                        coltype: t,
                        colflags: c.flags(),
                    }
                })
                .collect();

            let rows: Vec<Row> = query_result.flatten().collect();
            let mysql_result = MySQLResult { cols, rows};
            return Ok(mysql_result);
        }
        Err(err) => {
            error!("get mysql err:{:?}", err);
            return Err(err);
        }
    }
}


// this is where the proxy server implementation starts

struct Prepared {
    stmt: mysql::Statement,
    params: Vec<Column>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(remote="ColumnType")]
enum ColumnTypeRef{
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
    MYSQL_TYPE_NEWDATE, // Internal to MySql
    MYSQL_TYPE_VARCHAR,
    MYSQL_TYPE_BIT,
    MYSQL_TYPE_TIMESTAMP2,
    MYSQL_TYPE_DATETIME2,
    MYSQL_TYPE_TIME2,
    MYSQL_TYPE_TYPED_ARRAY, // Used for replication only
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

#[derive(Debug, Serialize
// , serde::Deserialize
)]
#[serde(remote = "Column")]
struct ColumnRef{
    pub table: String,
    pub column: String,
    #[serde(with="ColumnTypeRef")]
    pub coltype: ColumnType,
    #[serde(skip_serializing)]
    pub colflags: ColumnFlags,
}

// #[derive(serde::Serialize,
// serde::Deserialize
// )]
struct MySQLResult {
    #[serde(serialize_with = "column_vec_ser")]
    // #[serde(deserialize_with = "column_vec_deser")]
    cols: Vec<Column>,
    rows: Vec<Row>,
}

fn column_vec_ser<S: Serializer>(
    vec: &Vec<Column>,
    serializer: S
) -> Result<S::Ok, S::Error> {
    let col_ref_vec:Vec<ColumnRef> = vec.iter().map(|c|{
        return ColumnRef{
            column:c.column.clone(),
            table: c.table.clone(),
            coltype: c.coltype.clone(),
            colflags: c.colflags,
        }
    })
        .collect();
    // let mut col_slice = [&ColumnRef;col_ref_vec.len()];
    // for (idx,col_ref) in col_ref_vec.as_slice().iter().enumerate() {
    //     let ref_v = col_ref.clone();
    //     col_slice[idx] = ref_v;
    // }
    let slice:[&ColumnRef] = col_ref_vec.as_slice()
        .iter()
        .map(|c|{
            c.to_owned()
        })
        .collect()
        .as_slice();
    slice.serialize(serializer)
    // vec2.serialize(serializer)
}

pub struct MySQL {
    connection: Conn,
    redis_conn: redis::Connection,
    // NOTE: not *actually* static, but tied to our connection's lifetime.
    prepared: Slab<Prepared>,
    dialect: MySqlDialect,
}

impl MySQL {
    pub fn new(c: mysql::Conn, redis_conn: redis::Connection) -> Self {
        MySQL {
            connection: c,
            redis_conn,
            prepared: Slab::new(),
            dialect: MySqlDialect {},
        }
    }
}

#[derive(Debug)]
pub enum Error {
    MySQL(mysql::Error),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<mysql::Error> for Error {
    fn from(e: mysql::Error) -> Self {
        Error::MySQL(e)
    }
}

impl<W: io::Read + io::Write> MysqlShim<W> for MySQL {
    type Error = Error;

    fn on_prepare(&mut self, query: &str, info: StatementMetaWriter<W>) -> Result<(), Self::Error> {
        match self.connection.prep(query) {
            Ok(stmt) => {
                // the PostgreSQL server will tell us about the parameter types and output columns
                // of the query we just prepared. we now need to communicate this back to our MySQL
                // client, which requires translating between psql and mysql types.
                use std::mem;
                let params: Vec<_> = stmt
                    .params()
                    .into_iter()
                    .map(|t| {
                        let rt = t.column_type();
                        Column {
                            table: String::from(t.schema_str()),
                            column: String::from(t.name_str().to_owned()),
                            coltype: rt,
                            colflags: ColumnFlags::empty(),
                        }
                    })
                    .collect();
                let columns: Vec<_> = stmt
                    .columns()
                    .into_iter()
                    .map(|c| {
                        let t = c.column_type();
                        Column {
                            table: String::from(c.schema_str().to_owned()),
                            column: String::from(c.name_str().to_owned()),
                            coltype: c.column_type(),
                            colflags: ColumnFlags::empty(),
                        }
                    })
                    .collect();

                // keep track of the parameter types so we can decode the values provided by the
                // client when they later execute this statement.
                let stmt = Prepared { stmt, params };

                // the statement is tied to the connection, which as far as the compiler is aware
                // we only know lives for as long as the `&mut self` given to this function.
                // however, *we* know that the connection will live at least as long as the
                // prepared statement we insert into `self.prepared` (because there is no way to
                // get the prepared statements out!).
                let stmt = unsafe { mem::transmute(stmt) };

                let id = self.prepared.insert(stmt);
                let stmt = &self.prepared[id];
                info.reply(id as u32, &stmt.params, &columns)?;
                Ok(())
            }

            Err(e) => Err(e.into()),
        }
    }

    fn on_execute(
        &mut self,
        id: u32,
        ps: ParamParser,
        results: QueryResultWriter<W>,
    ) -> Result<(), Self::Error> {
        match self.prepared.get_mut(id as usize) {
            None => Ok(results.error(ErrorKind::ER_NO, b"no such prepared statement")?),
            Some(&mut Prepared { ref mut stmt, .. }) => {
                // let args = ps.into_iter()
                //     .map(|p|{
                //         p.coltype
                //     })
                //     .collect();
                // self.connection.query(stmt, &args[..])
                // let types = ps.into_iter()
                //     .map(|f| {
                //         f.coltype
                //     })
                //     .collect();
                // String::from("");
                // self.connection.query();
                Ok(results.error(ErrorKind::ER_NO, b"no such prepared statement")?)
            }
        }
    }

    fn on_close(&mut self, id: u32) {
        debug!("connection {} closed", id);
        self.prepared.remove(id as usize);
    }

    fn on_query(&mut self, sql: &str, results: QueryResultWriter<W>) -> Result<(), Self::Error> {
        // let r = self.connection.query(query);
        trace!("sql:{}", sql);
        let redis_key = format!("cache:{:?}", sql);
        let ast_opt = Parser::parse_sql(&self.dialect, sql);
        if let Ok(asts) = ast_opt {
            for ast in asts {
                trace!("ast:{:?}", ast);
                if let Statement::Query(boxed_query) = ast {
                    let query = boxed_query.as_ref();
                    trace!("ast query:{:?}", query);
                    if let SetExpr::Select(_select_expr_box) = &query.body {
                        // let select_expr = select_expr_box.as_ref();
                        // for table in &select_expr.from {
                        //     info!("table:{:?}", table);
                        // }
                        let cached_value_result: Result<String, RedisError> =
                            self.redis_conn.get(redis_key.clone());
                        match cached_value_result {
                            Ok(v) => {
                                trace!("cached value:{}", v);
                            }
                            Err(err) => {
                                error!("redis.get error:{:?}", err);
                            }
                        };
                    }
                    trace!("ast query body:{:?}", query.body);
                }
            }
        }

        let query_result_result: Result<QueryResult<Text>, mysql::Error> =
            self.connection.query_iter(String::from(sql));
        trace!("v:{:?}", query_result_result);
        let mysql_result_opt = handle_mysql_result(query_result_result);
        match mysql_result_opt {
            Ok(mysql_result) => {
                // let json_v = serde_json::to_string(&mysql_result)
                //     .unwrap_or_default();
                // trace!("json_v:{}",json_v);
                // self.redis_conn.set(redis_key.clone(), redis_v.as_str());
                let mut writer = results.start(&mysql_result.cols)?;
                for row in mysql_result.rows {
                    trace!("row:{:?}", row);
                    for col in &mysql_result.cols {
                        trace!("col:{:?}", col);
                        let column_value = &row[col.column.as_ref()];
                        trace!("blob column_value:{:?}", column_value);
                        writer.write_col(column_value)?;
                    }
                    writer.end_row()?;
                }
                Ok(writer.finish()?)
            }
            Err(err) => {
                Ok(results.error(ErrorKind::ER_NO, err.to_string().as_bytes())?)
            }
        }
    }

    fn on_init(&mut self, schema: &str, writer: InitWriter<'_, W>) -> Result<(), Self::Error> {
        debug!("schema:{}", schema);
        self.connection.select_db(schema);
        writer.ok()?;
        Ok(())
    }
}

impl Drop for MySQL {
    fn drop(&mut self) {
        // drop all the prepared statements *first*.
        self.prepared.clear();
        // *then* we can drop the connection (implicitly done).
    }
}