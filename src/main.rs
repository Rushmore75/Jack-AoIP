use core::slice;
use std::{net::{TcpStream, TcpListener}, thread, io::{Write, Read}, process::exit};

use futures::executor::block_on;
use jack::{Client, Control, ClosureProcessHandler, ProcessScope, PortSpec, jack_sys::{JackPortIsInput, JackPortIsOutput}};

fn main() {
    // TODO when the program starts it generates a handful of Xruns, this is probably
    // due to not stopping cleanly...
    block_on(async_start())
}

/**
buffer size of jack
 */
const BUFFER_SIZE: usize = 2048;
const RECV_ADDR: &str = "127.0.0.1:5001";

async fn async_start() {
    
    let receive = thread::spawn(|| {
        let listen = TcpListener::bind(RECV_ADDR).unwrap();

        let recv_socket = AoIP(Tcp::Listener(listen));
        // start_receive(recv_socket);
        start_on_transport(recv_socket, jack::AudioOut::default());
    });
    
    let send = thread::spawn(|| {
        
        let stream = TcpStream::connect(RECV_ADDR).unwrap();
        let send_socket = AoIP(Tcp::Stream(stream));
        start_on_transport(send_socket, jack::AudioIn::default());
    });
    
    send.join().unwrap();
    receive.join().unwrap();
}

fn start_on_transport<P>(mut socket: AoIP, port_spec: P) where P: 'static + PortSpec + Send + Copy {

    // Get the client data
    let (client, _status) = Client::new(
        "Rust", // TODO name needs to change via P type
        jack::ClientOptions::NO_START_SERVER,
    )
    .unwrap();
    
    // make a large number of ports
    let mut vec = Vec::new();
    for i in 0..1 { 
        let port = client.register_port(
            &format!("{}_{}", i, "in"),
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
                        socket.send(slice);
                        
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
                        socket.receive(mut_slice);
                        
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

pub struct AoIP(Tcp);
enum Tcp {
    /** Used for both stream data *to* and *from* a socket. */
    Stream(TcpStream),
    /** It will only be a listener long enough to find a stream to convert into. */
    Listener(TcpListener)
}

impl AoIP {

    /**
    Send the buffer away
     */
    pub fn send(&mut self, buffer: &[f32]) {
        let mut out_buffer = [0u8; BUFFER_SIZE*4];

        for buf_index in 0..buffer.len() {
            // convert the f32 to [u8; 4]
            let bytes = buffer[buf_index].to_be_bytes(); // be endian is network order

            // copy the new array into the buffer
            for byte_index in 0..4 {
                out_buffer[buf_index*4+byte_index] = bytes[byte_index];
            }   
        }
        
        let mut stream = match &self.0 {
            Tcp::Listener(_) => {
                println!("Why are you sending on a listening connection?");
                exit(1)
            },
            Tcp::Stream(s) => s,
        };
        stream.write(&out_buffer).unwrap();
    }

    /**
    Receive a buffer, data will be placed in the passed buffer
    */
    pub fn receive(&mut self, buffer: &mut [f32]) {
        let mut recv_buffer = [0u8; BUFFER_SIZE*4];
        // Handle tcp stream
        let mut stream = match &self.0 {
            Tcp::Stream(s) => s,
            Tcp::Listener(l) => {
                match l.accept() {
                    Ok((stream, addr)) => {
                        // This code block will run once, converting
                        // the listener into a read-able stream
                        println!("Established connection to {}", addr);
                        self.0 = Tcp::Stream(stream);
                        return; // skip this iteration, we'll get 'em next time
                    },
                    Err(_) => {
                        todo!()
                    },
                }
            },
        };
        //  This is slow, you might need to increase your buffer size if you are getting xruns
        stream.read(&mut recv_buffer).unwrap();

        for buffer_index in 0..buffer.len() {
            let mut byte_array:[u8;4]=[0;4];
            // put the four u8 into a sized array
            for i in 0..4 { byte_array[i] =  recv_buffer[buffer_index*4+i]; }
            // convert the byte array into a f32
            let f: f32 = f32::from_be_bytes(byte_array);
            // place into buffer
            buffer[buffer_index] = f;
        }
    }
}

pub struct Notifications;

impl jack::NotificationHandler for Notifications {
    fn thread_init(&self, _: &jack::Client) {
        println!("JACK: thread init");
    }

    fn shutdown(&mut self, status: jack::ClientStatus, reason: &str) {
        println!(
            "JACK: shutdown with status {:?} because \"{}\"",
            status, reason
        );
    }

    fn freewheel(&mut self, _: &jack::Client, is_enabled: bool) {
        println!(
            "JACK: freewheel mode is {}",
            if is_enabled { "on" } else { "off" }
        );
    }

    fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
        println!("JACK: sample rate changed to {}", srate);
        jack::Control::Continue
    }

    fn client_registration(&mut self, _: &jack::Client, name: &str, is_reg: bool) {
        println!(
            "JACK: {} client with name \"{}\"",
            if is_reg { "registered" } else { "unregistered" },
            name
        );
    }

    fn port_registration(&mut self, _: &jack::Client, port_id: jack::PortId, is_reg: bool) {
        println!(
            "JACK: {} port with id {}",
            if is_reg { "registered" } else { "unregistered" },
            port_id
        );
    }

    fn port_rename(
        &mut self,
        _: &jack::Client,
        port_id: jack::PortId,
        old_name: &str,
        new_name: &str,
    ) -> jack::Control {
        println!(
            "JACK: port with id {} renamed from {} to {}",
            port_id, old_name, new_name
        );
        jack::Control::Continue
    }

    fn ports_connected(
        &mut self,
        _: &jack::Client,
        port_id_a: jack::PortId,
        port_id_b: jack::PortId,
        are_connected: bool,
    ) {
        println!(
            "JACK: ports with id {} and {} are {}",
            port_id_a,
            port_id_b,
            if are_connected {
                "connected"
            } else {
                "disconnected"
            }
        );
    }

    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        println!("JACK: graph reordered");
        jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
        println!("JACK: xrun occurred");
        jack::Control::Continue
    }
}
