use std::{collections::HashMap, pin::Pin, str::FromStr, sync::Arc, time::Duration};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::{
	sync::mpsc,
	time::{sleep_until, Instant, Sleep},
};
use tracing::{error, trace, warn};

use crate::{
	p2p::{HookEvent, HookId},
	Peer, PeerStatus, RemoteIdentity, P2P,
};

/// The time between re-advertising the mDNS service.
const MDNS_READVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60); // Every minute re-advertise

/// Multicast DNS (mDNS) is used for discovery of peers over local networks.
#[derive(Debug)]
pub struct Mdns {
	p2p: Arc<P2P>,
	hook_id: HookId,
}

impl Mdns {
	pub fn spawn(p2p: Arc<P2P>) -> Result<Self, mdns_sd::Error> {
		let (tx, rx) = mpsc::channel(15);
		let hook_id = p2p.register_hook(tx);

		start(p2p.clone(), rx)?;

		Ok(Self { p2p, hook_id })
	}

	pub fn shutdown(self) {
		self.p2p.unregister_hook(self.hook_id);
	}
}

struct State {
	p2p: Arc<P2P>,
	service_domain: String,
	service_name: String,
	mdns_daemon: ServiceDaemon,
	next_mdns_advertisement: Pin<Box<Sleep>>,
}

fn start(p2p: Arc<P2P>, mut rx: mpsc::Receiver<HookEvent>) -> Result<(), mdns_sd::Error> {
	let service_domain = format!("_{}._udp.local.", p2p.app_name());
	let mut state = State {
		service_name: format!("{}.{service_domain}", p2p.remote_identity()),
		service_domain,
		p2p,
		mdns_daemon: ServiceDaemon::new()?,
		next_mdns_advertisement: Box::pin(sleep_until(
			Instant::now() + MDNS_READVERTISEMENT_INTERVAL,
		)),
	};
	let mdns_service = state.mdns_daemon.browse(&state.service_domain)?;

	tokio::spawn(async move {
		loop {
			tokio::select! {
				Some(event) = rx.recv() => match event {
					HookEvent::DiscoveredChange(_) => {},
					HookEvent::MetadataChange(_) | HookEvent::ListenersChange(_) => advertise(&mut state),
					HookEvent::Shutdown => shutdown(&mut state),
				},
				_ = &mut state.next_mdns_advertisement => advertise(&mut state),
				Ok(event) = mdns_service.recv_async() => on_event(&mut state, event)
			};
		}
	});

	Ok(())
}

fn advertise(state: &mut State) {
	let mut ports_to_service = HashMap::new();
	for addr in state.p2p.listeners().clone().values() {
		ports_to_service
			.entry(addr.port())
			.or_insert_with(Vec::new)
			.push(addr.ip());
	}

	let identity = state.p2p.remote_identity();
	let meta = state.p2p.metadata().clone();

	for (port, ips) in ports_to_service {
		let service = ServiceInfo::new(
			&state.service_domain,
			&identity.to_string(),
			&format!("{identity}.{}", state.service_domain),
			&*ips,
			port,
			// TODO: If a piece of metadata overflows a DNS record take care of splitting it across multiple.
			Some(meta.clone()),
		)
		.map(|s| s.enable_addr_auto());

		let service = match service {
			Ok(service) => service,
			Err(err) => {
				warn!("error creating mdns service info: {}", err);
				continue;
			}
		};

		trace!("advertising mdns service: {:?}", service);
		match state.mdns_daemon.register(service) {
			Ok(()) => {}
			Err(err) => warn!("error registering mdns service: {}", err),
		}
	}

	state.next_mdns_advertisement =
		Box::pin(sleep_until(Instant::now() + MDNS_READVERTISEMENT_INTERVAL));
}

fn on_event(state: &State, event: ServiceEvent) {
	match event {
		ServiceEvent::ServiceResolved(info) => {
			let Some(identity) = fullname_to_identity(&state, info.get_fullname()) else {
				return;
			};

			let mut discovered = state.p2p.discovered_mut();
			if let Some(peer) = discovered.get_mut(&identity) {
			} else {
				let mut peer = Peer::new();
				peer.set_state(PeerStatus::Discovered);
				let mut peer_meta = peer.service_mut();
				for property in info.get_properties().iter() {
					peer_meta.insert(property.key().to_string(), property.val_str().to_string());
				}

				// TODO: Allow adding extra metadata to services or something??? How does libp2p get this data???
				// info.get_addresses().iter().map(|addr| SocketAddr::new(*addr, info.get_port())).collect()

				discovered.insert(identity.clone(), peer);
			}
		}
		ServiceEvent::ServiceRemoved(_, fullname) => {
			let Some(identity) = fullname_to_identity(&state, &fullname) else {
				return;
			};

			todo!();
		}
		ServiceEvent::SearchStarted(_)
		| ServiceEvent::SearchStopped(_)
		| ServiceEvent::ServiceFound(_, _) => {}
	}
}

fn fullname_to_identity(
	State {
		p2p,
		service_domain: service_name,
		..
	}: &State,
	fullname: &str,
) -> Option<RemoteIdentity> {
	let Some(identity) = fullname.strip_prefix(&*service_name).map(|s| &s[1..]) else {
		warn!(
			"resolved peer advertising itself with an invalid fullname '{}'",
			fullname
		);
		return None;
	};

	let Ok(identity) = RemoteIdentity::from_str(identity) else {
		warn!("resolved peer advertising itself with an invalid remote identity '{identity}'");
		return None;
	};

	// Prevent discovery of the current peer.
	if identity == p2p.remote_identity() {
		return None;
	}

	Some(identity)
}

fn shutdown(state: &mut State) {
	state
		.mdns_daemon
		.unregister(&state.service_name)
		.map_err(|err| {
			error!(
				"error removing mdns service '{}': {err}",
				state.service_name
			);
		})
		.ok();

	// TODO: Without this mDNS is not sending it goodbye packets without a timeout. Try and remove this cause it makes shutdown slow.
	std::thread::sleep(Duration::from_millis(100));

	match state.mdns_daemon.shutdown() {
		Ok(chan) => {
			let _ = chan.recv();
		}
		Err(err) => {
			error!("error shutting down mdns daemon: {err}");
		}
	}
}