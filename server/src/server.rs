// #[macro_use]
// extern crate log;
//
// use clap::{App, Arg};
// use clap::{AppSettings, Parser};
// use env_logger::{Builder, Target};
//
// // use crate::server::start;
//
// mod server;
//
// #[derive(Parser)]
// #[clap(author, version, about, long_about = None)]
// #[clap(allow_negative_numbers = true)]
// #[clap(global_setting(AppSettings::DeriveDisplayOrder))]
// struct Cli {
//     #[clap(short, long, action = clap::ArgAction::Count)]
//     dev: u8,
// }
//
// fn init_logger(dev: u8) {
//     let mut builder = Builder::from_default_env();
//     builder.target(Target::Stdout);
//     builder.init();
//     // if dev == 1 {
//     //     let mut builder = Builder::from_default_env();
//     //     builder.target(Target::Stdout);
//     //     builder.init();
//     // } else {
//     //
//     // };
// }
//
// fn main() {
//     let cli = Cli::parse();
//     init_logger(cli.dev);
//     start();
// }

//! This example implements a (simple) proxy that allows connecting to PostgreSQL as though it were
//! a MySQL database. To try this out, start a PostgreSQL database at localhost:5432, and then run
//! this example. Notice that `main` does *not* use PostgreSQL bindings, just MySQL ones!

extern crate msql_srv;
extern crate slab;

use std::borrow::Borrow;
use std::{io, net, thread};

use log::{debug, info};
use msql_srv::{
    Column, ColumnFlags, ErrorKind, InitWriter, MysqlIntermediary, MysqlShim, ParamParser,
    QueryResultWriter, StatementMetaWriter,
};
use mysql::consts::ColumnType;
use mysql::prelude::Queryable;
use mysql::{from_row_opt, Conn, QueryResult, Text};
use slab::Slab;

pub fn start() {
    let mut threads = Vec::new();
    let listener = net::TcpListener::bind("127.0.0.1:3307").unwrap();

    while let Ok((s, _)) = listener.accept() {
        let v = thread::spawn(move || {
            let url = "mysql://root:root@127.0.0.1:3306";
            let conn_result = Conn::new(url);
            let conn = conn_result.unwrap();
            let mysql_result = MysqlIntermediary::run_on_tcp(MySQL::new(conn), s);
            match mysql_result {
                Ok(v) => {
                    trace!("result:{:?}", v);
                }
                Err(error) => {
                    error!("{:?}", error);
                }
            }
        });
        debug!("New connection established");
        threads.push(v);
    }

    for t in threads {
        t.join().unwrap();
    }
}

// this is where the proxy server implementation starts

struct Prepared {
    stmt: mysql::Statement,
    params: Vec<Column>,
}

struct MySQL {
    connection: Conn,
    // NOTE: not *actually* static, but tied to our connection's lifetime.
    prepared: Slab<Prepared>,
}

impl MySQL {
    fn new(c: mysql::Conn) -> Self {
        MySQL {
            connection: c,
            prepared: Slab::new(),
        }
    }
}

#[derive(Debug)]
enum Error {
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
                        let rt = t2t(t.column_type());
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

    fn on_query(&mut self, query: &str, results: QueryResultWriter<W>) -> Result<(), Self::Error> {
        // let r = self.connection.query(query);
        info!("query:{}", query);
        let mut query_text_result = self.connection.query_iter(String::from(query));
        match query_text_result {
            Ok(query_text) => {
                info!("v:{:?}", query_text);
                if let Some(rr) = query_text.into_iter().next() {
                    if let Some(v) = rr.iter().next() {
                        let cols: Vec<_> = v
                            .columns()
                            .into_iter()
                            .map(|c| {
                                let t = c.column_type();
                                Column {
                                    table: String::from(c.schema_str().to_owned()),
                                    column: String::from(c.name_str().to_owned()),
                                    coltype: t,
                                    colflags: ColumnFlags::empty(),
                                }
                            })
                            .collect();
                        let mut writer = results.start(&cols)?;
                        for row in rr {
                            for (c, col) in cols.iter().enumerate() {
                                debug!("col:{:?}", col);
                                match col.coltype {
                                    ColumnType::MYSQL_TYPE_SHORT => {
                                        let v: Option<i16> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ColumnType::MYSQL_TYPE_LONG => {
                                        let v: Option<i32> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ColumnType::MYSQL_TYPE_LONGLONG => {
                                        let v: Option<i64> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ColumnType::MYSQL_TYPE_FLOAT => {
                                        let v: Option<f32> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ColumnType::MYSQL_TYPE_DOUBLE => {
                                        let v: Option<f64> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ColumnType::MYSQL_TYPE_STRING => {
                                        let v: Option<String> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ColumnType::MYSQL_TYPE_VAR_STRING => {
                                        let opt: Option<Result<String, mysql::FromValueError>> =
                                            row.get_opt(c);
                                        if let Some(option_v) = opt {
                                            match option_v {
                                                Ok(real_v) => {
                                                    // debug!("col_v:{:?}",v);
                                                    if !writer.write_col(real_v).is_ok() {
                                                        return Ok(writer.finish_error(
                                                            ErrorKind::ER_NO,
                                                            b"Not Impl",
                                                        )?);
                                                    }
                                                }
                                                Err(err) => {
                                                    return Ok(writer.finish_error(
                                                        ErrorKind::ER_NO,
                                                        b"Not Impl",
                                                    )?);
                                                }
                                            }
                                        }
                                    }
                                    ColumnType::MYSQL_TYPE_BLOB => {
                                        let v: Option<String> = row.get(c);
                                        writer.write_col(v)?
                                    }
                                    ct => {
                                        let v: Option<String> = row.get(c);
                                        if !writer.write_col(v).is_ok() {
                                            return Ok(writer
                                                .finish_error(ErrorKind::ER_NO, b"Not Impl")?);
                                        }
                                    }
                                }
                            }
                        }

                        return Ok(writer.finish()?);
                    }
                }
            }
            Err(err) => {
                return Ok(results.error(ErrorKind::ER_NO, err.to_string().as_bytes())?);
            }
        }

        Ok(results.error(ErrorKind::ER_NO, b"Not Impl")?)
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

fn t2t(a: ColumnType) -> msql_srv::ColumnType {
    match a {
        ColumnType::MYSQL_TYPE_DECIMAL => msql_srv::ColumnType::MYSQL_TYPE_DECIMAL,
        ColumnType::MYSQL_TYPE_TINY => msql_srv::ColumnType::MYSQL_TYPE_TINY,
        ColumnType::MYSQL_TYPE_SHORT => msql_srv::ColumnType::MYSQL_TYPE_SHORT,
        ColumnType::MYSQL_TYPE_LONG => msql_srv::ColumnType::MYSQL_TYPE_LONG,
        ColumnType::MYSQL_TYPE_FLOAT => msql_srv::ColumnType::MYSQL_TYPE_FLOAT,
        ColumnType::MYSQL_TYPE_DOUBLE => msql_srv::ColumnType::MYSQL_TYPE_DOUBLE,
        ColumnType::MYSQL_TYPE_NULL => msql_srv::ColumnType::MYSQL_TYPE_NULL,
        ColumnType::MYSQL_TYPE_TIMESTAMP => msql_srv::ColumnType::MYSQL_TYPE_TIMESTAMP,
        ColumnType::MYSQL_TYPE_LONGLONG => msql_srv::ColumnType::MYSQL_TYPE_LONGLONG,
        ColumnType::MYSQL_TYPE_INT24 => msql_srv::ColumnType::MYSQL_TYPE_INT24,
        ColumnType::MYSQL_TYPE_DATE => msql_srv::ColumnType::MYSQL_TYPE_DATE,
        ColumnType::MYSQL_TYPE_TIME => msql_srv::ColumnType::MYSQL_TYPE_TIME,
        ColumnType::MYSQL_TYPE_DATETIME => msql_srv::ColumnType::MYSQL_TYPE_DATETIME,
        ColumnType::MYSQL_TYPE_YEAR => msql_srv::ColumnType::MYSQL_TYPE_YEAR,
        ColumnType::MYSQL_TYPE_NEWDATE => msql_srv::ColumnType::MYSQL_TYPE_NEWDATE,
        ColumnType::MYSQL_TYPE_VARCHAR => msql_srv::ColumnType::MYSQL_TYPE_VARCHAR,
        ColumnType::MYSQL_TYPE_BIT => msql_srv::ColumnType::MYSQL_TYPE_BIT,
        ColumnType::MYSQL_TYPE_TIMESTAMP2 => msql_srv::ColumnType::MYSQL_TYPE_TIMESTAMP2,
        ColumnType::MYSQL_TYPE_DATETIME2 => msql_srv::ColumnType::MYSQL_TYPE_DATETIME2,
        ColumnType::MYSQL_TYPE_TIME2 => msql_srv::ColumnType::MYSQL_TYPE_TIME2,
        ColumnType::MYSQL_TYPE_JSON => msql_srv::ColumnType::MYSQL_TYPE_JSON,
        ColumnType::MYSQL_TYPE_NEWDECIMAL => msql_srv::ColumnType::MYSQL_TYPE_NEWDECIMAL,
        ColumnType::MYSQL_TYPE_ENUM => msql_srv::ColumnType::MYSQL_TYPE_ENUM,
        ColumnType::MYSQL_TYPE_SET => msql_srv::ColumnType::MYSQL_TYPE_SET,
        ColumnType::MYSQL_TYPE_TINY_BLOB => msql_srv::ColumnType::MYSQL_TYPE_TINY_BLOB,
        ColumnType::MYSQL_TYPE_MEDIUM_BLOB => msql_srv::ColumnType::MYSQL_TYPE_MEDIUM_BLOB,
        ColumnType::MYSQL_TYPE_LONG_BLOB => msql_srv::ColumnType::MYSQL_TYPE_LONG_BLOB,
        ColumnType::MYSQL_TYPE_BLOB => msql_srv::ColumnType::MYSQL_TYPE_BLOB,
        ColumnType::MYSQL_TYPE_VAR_STRING => msql_srv::ColumnType::MYSQL_TYPE_VAR_STRING,
        ColumnType::MYSQL_TYPE_STRING => msql_srv::ColumnType::MYSQL_TYPE_STRING,
        ColumnType::MYSQL_TYPE_GEOMETRY => msql_srv::ColumnType::MYSQL_TYPE_GEOMETRY,
        ColumnType::MYSQL_TYPE_TYPED_ARRAY => msql_srv::ColumnType::MYSQL_TYPE_TYPED_ARRAY,
        ColumnType::MYSQL_TYPE_UNKNOWN => msql_srv::ColumnType::MYSQL_TYPE_UNKNOWN,
    }
}
