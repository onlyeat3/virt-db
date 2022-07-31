use std::{net, thread};
use mysql::Conn;
use msql_srv::MysqlIntermediary;
use log::{trace,error,debug,info};
use crate::mysql_protocol::MySQL;

pub fn start() {
    let mut threads = Vec::new();
    let listener = net::TcpListener::bind("127.0.0.1:3307").unwrap();

    while let Ok((s, _)) = listener.accept() {
        let v = thread::spawn(move || {
            //TODO pool
            let url = "mysql://root:root@127.0.0.1:3306";
            let conn_result = Conn::new(url);
            let conn = conn_result.unwrap();
            let client = redis::Client::open("redis://127.0.0.1/").unwrap();
            let redis_conn = client.get_connection().unwrap();

            let mysql_result = MysqlIntermediary::run_on_tcp(MySQL::new(conn, redis_conn), s);
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
