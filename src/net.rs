//! Minimal LAN / VPN multiplayer.
//!
//! ScrapForge has no central servers: any player can turn their game into a
//! host (`scrapforge --host`) and everybody else connects straight to it over a
//! LAN, Radmin VPN, Hamachi, ZeroTier or Tailscale:
//!
//! ```text
//! host    : scrapforge --host 25000
//! friend  : scrapforge --join 26.14.55.2:25000
//! ```
//!
//! Transport: TCP with 4-byte length-prefixed JSON frames (small code, no extra
//! dependencies, works through any VPN that forwards TCP).
//!
//! The **host is authoritative for the block grid**: clients send their edits to
//! the host, the host applies them and rebroadcasts the result. Player
//! transforms are replicated peer to peer at 15 Hz. Dynamic contraptions are
//! still simulated locally on every machine (see "Честно об ограничениях").

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::BlockWorld;

pub const DEFAULT_PORT: u16 = 25_000;

/// How many blocks travel in one snapshot frame.
const SNAPSHOT_CHUNK: usize = 400;
/// Transform replication rate.
const MOVE_HZ: f32 = 15.0;
/// Seconds without any packet before a peer is dropped.
const PEER_TIMEOUT: f32 = 15.0;
/// Largest frame we are willing to buffer (4 MB).
const MAX_FRAME: usize = 4_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Offline,
    Host,
    Client,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Offline => "offline",
            Role::Host => "host",
            Role::Client => "client",
        }
    }
}

/// One block as it travels over the wire.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct NetBlock {
    pub cell: [i32; 3],
    pub part: u16,
    pub rot: u8,
    pub color: u8,
    pub state: u8,
}

/// Everything the protocol can say.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Msg {
    /// Client -> host right after connecting.
    Hello { name: String },
    /// Host -> client: your id, the world seed and the host name.
    Welcome { you: u32, seed: u32, name: String },
    /// Host -> client: the grid, split into chunks. `done` marks the last one.
    Snapshot {
        blocks: Vec<NetBlock>,
        links: Vec<([i32; 3], [i32; 3])>,
        done: bool,
    },
    PeerJoin { id: u32, name: String },
    PeerLeave { id: u32 },
    /// Transform of a player.
    Move {
        id: u32,
        pos: [f32; 3],
        yaw: f32,
        pitch: f32,
    },
    /// A single block placed or removed.
    Edit {
        place: bool,
        cell: [i32; 3],
        part: u16,
        rot: u8,
        color: u8,
    },
    /// A short chat line (also used for system notices).
    Say { id: u32, text: String },
    Ping { t: f32 },
    Pong { t: f32 },
}

/// A buffered TCP connection (one frame = 4 byte big endian length + JSON).
struct Conn {
    stream: TcpStream,
    id: u32,
    buf: Vec<u8>,
}

impl Conn {
    fn new(stream: TcpStream, id: u32) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            id,
            buf: Vec::new(),
        })
    }

    /// Reads everything currently available. Returns the messages plus whether
    /// the connection is dead.
    fn pump(&mut self) -> (Vec<Msg>, bool) {
        let mut out = Vec::new();
        let mut tmp = [0u8; 8192];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => return (out, true),
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => return (out, true),
            }
        }
        loop {
            if self.buf.len() < 4 {
                break;
            }
            let len = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
            if len > MAX_FRAME || self.buf.len() < 4 + len {
                break;
            }
            let payload: Vec<u8> = self.buf.drain(4..4 + len).collect();
            if let Ok(msg) = serde_json::from_slice::<Msg>(&payload) {
                out.push(msg);
            }
        }
        (out, false)
    }

    fn send(&mut self, msg: &Msg) -> bool {
        let body = match serde_json::to_vec(msg) {
            Ok(body) => body,
            Err(_) => return false,
        };
        if body.len() > MAX_FRAME {
            return false;
        }
        let mut data = Vec::with_capacity(4 + body.len());
        data.extend_from_slice(&(body.len() as u32).to_be_bytes());
        data.extend_from_slice(&body);
        match self.stream.write_all(&data) {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => false,
            Err(_) => false,
        }
    }
}

/// Another player as seen over the network.
pub struct Peer {
    pub id: u32,
    pub name: String,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub target: Vec3,
    pub target_yaw: f32,
    pub entity: Option<Entity>,
    pub last_seen: f32,
}

#[derive(Resource)]
pub struct Net {
    pub role: Role,
    pub name: String,
    pub self_id: u32,
    pub seed: u32,
    pub listener: Option<TcpListener>,
    /// Host: one connection per client. Client: exactly one (to the host).
    conns: Vec<Conn>,
    pub peers: HashMap<u32, Peer>,
    next_id: u32,
    /// Messages received since the last pump, tagged with the sender id.
    pub inbox: Vec<(u32, Msg)>,
    pub status: String,
    pub latency_ms: f32,
    pub peer_count: usize,
    send_timer: f32,
    ping_timer: f32,
    ping_sent: Option<f32>,
    pub clock: f32,
}

impl Default for Net {
    fn default() -> Self {
        Self::offline("Player".to_string(), 0)
    }
}

impl Net {
    pub fn offline(name: String, seed: u32) -> Self {
        Self {
            role: Role::Offline,
            name,
            self_id: 1,
            seed,
            listener: None,
            conns: Vec::new(),
            peers: HashMap::new(),
            next_id: 2,
            inbox: Vec::new(),
            status: "offline".to_string(),
            latency_ms: 0.0,
            peer_count: 0,
            send_timer: 0.0,
            ping_timer: 0.0,
            ping_sent: None,
            clock: 0.0,
        }
    }

    /// Starts a host: listens on `port` and serves the current world.
    pub fn host(port: u16, name: String, seed: u32) -> io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        listener.set_nonblocking(true)?;
        let mut net = Self::offline(name, seed);
        net.role = Role::Host;
        net.self_id = 1;
        net.listener = Some(listener);
        net.status = format!("хост на порту {} — жду игроков", port);
        Ok(net)
    }

    /// Connects to a host. `addr` is `ip:port` (the port may be omitted).
    pub fn join(addr: &str, name: String, seed: u32) -> io::Result<Self> {
        let addr = if addr.contains(':') {
            addr.to_string()
        } else {
            format!("{}:{}", addr, DEFAULT_PORT)
        };
        let stream = TcpStream::connect(&addr)?;
        stream.set_nodelay(true)?;
        // The connect() above is blocking, so make the socket async afterwards.
        let mut conn = Conn::new(stream, 1)?;
        conn.send(&Msg::Hello { name: name.clone() });
        let mut net = Self::offline(name, seed);
        net.role = Role::Client;
        net.self_id = 0; // the host tells us who we are
        net.conns.push(conn);
        net.status = format!("подключаюсь к {}", addr);
        Ok(net)
    }

    pub fn is_connected(&self) -> bool {
        self.role != Role::Offline
    }

    /// Accepts pending connections (host only).
    fn accept_new(&mut self) {
        let Some(listener) = self.listener.as_ref() else {
            return;
        };
        loop {
            match listener.accept() {
                Ok((stream, addr)) => {
                    let id = self.next_id;
                    self.next_id += 1;
                    if let Ok(conn) = Conn::new(stream, id) {
                        self.conns.push(conn);
                        self.status = format!("подключился {}", addr);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }

    /// Reads the network and fills `inbox`. Call once per frame.
    pub fn pump(&mut self, dt: f32) {
        self.clock += dt;
        self.inbox.clear();
        if self.role == Role::Offline {
            return;
        }
        self.accept_new();

        let now = self.clock;
        let mut dead: Vec<u32> = Vec::new();
        for conn in self.conns.iter_mut() {
            let (msgs, closed) = conn.pump();
            for msg in msgs {
                self.inbox.push((conn.id, msg));
            }
            if closed {
                dead.push(conn.id);
            }
            if let Some(peer) = self.peers.get_mut(&conn.id) {
                peer.last_seen = now;
            }
        }
        for id in dead {
            self.drop_peer(id);
        }

        // Time out silent peers.
        let stale: Vec<u32> = self
            .peers
            .keys()
            .copied()
            .filter(|id| now - self.peers[id].last_seen > PEER_TIMEOUT)
            .collect();
        for id in stale {
            self.drop_peer(id);
        }

        self.peer_count = self.peers.len();

        // Latency probe once per second.
        self.ping_timer += dt;
        if self.ping_timer >= 1.0 {
            self.ping_timer = 0.0;
            if self.role == Role::Client && self.ping_sent.is_none() {
                if self.send(&Msg::Ping { t: self.clock }) {
                    self.ping_sent = Some(self.clock);
                }
            }
        }
    }

    pub fn on_pong(&mut self, t: f32) {
        if let Some(sent) = self.ping_sent.take() {
            self.latency_ms = ((self.clock - sent) * 1000.0).max(0.0);
        }
        let _ = t;
    }

    fn drop_peer(&mut self, id: u32) {
        self.peers.remove(&id);
        self.conns.retain(|c| c.id != id);
        self.broadcast(&Msg::PeerLeave { id }, None);
        self.status = format!("игрок {} вышел", id);
    }

    /// Sends a message to the host (client) or to every client (host).
    pub fn send(&mut self, msg: &Msg) -> bool {
        let mut ok = true;
        for conn in self.conns.iter_mut() {
            if !conn.send(msg) {
                ok = false;
            }
        }
        ok
    }

    pub fn send_to(&mut self, id: u32, msg: &Msg) -> bool {
        for conn in self.conns.iter_mut() {
            if conn.id == id {
                return conn.send(msg);
            }
        }
        false
    }

    pub fn broadcast(&mut self, msg: &Msg, except: Option<u32>) -> bool {
        let mut ok = true;
        for conn in self.conns.iter_mut() {
            if except == Some(conn.id) {
                continue;
            }
            if !conn.send(msg) {
                ok = false;
            }
        }
        ok
    }

    /// True when it is time to replicate the local transform.
    pub fn tick_move(&mut self, dt: f32) -> bool {
        self.send_timer += dt;
        if self.send_timer >= 1.0 / MOVE_HZ {
            self.send_timer = 0.0;
            return true;
        }
        false
    }

    /// Builds the snapshot frames for the whole static grid.
    pub fn snapshot_messages(world: &BlockWorld) -> Vec<Msg> {
        let mut frames = Vec::new();
        let mut chunk: Vec<NetBlock> = Vec::with_capacity(SNAPSHOT_CHUNK);
        for (cell, block) in world.blocks.iter() {
            chunk.push(NetBlock {
                cell: [cell.x, cell.y, cell.z],
                part: block.part,
                rot: block.rot,
                color: block.color,
                state: block.state,
            });
            if chunk.len() >= SNAPSHOT_CHUNK {
                frames.push(Msg::Snapshot {
                    blocks: std::mem::take(&mut chunk),
                    links: Vec::new(),
                    done: false,
                });
            }
        }
        let links: Vec<([i32; 3], [i32; 3])> = world
            .links
            .iter()
            .map(|(a, b)| ([a.x, a.y, a.z], [b.x, b.y, b.z]))
            .collect();
        frames.push(Msg::Snapshot {
            blocks: chunk,
            links,
            done: true,
        });
        frames
    }

    /// Registers a peer (or refreshes it) from a Move/PeerJoin packet.
    pub fn touch_peer(&mut self, id: u32, name: Option<String>, pos: Vec3, yaw: f32, pitch: f32) {
        let now = self.clock;
        let entry = self.peers.entry(id).or_insert(Peer {
            id,
            name: name.clone().unwrap_or_else(|| format!("P{}", id)),
            pos,
            yaw,
            pitch,
            target: pos,
            target_yaw: yaw,
            entity: None,
            last_seen: now,
        });
        entry.target = pos;
        entry.target_yaw = yaw;
        entry.pitch = pitch;
        entry.last_seen = now;
        if let Some(name) = name {
            entry.name = name;
        }
    }

    /// One line for the HUD.
    pub fn hud_line(&self) -> String {
        match self.role {
            Role::Offline => "одиночная игра".to_string(),
            Role::Host => format!("хост · игроков: {} · {}", self.peers.len(), self.name),
            Role::Client => format!(
                "клиент · id {} · игроков: {} · пинг {:.0} мс",
                self.self_id,
                self.peers.len() + 1,
                self.latency_ms
            ),
        }
    }
}

/// Parses `--host [port]`, `--join addr[:port]` and `--name Player` from argv.
pub struct NetOptions {
    pub host: Option<u16>,
    pub join: Option<String>,
    pub name: String,
}

impl NetOptions {
    pub fn parse() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut host = None;
        let mut join = None;
        let mut name = std::env::var("SCRAPFORGE_NAME")
            .unwrap_or_else(|_| "Игрок".to_string());
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--host" => {
                    host = Some(
                        args.get(i + 1)
                            .and_then(|p| p.parse::<u16>().ok())
                            .unwrap_or(DEFAULT_PORT),
                    );
                    if args.get(i + 1).map(|p| p.parse::<u16>().is_ok()) == Some(true) {
                        i += 1;
                    }
                }
                "--join" => {
                    if let Some(addr) = args.get(i + 1).cloned() {
                        if !addr.starts_with('-') {
                            join = Some(addr);
                            i += 1;
                        }
                    }
                }
                "--name" => {
                    if let Some(n) = args.get(i + 1).cloned() {
                        name = n;
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Self { host, join, name }
    }
}
