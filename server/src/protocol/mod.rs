//! An extensible MySQL Proxy Server based on tokio-core

use std::borrow::BorrowMut;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind, Read, Write};
use std::net::Shutdown;
use std::rc::Rc;

use byteorder::*;
use chrono::{DateTime, Local};
use futures::{Async, Future, Poll};
use tokio_core::net::TcpStream;

use packet_writer::PacketWriter;
// #[cfg(feature = "tls")]
// use tokio_rustls::rustls::ServerConfig;

pub use crate::myc::constants::{CapabilityFlags, ColumnFlags, ColumnType, StatusFlags};
use crate::sys_metrics::ExecLog;
// #[cfg(feature = "tls")]
// pub use crate::tls::{plain_run_with_options, secure_run_with_options};

pub mod packet_reader;
pub mod packet_writer;


/// Handlers return a variant of this enum to indicate how the proxy should handle the packet.
#[derive(Debug, PartialEq)]
pub enum Action {
    /// drop the packet
    Drop,
    /// forward the packet unmodified
    Forward,
    /// forward a mutated packet
    Mutate(Packet),
    /// respond to the packet without forwarding
    Respond(Vec<Packet>),
    /// respond with an error packet
    Error {
        code: u16,
        state: [u8; 5],
        msg: String,
    },
}

/// Packet handlers need to implement this trait
pub trait PacketHandler {
    fn handle_request(&mut self, p: &Packet,ctx:&mut RefMut<ConnectionContext>) -> Action;
    fn handle_response(&mut self, p: &Packet,ctx:&mut RefMut<ConnectionContext>) -> Action;
    fn handle_response_finish(&mut self, packets: Vec<Packet>,ctx:&mut RefMut<ConnectionContext>);
    fn get_context(&mut self) -> Rc<RefCell<ConnectionContext>>;
}

#[derive(Debug, PartialEq,Eq,Clone)]
pub struct ConnectionContext{
    pub sql:Option<String>,
    pub should_update_cache:bool,
    pub fn_start_time: DateTime<Local>,
    pub mysql_exec_start_time: DateTime<Local>,
    pub redis_duration:i64,
    pub from_cache:bool,
    pub cache_duration:i32,
}

/// A packet is just a wrapper for a Vec<u8>
#[derive(Debug, PartialEq)]
pub struct Packet {
    pub bytes: Vec<u8>,
}

impl Packet {
    pub fn new(bytes:Vec<u8>) -> Self {
        Packet{
            bytes,
        }
    }

    /// Create an error packet
    pub fn error_packet(code: u16, state: [u8; 5], msg: String) -> Self {
        // start building payload
        let mut payload: Vec<u8> = Vec::with_capacity(9 + msg.len());
        payload.push(0xff); // packet type
        payload.write_u16::<LittleEndian>(code).unwrap(); // error code
        payload.extend_from_slice("#".as_bytes()); // sql_state_marker
        payload.extend_from_slice(&state); // SQL STATE
        payload.extend_from_slice(msg.as_bytes());

        // create header with length and sequence id
        let mut header: Vec<u8> = Vec::with_capacity(4 + 9 + msg.len());
        header
            .write_u32::<LittleEndian>(payload.len() as u32)
            .unwrap();
        header.pop(); // we need 3 byte length, so discard last byte
        header.push(1); // sequence_id

        // combine the vectors
        header.extend_from_slice(&payload);

        // now move the vector into the packet
        Packet { bytes: header }
    }

    pub fn sequence_id(&self) -> u8 {
        self.bytes[3]
    }

    /// Determine the type of packet
    pub fn packet_type(&self) -> Result<PacketType, Error> {
        match self.bytes[4] {
            0x00 => Ok(PacketType::ComSleep),
            0x01 => Ok(PacketType::ComQuit),
            0x02 => Ok(PacketType::ComInitDb),
            0x03 => Ok(PacketType::ComQuery),
            0x04 => Ok(PacketType::ComFieldList),
            0x05 => Ok(PacketType::ComCreateDb),
            0x06 => Ok(PacketType::ComDropDb),
            0x07 => Ok(PacketType::ComRefresh),
            0x08 => Ok(PacketType::ComShutdown),
            0x09 => Ok(PacketType::ComStatistics),
            0x0a => Ok(PacketType::ComProcessInfo),
            0x0b => Ok(PacketType::ComConnect),
            0x0c => Ok(PacketType::ComProcessKill),
            0x0d => Ok(PacketType::ComDebug),
            0x0e => Ok(PacketType::ComPing),
            0x0f => Ok(PacketType::ComTime),
            0x10 => Ok(PacketType::ComDelayedInsert),
            0x11 => Ok(PacketType::ComChangeUser),
            0x12 => Ok(PacketType::ComBinlogDump),
            0x13 => Ok(PacketType::ComTableDump),
            0x14 => Ok(PacketType::ComConnectOut),
            0x15 => Ok(PacketType::ComRegisterSlave),
            0x16 => Ok(PacketType::ComStmtPrepare),
            0x17 => Ok(PacketType::ComStmtExecute),
            0x18 => Ok(PacketType::ComStmtSendLongData),
            0x19 => Ok(PacketType::ComStmtClose),
            0x1a => Ok(PacketType::ComStmtReset),
            0x1d => Ok(PacketType::ComDaemon),
            0x1e => Ok(PacketType::ComBinlogDumpGtid),
            0x1f => Ok(PacketType::ComResetConnection),
            _ => Err(Error::new(ErrorKind::Other, "Invalid packet type")),
        }
    }
}

#[derive(Copy, Clone)]
pub enum PacketType {
    ComSleep = 0x00,
    ComQuit = 0x01,
    ComInitDb = 0x02,
    ComQuery = 0x03,
    ComFieldList = 0x04,
    ComCreateDb = 0x05,
    ComDropDb = 0x06,
    ComRefresh = 0x07,
    ComShutdown = 0x08,
    ComStatistics = 0x09,
    ComProcessInfo = 0x0a,
    ComConnect = 0x0b,
    ComProcessKill = 0x0c,
    ComDebug = 0x0d,
    ComPing = 0x0e,
    ComTime = 0x0f,
    ComDelayedInsert = 0x10,
    ComChangeUser = 0x11,
    ComBinlogDump = 0x12,
    ComTableDump = 0x13,
    ComConnectOut = 0x14,
    ComRegisterSlave = 0x15,
    ComStmtPrepare = 0x16,
    ComStmtExecute = 0x17,
    ComStmtSendLongData = 0x18,
    ComStmtClose = 0x19,
    ComStmtReset = 0x1a,
    ComDaemon = 0x1d,
    ComBinlogDumpGtid = 0x1e,
    ComResetConnection = 0x1f,
}

/// Wrapper for TcpStream with some built-in buffering
pub struct ConnReader {
    stream: Rc<TcpStream>,
    packet_buf: Vec<u8>,
    read_buf: Vec<u8>,
}

/// Wrapper for TcpStream with some built-in buffering
pub struct ConnWriter {
    stream: Rc<TcpStream>,
    write_buf: Vec<u8>,
}

impl ConnReader {
    fn new(stream: Rc<TcpStream>) -> Self {
        ConnReader {
            stream: stream,
            packet_buf: Vec::with_capacity(4096),
            read_buf: vec![0_u8; 4096],
        }
    }

    /// Read from the socket until the status is NotReady
    fn read(&mut self) -> Poll<(), io::Error> {
        // debug!("read()");
        loop {
            match self.stream.poll_read() {
                Async::Ready(_) => {
                    let n = try_nb!((&*self.stream).read(&mut self.read_buf[..]));
                    if n == 0 {
                        return Err(Error::new(ErrorKind::Other, "connection closed"));
                    }
                    self.packet_buf.extend_from_slice(&self.read_buf[0..n]);
                }
                _ => return Ok(Async::NotReady),
            }
        }
    }

    fn next(&mut self) -> Option<Packet> {
        // debug!("next()");
        // do we have a header
        if self.packet_buf.len() > 3 {
            let l = parse_packet_length(&self.packet_buf);
            // do we have the whole packet?
            let s = 4 + l;
            if self.packet_buf.len() >= s {
                let p = Packet {
                    bytes: self.packet_buf.drain(0..s).collect(),
                };
                Some(p)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl ConnWriter {
    fn new(stream: Rc<TcpStream>) -> Self {
        ConnWriter {
            stream: stream,
            write_buf: Vec::with_capacity(4096),
        }
    }

    /// Write a packet to the write buffer
    fn push(&mut self, p: &Packet) {
        //        debug!("push() capacity: {} position: {} packet_size: {}",
        //               self.write_buf.capacity(), self.write_pos, p.bytes.len());

        self.write_buf.extend_from_slice(&p.bytes);
        // debug!("end push()");
    }

    /// Writes the contents of the write buffer to the socket
    fn write(&mut self) -> Poll<(), io::Error> {
        // debug!("write()");
        while self.write_buf.len() > 0 {
            match self.stream.poll_write() {
                Async::Ready(_) => {
                    let s = (&*self.stream).write(&self.write_buf[..])?;
                    let _: Vec<u8> = self.write_buf.drain(0..s).collect();
                }
                _ => return Ok(Async::NotReady),
            }
        }
        return Ok(Async::Ready(()));
    }
}

pub struct Pipe<H: PacketHandler + 'static> {
    pub client_reader: ConnReader,
    pub client_writer: ConnWriter,
    pub server_reader: ConnReader,
    pub server_writer: ConnWriter,
    // pub proxy_client_reader: PacketReader<Rc<TcpStream>>,
    pub client_packet_writer: PacketWriter,
    handler: H,
}

impl<H> Pipe<H>
    where
        H: PacketHandler + 'static,
{
    pub fn new(client: Rc<TcpStream>, server: Rc<TcpStream>, handler: H) -> Pipe<H> {
        Pipe {
            client_reader: ConnReader::new(client.clone()),
            client_writer: ConnWriter::new(client.clone()),
            server_reader: ConnReader::new(server.clone()),
            server_writer: ConnWriter::new(server),
            // proxy_client_reader: PacketReader::new(client.clone()),
            client_packet_writer: PacketWriter::new(client),
            handler,
        }
    }
}

impl<H> Future for Pipe<H>
    where
        H: PacketHandler + 'static,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<(), Error> {
        let mut ctx = self.handler.get_context();
        let mut ctx = (&*ctx).borrow_mut();
        loop {
            let mut client_read = self.client_reader.read();

            // process buffered requests
            while let Some(request) = self.client_reader.next() {
                let slice = &request.bytes[5..];
                let handle_request_result = self
                    .handler
                    .handle_request(&request, &mut ctx);
                match handle_request_result{
                    Action::Drop => {}
                    Action::Forward => self.server_writer.push(&request),
                    Action::Mutate(ref p2) => self.server_writer.push(p2),
                    Action::Respond(ref v) => {
                        for p in v {
                            self.client_writer.push(&p);
                        }
                    }
                    Action::Error { code, state, msg } => {
                        let error_packet = Packet::error_packet(code, state, msg);
                        self.client_writer.push(&error_packet);
                    }
                };

                if let Ok(sql) = String::from_utf8(slice.to_vec()){
                    ctx.sql = Some(sql.clone());
                    // info!("push sql:{:?}",sql);
                };
            }

            // try reading from server
            let server_read = self.server_reader.read();
            // process buffered responses
            let mut packets = vec![];
            let mut write_finish = false;
            while let Some(response) = self.server_reader.next() {
                // info!("ctx:{:?}",ctx);
                packets.push(Packet::new(response.bytes.clone()));
                match self.handler.handle_response(&response, &mut ctx) {
                    Action::Drop => {}
                    Action::Forward => self.client_writer.push(&response),
                    Action::Mutate(ref p2) => self.client_writer.push(p2),
                    Action::Respond(ref v) => {
                        for p in v {
                            self.server_writer.push(&p);
                        }
                    }
                    Action::Error { code, state, msg } => {
                        let error_packet = Packet::error_packet(code, state, msg);
                        self.client_writer.push(&error_packet);
                    }
                };
                write_finish = true;
            }
            if write_finish {
                // let mut chars = vec![];
                // for x in packets {
                //     for v in x.bytes {
                //         chars.push(v as char);
                //     }
                // }
                // let s:String = chars.iter()
                //     .collect();
                // info!("sql:{:?},packets:{:?}",ctx.sql,s);
                self.handler.handle_response_finish(packets, &mut ctx);
            }

            // perform all of the writes at the end, since the request handlers may have
            // queued packets in either, or both directions

            // try writing to client
            let client_write = self.client_writer.write();

            // if the server connection has closed, close the client connection too
            match &server_read {
                &Err(ref e) => {
                    debug!("Server closed connection: {}", e);
                    match self.client_writer.stream.shutdown(Shutdown::Write) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }
                _ => {}
            }

            // try writing to server
            let server_write = self.server_writer.write();

            // if the client connection has closed, close the server connection too
            match &client_read {
                &Err(ref e) => {
                    debug!("Client closed connection: {}", e);
                    match self.server_writer.stream.shutdown(Shutdown::Write) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }
                _ => {}
            }

            try_ready!(client_read);
            try_ready!(client_write);
            try_ready!(server_read);
            try_ready!(server_write);
        }
    }
}

/// Parse the MySQL packet length (3 byte little-endian)
fn parse_packet_length(header: &[u8]) -> usize {
    (((header[2] as u32) << 16) | ((header[1] as u32) << 8) | header[0] as u32) as usize
}

// max payload size 2^(24-1)
pub const U24_MAX: usize = 16_777_215;
