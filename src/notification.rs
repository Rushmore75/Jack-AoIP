/**
Pass a boolean of wether or not you want the notifications to print out
what is happening. Setting: `true` prints.
 */
pub struct Notifications(pub bool);

impl jack::NotificationHandler for Notifications {
    fn thread_init(&self, _: &jack::Client) {
        if self.0 {
            println!("JACK: thread init");
        }
    }

    fn shutdown(&mut self, status: jack::ClientStatus, reason: &str) {
        if self.0 {
            println!(
                "JACK: shutdown with status {:?} because \"{}\"",
                status, reason
            );
        }
    }

    fn freewheel(&mut self, _: &jack::Client, is_enabled: bool) {
        if self.0 {
            println!(
                "JACK: freewheel mode is {}",
                if is_enabled { "on" } else { "off" }
            );
        }
    }

    fn sample_rate(&mut self, _: &jack::Client, state: jack::Frames) -> jack::Control {
        if self.0 {
            println!("JACK: sample rate changed to {}", state);
        }
        jack::Control::Continue
    }

    fn client_registration(&mut self, _: &jack::Client, name: &str, is_reg: bool) {
        if self.0 {
            println!(
            "JACK: {} client with name \"{}\"",
            if is_reg { "registered" } else { "unregistered" },
            name
            );
        }
    }

    fn port_registration(&mut self, _: &jack::Client, port_id: jack::PortId, is_reg: bool) {
        if self.0 {
            println!(
                "JACK: {} port with id {}",
                if is_reg { "registered" } else { "unregistered" },
                port_id
            );
        }
    }

    fn port_rename(
        &mut self,
        _: &jack::Client,
        port_id: jack::PortId,
        old_name: &str,
        new_name: &str,
    ) -> jack::Control {
        if self.0 {
            println!(
                "JACK: port with id {} renamed from {} to {}",
                port_id, old_name, new_name
            );
        }
        jack::Control::Continue
    }

    fn ports_connected(
        &mut self,
        _: &jack::Client,
        port_id_a: jack::PortId,
        port_id_b: jack::PortId,
        are_connected: bool,
    ) {
        if self.0 {
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
    }

    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        if self.0 {
            println!("JACK: graph reordered");
        }
        jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
        // I don't need this telling me I have xruns, I can see that in Catia
        if self.0 {
            println!("JACK: xrun occurred");
        }
        jack::Control::Continue
    }
}
