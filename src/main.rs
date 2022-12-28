#![feature(io_error_uncategorized)]

mod notification;
mod aoip;

use core::slice;
use std::{net::{UdpSocket}, thread, process::ExitCode};

use aoip::{AoIP, NetworkModel, Udp};
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

fn start_on_transport<P, T>(mut socket: AoIP<T>, port_spec: P, connections: u32) -> ExitCode
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
    move |client: &Client, ps: &ProcessScope| -> Control {
        if client.transport().query_state().expect("Failed to query transport state") == jack::TransportState::Rolling {
            // changed to if statement from match, I would have to imagine 1 bool check
            // is faster than a match check, don't know if this is true.
            // but it gets run *a lot* so it can't hurt.
            
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
        };
        jack::Control::Continue
    };
    
    // Activate
    let process = ClosureProcessHandler::new(process_callback);
    let _active_client = client.activate_async(
        Notifications(false),
        process)
        .unwrap();

    loop{}
    // ExitCode::SUCCESS // unreachable
}
