extern crate slab;

use std::{io, net, result, thread};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use log::{debug, error, info, trace};
use metrics::histogram;
use mysql_async::{Conn, Error, from_row_opt, from_value, from_value_opt, QueryResult, Row, TextProtocol, Value};
use mysql_async::consts::ColumnType;
use mysql_async::prelude::Queryable;
use nom::combinator::iterator;
use redis::{AsyncCommands, Commands, RedisError, RedisResult};
use redis::aio::Connection;
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::{SerializeSeq, SerializeStruct};
use slab::Slab;
use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::{Parser, ParserError};

use opensrv_mysql::{AsyncMysqlIntermediary, AsyncMysqlShim, Column, ColumnFlags, ErrorKind, InitWriter, OkResponse, ParamParser, QueryResultWriter, StatementMetaWriter};
use tokio::io::AsyncWrite;

//
// async fn handle_mysql_result<'a>(
//     query_result_result: &'a Result<&'a QueryResult<TextProtocol>, &'a mysql_async::Error>,
// ) -> Result<MySQLResult, mysql_async::Error> {
//     return match query_result_result {
//         Ok(mut query_result) => {
//             trace!("columns:{:?}", query_result.columns());
//             let cols: Vec<Column> = query_result
//                 .columns()
//                 .iter()
//                 .map(|c_arc| {
//                     let c = c_arc.to_vec().get(0).unwrap().clone();
//                     let t = c.column_type();
//                     Column {
//                         table: String::from(c.schema_str().to_owned()),
//                         column: String::from(c.name_str().to_owned()),
//                         coltype: t,
//                         colflags: c.flags(),
//                     }
//                 })
//                 .collect();
//             let rows_result = query_result.collect::<Row>().await;
//             match rows_result {
//                 Ok(rows) => {
//                     let mysql_result = MySQLResult { cols, rows };
//                     Ok(mysql_result)
//                 }
//                 Err(err) => {
//                     error!("get mysql err:{:?}", err);
//                     Err(err)
//                 }
//             }
//             // let rows: Vec<Row> = query_result.flatten().collect();
//
//         }
//         Err(err) => {
//             error!("get mysql err:{:?}", err);
//             err
//         }
//     }
// }


// this is where the proxy server implementation starts

struct Prepared {
    stmt: mysql_async::Statement,
    params: Vec<Column>,
    sql: String,
}

#[derive(Debug)]
pub struct MySQLResult {
    // #[serde(serialize_with = "column_vec_ser")]
    // #[serde(deserialize_with = "column_vec_deser")]
    pub cols: Vec<Column>,
    pub rows: Vec<Row>,
}

pub struct MySQL {
    connection: Conn,
    redis_conn: Connection,
    // NOTE: not *actually* static, but tied to our connection's lifetime.
    prepared: Slab<Prepared>,
    dialect: MySqlDialect,
}

impl MySQL {
    pub fn new(c: mysql_async::Conn, redis_conn: Connection) -> Self {
        MySQL {
            connection: c,
            redis_conn,
            prepared: Slab::new(),
            dialect: MySqlDialect {},
        }
    }
}

#[derive(Debug)]
pub enum VirtDBMySQLError {
    MySQL_ASYNC(mysql_async::Error),
    Io(io::Error),
}

impl From<io::Error> for VirtDBMySQLError {
    fn from(e: io::Error) -> Self {
        VirtDBMySQLError::Io(e)
    }
}

impl From<mysql_async::Error> for VirtDBMySQLError {
    fn from(e: Error) -> Self {
        VirtDBMySQLError::MySQL_ASYNC(e)
    }
}

impl VirtDBMySQLError {
    pub fn to_string(self) -> String {
        return match self {
            VirtDBMySQLError::MySQL_ASYNC(mysql_async_err) => {
                mysql_async_err.to_string()
            }
            VirtDBMySQLError::Io(err) => {
                err.to_string()
            }
        }
    }
}

impl MySQL {
    pub async fn execute_query(&mut self, sql:&str) ->Result<MySQLResult,VirtDBMySQLError>{
        // let r = self.connection.query(query);
        trace!("sql:{}", sql);
        let redis_key = format!("cache:{:?}", sql);
        let ast_opt:Result<Vec<Statement>,ParserError> = Parser::parse_sql(&self.dialect, sql);
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
                            self.redis_conn.get(redis_key.clone()).await;
                        if let Ok(redis_v) = cached_value_result {
                            let mysql_result: MySQLResult = serde_json::from_str(&*redis_v).unwrap();
                            trace!("decoded_v:{:?}",mysql_result);
                            return Ok(mysql_result);
                        }
                    }
                    trace!("ast query body:{:?}", query.body);
                }
            }
        }

        // let query_result_result: Result<QueryResult<TextProtocol>, Error> =
        //     self.connection.query_iter(String::from(sql)).await;

        let mut query_result: QueryResult<TextProtocol> = self.connection.query_iter(String::from(sql)).await?;
        trace!("v:{:?}", query_result);
        // let mysql_result_opt = handle_mysql_result(query_result_result).await;
        trace!("columns:{:?}", query_result.columns());
        let mut cols: Vec<Column> = vec![];
        for arc_col in query_result.columns() {
            cols = arc_col.iter()
                .map(|mut c| {
                    let t = c.column_type();
                    let rc = Column {
                        table: String::from(c.schema_str().to_owned()),
                        column: String::from(c.name_str().to_owned()),
                        coltype: t,
                        colflags: c.flags(),
                    };
                    return rc;
                })
                .collect();
        }

        let rows = query_result.collect::<Row>().await?;
        let mysql_result = MySQLResult { cols, rows };
        // let rows: Vec<Row> = query_result.flatten().collect();
        let json_v = serde_json::to_string(&mysql_result)
            .unwrap_or_default();
        trace!("json_v:{}",json_v);
        let rv: RedisResult<Vec<Vec<u8>>> = self.redis_conn.set_ex(redis_key.clone(), json_v.as_str(), 60).await;
        Ok(mysql_result)
    }
}

#[async_trait::async_trait]
impl<W: AsyncWrite + Send + Unpin> AsyncMysqlShim<W> for MySQL {
    type Error = VirtDBMySQLError;

    fn version(&self) -> &str {
        "virt-db 1.0.0-alpha"
    }

    async fn on_prepare<'a>(&'a mut self, query: &'a str, info: StatementMetaWriter<'a, W>) -> Result<(), Self::Error> {
        let startTime = SystemTime::now();
        let r = match self.connection.prep(query).await {
            Ok(stmt) => {
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
                let stmt = Prepared { stmt, params, sql: String::from(query.clone()) };

                // the statement is tied to the connection, which as far as the compiler is aware
                // we only know lives for as long as the `&mut self` given to this function.
                // however, *we* know that the connection will live at least as long as the
                // prepared statement we insert into `self.prepared` (because there is no way to
                // get the prepared statements out!).
                let stmt = unsafe { mem::transmute(stmt) };

                let id = self.prepared.insert(stmt);
                let stmt = &self.prepared[id];
                info.reply(id as u32, &stmt.params, &columns).await?;
                Ok(())
            }

            Err(e) => {
                Err(e.into())
            }
        };
        let duration = SystemTime::now().duration_since(startTime).unwrap().as_millis();
        histogram!("sql_duration", duration as f64, "sql" => String::from(query.clone()));

        return r;
    }

    async fn on_execute<'a>(&'a mut self, id: u32, params: ParamParser<'a>, results: QueryResultWriter<'a, W>) -> Result<(), Self::Error> {
        // let start_time = SystemTime::now();
        // let r = match self.prepared.get_mut(id as usize) {
        //     None => results.error(ErrorKind::ER_NO, b"no such prepared statement").await?,
        //     Some(&mut Prepared { ref mut stmt, ref mut params, ref mut sql }) => {
        //         // let args = params.into_iter()
        //         //     .map(|p|{
        //         //         p.coltype
        //         //     })
        //         //     .collect();
        //         // let r = self.connection.exec(stmt, params).await;
        //         // let types = params.into_iter()
        //         //     .map(|f| {
        //         //         f.coltype
        //         //     })
        //         //     .collect();
        //         // results.completed(OkResponse::default()).await?;
        //         let err_result = results.error(ErrorKind::ER_NO, b"no such prepared statement").await?;
        //
        //         let duration = SystemTime::now().duration_since(start_time).unwrap().as_millis();
        //         histogram!("sql_duration", duration as f64, "sql" => String::from(sql.clone()));
        //         return err_result;
        //     }
        // }.await;
        let r = results.error(ErrorKind::ER_NO, b"Not Support").await?;
        return Ok(r);
    }

    async fn on_close(&mut self, id: u32) {
        debug!("connection {} closed", id);
        self.prepared.remove(id as usize);
    }

    async fn on_query<'a>(&'a mut self, sql: &'a str, results: QueryResultWriter<'a, W>) -> Result<(), Self::Error> {
        let start_time = SystemTime::now();
        let mysql_result_wrapper = self.execute_query(sql).await;
        return match mysql_result_wrapper {
            Ok(mysql_result) => {
                let mut writer = results.start(&mysql_result.cols).await?;
                for row in mysql_result.rows {
                    trace!("row:{:?}", row);
                    for col in &mysql_result.cols {
                        trace!("col:{:?}", col);
                        let column_value = &row[col.column.as_ref()];
                        trace!("blob column_value:{:?}", column_value);
                        writer.write_col(column_value)?;
                    }
                    writer.end_row().await?;
                }

                let duration = SystemTime::now().duration_since(start_time).unwrap().as_millis();
                histogram!("sql_duration", duration as f64, "sql" => String::from(sql.clone()));
                Ok(writer.finish().await?)
            },
            Err(mut e) => {
                results.error(ErrorKind::ER_YES, e.to_string().as_bytes()).await?;
                Ok(())
            }
        }

    }

    async fn on_init<'a>(&'a mut self, schema: &'a str, writer: InitWriter<'a, W>) -> Result<(), Self::Error> {
        debug!("schema:{}", schema);
        // self.connection.select_db(schema);
        let command_select_db_string = format!("use `{schema}`");
        let command_select_db_str = command_select_db_string.as_str();
        let r: Result<Vec<(u32, String)>, mysql_async::Error> = self.connection.query(command_select_db_str).await;
        return match r {
            Ok(_) => {
                Ok(writer.ok().await?)
            }
            Err(e) => {
                writer.error(ErrorKind::ER_YES, e.to_string().as_bytes()).await?;
                Ok(())
            }
        };
    }
}

impl Drop for MySQL {
    fn drop(&mut self) {
        // drop all the prepared statements *first*.
        self.prepared.clear();
        // *then* we can drop the connection (implicitly done).
    }
}