#![feature(io_error_uncategorized)]

mod notification;
mod aoip;

use core::slice;
use std::{net::{UdpSocket, ToSocketAddrs}, thread::{self, JoinHandle}, process::ExitCode, sync::{RwLock, Arc}};

use aoip::{AoIP, NetworkModel, Udp};
use jack::{Client, Control, ClosureProcessHandler, ProcessScope, PortSpec, jack_sys::{JackPortIsInput, JackPortIsOutput}};
use notification::Notifications;
/**
buffer size of jack
 */
const BUFFER_SIZE: usize = 1024;
const LOCAL_ADDR: &str = "192.168.1.199:5000";
const REMOTE_ADDR: &str = "192.168.1.42:8096";

fn main() {
    // TODO when the program stops it generates a handful of Xruns, this is probably
    // due to not stopping cleanly...
    // TODO test mapping of large amount of connections
    // TODO allow for buffer size to be chosen after compile, via lazy static and array slices
    // TODO put in a buffer of 0s when transport stops
    // TODO add transport control / syncing
    let running = false;

    let rwlock = Arc::new(RwLock::new(running));
    
    // let source = start_udp_source(REMOTE_ADDR, LOCAL_ADDR, 2);
    let sink = start_udp_sink(REMOTE_ADDR, LOCAL_ADDR, 2, rwlock.clone());

    // Do control logic for if it should be sending 

    println!("Press ENTER to toggle state");
    loop {
        let stdin = std::io::stdin();
        let mut buf = String::new(); 
        stdin.read_line(&mut buf).unwrap();
        
        // This allows for the read lock to drop so that we can
        // write to the value in the match statement. 
        let save = *rwlock.read().unwrap(); 
        match save {
            true => *rwlock.write().unwrap() = false,
            false => *rwlock.write().unwrap() = true,
        }
        println!("Changed to: {}", *rwlock.read().unwrap());
        
    } 
    // source.join().unwrap();
    // sink.join().unwrap();
}

/**
# Usage
The udp source will create an audio output for you to use.
Local address is your `address:port` and remote address is the `address:port`
from where you are (expecting) to receive audio.
```
// As such:
let source = start_udp_source(SEND_ADDR, RECV_ADDR, 2);
source.join().unwrap();
```
 */
pub fn start_udp_source<A>(remote_addr: A, local_addr: A, connections: u32, running: Arc<RwLock<bool>>) -> JoinHandle<()>
where A: 'static + ToSocketAddrs + Send + Copy + Sync
{
    let receive = thread::spawn(move || {
        let socket = UdpSocket::bind(local_addr).unwrap();
        socket.connect(remote_addr).unwrap();
        let aoip = AoIP(Udp(socket));
        wait_on_signal(aoip, jack::AudioOut::default(), connections, running);
    });

    receive
}

/**
# Usage
The udp sink will collect and send audio to be collected by a source somewhere.
Local address is your `address:port` while remote address is the `address:port`
to where you (are expecting) a source to be to collect the audio.
```
// As such:
let sink = start_udp_sink(RECV_ADDR, SEND_ADDR, 2);
sink.join().unwrap();
```
 */
pub fn start_udp_sink<A>(remote_addr: A, local_addr: A, connections: u32, running: Arc<RwLock<bool>>) -> JoinHandle<()>
where A: 'static + ToSocketAddrs + Send + Copy + Sync
{
    let send = thread::spawn(move || {
        let socket = UdpSocket::bind(local_addr).unwrap();
        socket.connect(remote_addr).unwrap();
        let aoip = AoIP(Udp(socket));
        wait_on_signal(aoip, jack::AudioIn::default(), connections, running);
    });
    
    send
}

fn wait_on_signal<P, T>(mut socket: AoIP<T>, port_spec: P, connections: u32, running: Arc<RwLock<bool>>) -> ExitCode
where P: 'static + PortSpec + Send + Copy, T: 'static + NetworkModel + Sized + Send,
{

    let mut is_input = false;
    let mut is_output = false;

    // check if we are working with an input or output
    let name = match &port_spec.jack_flags().bits() {
        &JackPortIsInput => {
            is_input = true;
            "Sink"
        },
        &JackPortIsOutput => {
            is_output = true;
            "Source"
        },
        _ => {"Other"}
    };

    // Create the client.
    let (client, _status) = match Client::new(
        &format!("Network_{}", name),
        jack::ClientOptions::NO_START_SERVER,
    ) {
        Ok(value) => value,
        Err(e) => match e {
            jack::Error::ClientError(e) => {
                println!("{:?}", e);
                println!("You probably forgot to start jack.");
                println!("Aborting.");
                
                return ExitCode::FAILURE
            },
            _ => {
                panic!("{}", e);
            }
        },
    };

    // Make sure we are using the correct buffer size, otherwise, exit.
    if client.buffer_size() != BUFFER_SIZE as u32 {
        println!("Incorrect BUFFER_SIZE. Found {}, should be: {}", BUFFER_SIZE, client.buffer_size());
        return ExitCode::FAILURE
    }

    // Create ports
    let mut vec = Vec::new();
    for i in 0..connections { 
        let port = client.register_port(
            &format!("{}_{}", "port", i),
            port_spec)
            .unwrap(); 
        vec.push(port);
    };
    
    // Create a generalized callback for all of the ports
    let process_callback = 
    move |_: &Client, ps: &ProcessScope| -> Control {
        match running.read() {
            Ok(val) => {
                if *val {
                    // AudioIn
                    if is_input {
                        // Go thru all the ports.
                        vec.iter().for_each(|f| {
                            // Get audio buffer.
                            let slice = unsafe {
                                // This code is taken from the impl of Port<AudioIn>
                                slice::from_raw_parts(
                                    f.buffer(ps.n_frames()) as *const f32,
                                    ps.n_frames() as usize,
                                )
                            };
                            socket.0.send(slice);
                        });
                    }
                    
                    // AudioOut
                    else if is_output {
                        // Go thru all the ports.
                        vec.iter_mut().for_each(|f| {
                            // Get audio buffer.
                            let mut_slice = unsafe {
                                slice::from_raw_parts_mut(
                                    f.buffer(ps.n_frames()) as *mut f32,
                                    ps.n_frames() as usize
                                )
                            };
                            socket.0.receive(mut_slice);
                        });
                    }
                }
                jack::Control::Continue
            }
            Err(_) => todo!(),
        }
    };
    
    // Activate
    let process = ClosureProcessHandler::new(process_callback);
    let _active_client = client.activate_async(
        Notifications(false),
        process)
        .unwrap();

    loop {}
//    ExitCode::SUCCESS // unreachable
}
