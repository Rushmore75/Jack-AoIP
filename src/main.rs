mod notification;
mod aoip;

use core::slice;
use std::{net::{UdpSocket, TcpListener, TcpStream}, thread};

use aoip::{AoIP, NetworkModel, Udp, Tcp};
use jack::{Client, Control, ClosureProcessHandler, ProcessScope, PortSpec, jack_sys::{JackPortIsInput, JackPortIsOutput}};
use notification::Notifications;
/**
buffer size of jack
 */
const BUFFER_SIZE: usize = 1024;
const SEND_ADDR: &str = "127.0.0.1:6001";
const RECV_ADDR: &str = "127.0.0.1:5001";

fn main() {
    // TODO when the program stops it generates a handful of Xruns, this is probably
    // due to not stopping cleanly...
    // TODO when multiple connections are going they are not mapped 0->0 , 1->1, etc... :(
    let receive = thread::spawn(|| {
        let socket = UdpSocket::bind(RECV_ADDR).unwrap();
        socket.connect(SEND_ADDR).unwrap();
        let aoip = AoIP(Udp(socket));
    
        start_on_transport(aoip, jack::AudioOut::default(), 2);
    });
    
    let send = thread::spawn(|| {
        let socket = UdpSocket::bind(SEND_ADDR).unwrap();
        socket.connect(RECV_ADDR).unwrap();
        let aoip = AoIP(Udp(socket));
        
        start_on_transport(aoip, jack::AudioIn::default(), 2);
    });
    
    send.join().unwrap();
    receive.join().unwrap();
}

fn start_on_transport<P, T>(mut socket: AoIP<T>, port_spec: P, connections: u32)
where P: 'static + PortSpec + Send + Copy, T: 'static + NetworkModel + Sized + Send,
{

    // Get the client data
    let (client, _status) = Client::new(
        "Rust", // TODO name needs to change via P type
        jack::ClientOptions::NO_START_SERVER,
    )
    .unwrap(); // TODO handle error where you start this before jack, make it exit cleaner
    
    // make a large number of ports
    let mut vec = Vec::new();
    for i in 0..connections { 
        let port = client.register_port(
            &format!("{}_{}", "port", i),
            port_spec)
            .unwrap(); 
        vec.push(port);
    };
    
    // make a generalized callback for all of them
    let process_callback = 
    move |client: &Client, ps: &ProcessScope| -> Control {
        if client.transport().query_state().expect("Failed to query transport state") == jack::TransportState::Rolling {
            match port_spec.jack_flags().bits() {
                JackPortIsInput => {
                    // AudioIn
                    vec.iter().for_each(|f| {
                        let slice = unsafe {
                            // This code is taken from the impl of Port<AudioIn>
                            slice::from_raw_parts(
                                f.buffer(ps.n_frames()) as *const f32,
                                ps.n_frames() as usize,
                            )
                        };
                        socket.0.send(slice);
                        
                    });
                },
                JackPortIsOutput => {
                    // AudioOut
                    vec.iter_mut().for_each(|f| {
                        let mut_slice = unsafe {
                            slice::from_raw_parts_mut(
                                f.buffer(ps.n_frames()) as *mut f32,
                                ps.n_frames() as usize
                            )
                        };

                        socket.0.receive(mut_slice);
                        
                    });
                },
                _ => {}
            };
        }
        jack::Control::Continue
    };
    
    // Activate
    let process = ClosureProcessHandler::new(process_callback);
    let _active_client = client.activate_async(
        Notifications,
        process)
        .unwrap();

    loop{}
}
