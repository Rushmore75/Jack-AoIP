use crate::BUFFER_SIZE;
use std::{net::{TcpStream, TcpListener, UdpSocket}, process::exit, io::{Write, Read}};


/**
Convert an array of `f32` into an array of `u8`. 
This is with the purposes of jack in mind. So the input array should equal `BUFFER_SIZE`.
There are, however, no checks on this. It will probably just go out of bounds if that happens,
which will panic the whole program. You should have no problem passing in arrays taken from jack tho.


# How to obtain f32 array:
```
// This code is taken from the impl of Port<AudioIn>

// slice will have length of jack's buffer size.
let slice: &[f32] = unsafe {
    slice::from_raw_parts(
        f.buffer(ps.n_frames()) as *const f32,
        ps.n_frames() as usize,
    )
}
``` 

# New Docs:
Pass the array of `[f32]` into the function along with the `[u8]` array for the
converted `f32`s to be placed in. The `[u8]` array needs to be ***x4*** longer
that the `[f32]` array. (`1 f32 == 4 u8`)

*/
#[inline]
fn f32_to_u8_array(array: &[f32], output: &mut [u8]) {
    for array_index in 0..array.len() {
        // convert the f32 to [u8; 4]
        let bytes = array[array_index].to_be_bytes(); // big endian is network order
        // copy the new array into the buffer
        for byte_index in 0..4 {
            output[array_index*4+byte_index] = bytes[byte_index];
        }   
    }
}

/**
Converts array of `u8` into an array of `f32`, placing this result in
the passed "`output`" array. So long as `u8` array as a length of **x4** longer
than the `f32` array, no problems should arise.
 */
#[inline]
fn u8_to_f32_array(array: &[u8], output: &mut [f32]) {
    for buffer_index in 0..output.len() {
        let mut byte_array:[u8;4]=[0;4];
        // put the four u8 into a sized array
        for i in 0..4 { byte_array[i] = array[buffer_index*4+i]; }
        // convert the byte array into a f32
        let f: f32 = f32::from_be_bytes(byte_array);
        // place into buffer
        output[buffer_index] = f;
    }
}


pub trait NetworkModel {
    /**
    Send the pass `f32` buffer to the underlying socket. Wether this is 
    a tcp or udp connection depends on which type you passed in during
    creation.
     */
    fn send(&mut self, buffer: &[f32]);

    /**
    Receive a buffer of `f32` from the underlying socket. Wether this
    is a tcp or udp connection depends on the type you passed in during
    creation.
     */
    fn receive(&mut self, buffer: &mut [f32]);
}

/**
# TCP vs UDP
Since `tcp` requires you to establish a connection between to address before you
start sending data, it makes it easier to know if you have a connection problem
earlier than with using `udp`. *However* it is ***much*** slower than `udp`. While
writing this on a `AMD FX-8320 (8) @ 3.500GHz` I could only get the buffer size down
to `2048` before getting Xruns on `tcp`. While I got down to `265` using `udp`. At which point
other applications (firefox) started giving Xruns as well. Using `128` started to get too many
Xruns to be considered "good" (but still better than `tcp @ 1024`!)

---

# Example usage
These examples assume you have send and receive address setup, ex:
```
const SEND_ADDR: &str = "127.0.0.1:6001";
const RECV_ADDR: &str = "127.0.0.1:5001";
```
---
## Using a udp connection:
Faster, and probably a better choice for sending audio.
```   
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
```
---
## Using a tcp connection:
Setup is basically the same as udp, this connection however will be (***a lot***) slower.
Unless you have a *very* good reason your should use udp.
```
let receive = thread::spawn(|| {
    let listen: TcpListener = TcpListener::bind(RECV_ADDR).unwrap();
    let recv_socket: AoIP<Tcp> = AoIP(Tcp::Listener(listen));
    start_on_transport(recv_socket, jack::AudioOut::default(), 2);
});

let send = thread::spawn(|| {
    let stream: TcpStream = TcpStream::connect(RECV_ADDR).unwrap();
    let send_socket: AoIP<Tcp> = AoIP(Tcp::Stream(stream));
    start_on_transport(send_socket, jack::AudioIn::default(), 2);
});

send.join().unwrap();
receive.join().unwrap();
```
 */
pub struct AoIP<T>(pub T) where T: NetworkModel + Sized;

pub enum Tcp {
    /** Used for both stream data *to* and *from* a socket. */
    Stream(TcpStream),
    /** It will only be a listener long enough to find a stream to convert into. */
    _Listener(TcpListener)
}

impl NetworkModel for Tcp {

    fn send(&mut self, buffer: &[f32]) {
        // Generate buffer and load f32s (converted into u8) into it
        let mut send_buffer = [0u8; BUFFER_SIZE*4];
        f32_to_u8_array(buffer, &mut send_buffer);
        
        // Get the stream from self
        let mut stream = match &self {
            Tcp::_Listener(_) => {
                println!("Why are you sending on a listening connection?");
                exit(1)
            },
            Tcp::Stream(s) => s,
        };
        // Write out data
        stream.write(&send_buffer).unwrap();
    }

    fn receive(&mut self, buffer: &mut [f32]) {
        // Handle tcp stream
        let mut stream = match &self {
            Tcp::Stream(s) => s,
            Tcp::_Listener(l) => {
                match l.accept() {
                    Ok((stream, addr)) => {
                        // This code block will run once, converting
                        // the listener into a read-able stream
                        println!("Established connection to {}", addr);
                        *self = Tcp::Stream(stream);
                        return; // skip this iteration, we'll get 'em next time
                    },
                    Err(_) => {
                        todo!()
                    },
                }
            },
        };

        let mut recv_buffer = [0u8; BUFFER_SIZE*4];
        //  This is slow, you might need to increase your buffer size if you are getting xruns
        stream.read(&mut recv_buffer).unwrap();

        // convert the read buffer into f32s for the output buffer
        u8_to_f32_array(&recv_buffer, buffer);

        // for buffer_index in 0..buffer.len() {
        //     let mut byte_array:[u8;4]=[0;4];
        //     // put the four u8 into a sized array
        //     for i in 0..4 { byte_array[i] =  recv_buffer[buffer_index*4+i]; }
        //     // convert the byte array into a f32
        //     let f: f32 = f32::from_be_bytes(byte_array);
        //     // place into buffer
        //     buffer[buffer_index] = f;
        // }
    }

}

/**
A UDP socket, to be used for sending audio over ip.
You need to connect the socket before passing it here.

Creating the receiving socket. The only thing that would
change for the sending socket is swapping RECV_ADDR <-> SEND_ADDR
```

let socket = UdpSocket::bind(RECV_ADDR).unwrap();
socket.connect(SEND_ADDR).unwrap();
let aoip = AoIP(Udp(socket));

```
 */
pub struct Udp(pub UdpSocket);

impl NetworkModel for Udp {

    fn send(&mut self, buffer: &[f32]) {

        let mut send_buffer = [0u8; BUFFER_SIZE*4];

        f32_to_u8_array(buffer, &mut send_buffer);
        
        // If this says set a destination address, this can't be recovered from here as
        // you probably didn't set a receiving address either, which we can't change from here.
        self.0.send(&send_buffer).unwrap(); // TODO make this a clean shutdown of Err
    }

    fn receive(&mut self, buffer: &mut [f32]) {

        let mut recv_buffer = [0u8; BUFFER_SIZE*4];

        self.0.recv(&mut recv_buffer).unwrap();

        // slices could work...
        // u8_to_f32_array(&recv_buffer[..BUFFER_SIZE*4], buffer);

        u8_to_f32_array(&recv_buffer, buffer);
    }
}
