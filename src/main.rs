use structopt::StructOpt;
use jack::{AsyncClient,ProcessHandler};

#[derive(StructOpt)]
#[structopt(about = "A cli application that applies the specified amount of delay between two input and two output ports.")]
struct Args {
	#[structopt(short = "n", help = "Number of frames to delay the signal by")]
	delay_frames: usize,
	#[structopt(short, parse(from_occurrences), help = "Verbosity (-vv for maximum output)")]
	verbosity: u8,
}

/// Pushes received frames into memory vector
fn receive_frames (in_port: &[f32], memory: &mut Vec<f32>) {
	for frame in in_port {
		memory.push(*frame);
	}
}

/// sends frames from memory vector to output port
/// if the vector is longer than the specified amount of frames.
fn send_frames (delay_frames: usize, out_port: &mut[f32], memory: &mut Vec<f32>, flush: &mut bool) {
	if *flush {
		out_port.clone_from_slice(&memory[..out_port.len()]);
		*memory = memory[out_port.len()..].into();
	} else if memory.len() >= delay_frames {
		*flush = true;
	} 
}

/// Handles printing notifications from Jack.
/// The const generic parameter specifies the verbosity of output.
struct Notifications<const V: u8>;

impl <const V:u8> jack::NotificationHandler for Notifications<V> {
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
		if V > 0 {
			println!(
				"JACK: freewheel mode is {}",
				if is_enabled { "on" } else { "off" }
			);
		}
	}

	fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
		if V > 0 {
			println!("JACK: sample rate changed to {}", srate);
		}
		jack::Control::Continue
	}

	fn client_registration(&mut self, _: &jack::Client, name: &str, is_reg: bool) {
		if V > 1 {
			println!(
				"JACK: {} client with name \"{}\"",
				if is_reg { "registered" } else { "unregistered" },
				name
			);
		}
	}

	fn port_registration(&mut self, _: &jack::Client, port_id: jack::PortId, is_reg: bool) {
		if V > 1 {
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
		if V > 1 {
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
		if V > 1 {
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
		if V > 1 {
			println!("JACK: graph reordered");
		}
		jack::Control::Continue
	}

	fn xrun(&mut self, _: &jack::Client) -> jack::Control {
		if V > 0 {
			println!("JACK: xrun occurred");
		}
		jack::Control::Continue
	}

	fn latency(&mut self, _: &jack::Client, mode: jack::LatencyType) {
		if V > 1 {
			println!(
				"JACK: {} latency has changed",
				match mode {
					jack::LatencyType::Capture => "capture",
					jack::LatencyType::Playback => "playback",
				}
			);
		}
	}
}

/// This is a setup for some horrible code later on
// TODO: find a better way to handle verbosity
enum ClientDump <P: Send + ProcessHandler, const A: u8, const B: u8, const C: u8>{
	ACli(AsyncClient<Notifications::<A>, P>),
	BCli(AsyncClient<Notifications::<B>, P>),
	CCli(AsyncClient<Notifications::<C>, P>),
}

fn main() {

	let args = Args::from_args();

	println!("Verbosity: {}, buffer: {}", args.verbosity, args.delay_frames);

	let v = args.verbosity;

	let (jack_client, _status) =
		jack::Client::new("rust_delay", jack::ClientOptions::NO_START_SERVER).unwrap();
	let in_1 = jack_client
		.register_port("in1", jack::AudioIn::default())
		.unwrap();
	let in_2 = jack_client
		.register_port("in2", jack::AudioIn::default())
		.unwrap();
	let mut out_1 = jack_client
		.register_port("out1", jack::AudioOut::default())
		.unwrap();
	let mut out_2 = jack_client
		.register_port("out2", jack::AudioOut::default())
		.unwrap();
	
	let mut flush = false;
	let mut mem1 = vec!();
	let mut mem2 = vec!();

	let process = jack::ClosureProcessHandler::new(
		// this closure gets called repeatedly to handle the audio frames.
		move | _: &jack::Client, ps: &jack::ProcessScope | -> jack::Control {
		let in_1_p = in_1.as_slice(ps);
		let in_2_p = in_2.as_slice(ps);
		let out_1_p = out_1.as_mut_slice(ps);
		let out_2_p = out_2.as_mut_slice(ps);

		receive_frames(in_1_p, &mut mem1);
		receive_frames(in_2_p, &mut mem2);
		send_frames(args.delay_frames, out_1_p, &mut mem1, &mut flush);
		send_frames(args.delay_frames, out_2_p, &mut mem2, &mut flush);
		jack::Control::Continue
		}
	);

	// The following code is stolen from Kat Maddox
	// https://twitter.com/ctrlshifti/status/1288745146759000064
	let _active_client = match v {
		0 => ClientDump::ACli(
			jack_client.activate_async(Notifications::<0>, process).unwrap()
		),
		1 => ClientDump::BCli(
			jack_client.activate_async(Notifications::<1>, process).unwrap()
		),
		_ => ClientDump::CCli(
			jack_client.activate_async(Notifications::<2>, process).unwrap()
		),
	};

	loop{}
}


