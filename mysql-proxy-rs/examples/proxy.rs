//! MySQL Proxy Server
extern crate mysql_common;
extern crate mysql_proxy;

use mysql_proxy::*;

#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate futures;
#[macro_use]
extern crate tokio_core;
extern crate byteorder;

use std::{env, iter};
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;

use futures::stream::Stream;
use futures::Future;
use tokio::runtime::Builder;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use mysql_proxy::packet_writer::PacketWriter;
use mysql_proxy::resultset::QueryResultWriter;
use mysql_proxy_rs::Pipe;

fn main() {
    env_logger::init().unwrap();

    // determine address for the proxy to bind to
    let bind_addr = env::args().nth(1).unwrap_or("127.0.0.1:3307".to_string());
    let bind_addr = bind_addr.parse::<SocketAddr>().unwrap();

    // determine address of the MySQL instance we are proxying for
    let mysql_addr = env::args().nth(2).unwrap_or("127.0.0.1:3306".to_string());
    let mysql_addr = mysql_addr.parse::<SocketAddr>().unwrap();

    // Create the tokio event loop that will drive this server
    let mut l = Core::new().unwrap();

    // Get a reference to the reactor event loop
    let handle = l.handle();

    // Create a TCP listener which will listen for incoming connections
    let socket = TcpListener::bind(&bind_addr, &l.handle()).unwrap();
    println!("Listening on: {}", bind_addr);

    let rt = Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();
    let rt = Arc::new(rt);
    // for each incoming connection
    let done = socket.incoming().for_each(move |(socket, _)| {
        let rt = rt.clone();
        // create a future to serve requests
        let future = TcpStream::connect(&mysql_addr, &handle)
            .and_then(move |mysql| Ok((socket, mysql)))
            .and_then(move |(client, server)| {
                Pipe::new(Rc::new(client), Rc::new(server), DemoHandler { rt })
            });

        // tell the tokio reactor to run the future
        handle.spawn(future.map_err(|err| {
            println!("Failed to spawn future: {:?}", err);
        }));

        // everything is great!
        Ok(())
    });
    l.run(done).unwrap();
}

const PLAIN_OK: &[u8] = b"\x00\x01\x00\x02\x00\x00\x00";

struct DemoHandler {
    rt: Arc<tokio::runtime::Runtime>,
}

impl PacketHandler for DemoHandler {
    fn handle_request(&mut self, p: &Packet, client_reader: &ConnReader, client_writer: &ConnWriter, client_packet_writer: &mut PacketWriter) -> Action {
        // print_packet_chars(&p.bytes);
        match p.packet_type() {
            Ok(PacketType::ComQuery) => {
                // ComQuery packets just contain a SQL string as the payload
                let slice = &p.bytes[5..];

                // convert the slice to a String object
                let sql = String::from_utf8(slice.to_vec()).expect("Invalid UTF-8");

                // log the query
                println!("SQL: {}", sql);

                // dumb example of conditional proxy behavior
                if sql.contains("avocado") {
                    // take over processing of this packet and return an error packet
                    // to the client
                    Action::Error {
                        code: 1064,                            // error code
                        state: [0x31, 0x32, 0x33, 0x34, 0x35], // sql state
                        msg: String::from("Proxy rejecting any avocado-related queries"),
                    }
                } else if sql.contains("select 1") {
                    self.rt.block_on(async {
                        let server_capabilities = CapabilityFlags::CLIENT_PROTOCOL_41
                            | CapabilityFlags::CLIENT_SECURE_CONNECTION
                            | CapabilityFlags::CLIENT_PLUGIN_AUTH
                            | CapabilityFlags::CLIENT_PLUGIN_AUTH_LENENC_CLIENT_DATA
                            | CapabilityFlags::CLIENT_CONNECT_WITH_DB
                            | CapabilityFlags::CLIENT_DEPRECATE_EOF;
                        let w = QueryResultWriter::new(client_packet_writer,false,server_capabilities);
                        w.completed(OkResponse::default()).await
                    }).unwrap();
                    Action::Drop
                } else {
                    // pass the packet to MySQL unmodified
                    Action::Forward
                }
            }
            _ => Action::Forward,
        }
    }

    fn handle_response(&mut self, packet: &Packet) -> Action {
        // forward all responses to the client
        print_packet_chars(&*packet.bytes);
        println!();
        Action::Forward
    }
}

#[allow(dead_code)]
pub fn print_packet_chars(buf: &[u8]) {
    for i in 0..buf.len() {
        print!("{}", buf[i] as char);
    }
}

#[allow(dead_code)]
pub fn print_packet_bytes(buf: &[u8]) {
    print!("[");
    for i in 0..buf.len() {
        if i % 8 == 0 {
            println!("");
        }
        print!("{:#04x} ", buf[i]);
    }
    println!("]");
}
