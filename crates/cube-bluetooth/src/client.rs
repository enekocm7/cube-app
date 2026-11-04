//! Scanning and connecting, on a background runtime.

use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    ValueNotification,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use cube_core::HardwareState;
use futures_util::{Stream, StreamExt as _};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot;

use crate::picker::{Device, ScanError};
use crate::protocol::{self, Update};

/// How long a timer gets to accept a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// How many times service discovery runs before giving up. Timers often
/// refuse the first attempts right after connecting.
const DISCOVERY_ATTEMPTS: u32 = 6;
const DISCOVERY_RETRY: Duration = Duration::from_millis(200);
/// How long a discovered service gets to list its characteristics.
const CHARACTERISTIC_ATTEMPTS: u32 = 10;
const CHARACTERISTIC_RETRY: Duration = Duration::from_millis(100);

/// The stream of a peripheral's notifications.
type Notifications = Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;
/// The stream of an adapter's events.
type CentralEvents = Pin<Box<dyn Stream<Item = CentralEvent> + Send>>;

/// Starts the runtime on first use, so people without a timer never pay for it.
fn runtime() -> Result<&'static Runtime, String> {
    static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("cube-bluetooth")
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// What a scan reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanEvent {
    Device(Device),
    /// The scan is over.
    Failed(ScanError),
}

/// A running scan, which stops when dropped.
pub struct Scan {
    events: flume::Receiver<ScanEvent>,
    _stop: oneshot::Sender<()>,
}

impl Scan {
    /// Waits for the next report. Returns `None` once the scan is over.
    pub async fn next(&self) -> Option<ScanEvent> {
        self.events.recv_async().await.ok()
    }
}

/// Why a connection could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectError {
    /// The timer is no longer in range or known to the system.
    NotFound,
    TimedOut,
    Failed(String),
}

/// Something wrong with a connection that stays up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// A state update was too short to read.
    IncompleteUpdate,
    UnsupportedState(u8),
    /// The finished time was too short to read, so the solve is lost.
    IncompleteTime,
    /// The finished time could not be read, so the solve is lost.
    TimeUnreadable(String),
}

/// What a connection reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    Connected,
    State(HardwareState),
    Problem(Problem),
    /// The connection was never made. Nothing follows.
    Failed(ConnectError),
    /// An established connection dropped. Nothing follows.
    Lost,
}

/// The reports of a connection.
pub struct LinkEvents(flume::Receiver<LinkEvent>);

impl LinkEvents {
    /// Waits for the next report. Returns `None` once the connection is over.
    pub async fn next(&self) -> Option<LinkEvent> {
        self.0.recv_async().await.ok()
    }
}

/// A connection, made or being made, which disconnects when dropped.
pub struct Link {
    stop: oneshot::Sender<()>,
    closed: oneshot::Receiver<Result<(), String>>,
}

impl Link {
    /// Disconnects the timer and waits for the result.
    pub async fn disconnect(self) -> Result<(), String> {
        let Self { stop, closed } = self;
        drop(stop);
        closed.await.unwrap_or(Ok(()))
    }
}

/// Starts scans and connections.
#[derive(Clone, Default)]
pub struct Client {
    /// The adapter of the latest scan. The system hands out peripherals per
    /// adapter, so a connection must use the one that found the timer.
    adapter: Arc<Mutex<Option<Adapter>>>,
}

impl Client {
    /// Looks for timers until the returned handle is dropped.
    pub fn scan(&self) -> Scan {
        let (events, receiver) = flume::unbounded();
        let (stop, stopped) = oneshot::channel();
        match runtime() {
            Ok(runtime) => {
                runtime.spawn(run_scan(Arc::clone(&self.adapter), events, stopped));
            }
            Err(error) => {
                let _ = events.send(ScanEvent::Failed(ScanError::Failed(error)));
            }
        }
        Scan {
            events: receiver,
            _stop: stop,
        }
    }

    /// Connects to a timer found by a scan.
    pub fn connect(&self, device: &Device) -> (Link, LinkEvents) {
        let (events, receiver) = flume::unbounded();
        let (stop, stopped) = oneshot::channel();
        let (closed, on_closed) = oneshot::channel();
        let adapter = self
            .adapter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match runtime() {
            Ok(runtime) => {
                let id = device.id.clone();
                runtime.spawn(run_link(adapter, id, events, stopped, closed));
            }
            Err(error) => {
                let _ = events.send(LinkEvent::Failed(ConnectError::Failed(error)));
            }
        }
        let link = Link {
            stop,
            closed: on_closed,
        };
        (link, LinkEvents(receiver))
    }
}

/// Returns the system's first Bluetooth adapter.
async fn find_adapter() -> Result<Adapter, ScanError> {
    let failed = |error: btleplug::Error| ScanError::Failed(error.to_string());
    let manager = Manager::new().await.map_err(failed)?;
    let adapters = manager.adapters().await.map_err(failed)?;
    adapters.into_iter().next().ok_or(ScanError::NoAdapter)
}

async fn run_scan(
    shared: Arc<Mutex<Option<Adapter>>>,
    events: flume::Sender<ScanEvent>,
    mut stop: oneshot::Receiver<()>,
) {
    let adapter = match find_adapter().await {
        Ok(adapter) => adapter,
        Err(error) => {
            let _ = events.send(ScanEvent::Failed(error));
            return;
        }
    };
    *shared.lock().unwrap_or_else(PoisonError::into_inner) = Some(adapter.clone());

    if let Err(error) = scan_until_stopped(&adapter, &events, &mut stop).await {
        let _ = events.send(ScanEvent::Failed(ScanError::Failed(error.to_string())));
    }
    // The scan may already be over, and then there is nothing left to stop.
    let _ = adapter.stop_scan().await;
}

async fn scan_until_stopped(
    adapter: &Adapter,
    events: &flume::Sender<ScanEvent>,
    stop: &mut oneshot::Receiver<()>,
) -> btleplug::Result<()> {
    let mut central = adapter.events().await?;
    adapter.start_scan(ScanFilter::default()).await?;
    loop {
        let event = tokio::select! {
            _ = &mut *stop => return Ok(()),
            event = central.next() => event,
        };
        let Some(event) = event else {
            return Err(btleplug::Error::Other("discovery stopped".into()));
        };
        let (CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id)) = event else {
            continue;
        };
        if let Some(device) = timer_device(adapter, &id).await
            && events.send(ScanEvent::Device(device)).is_err()
        {
            return Ok(());
        }
    }
}

/// Returns a discovered peripheral if it is a timer. Devices vanish mid-scan
/// all the time, and the next report of one that matters will get through.
async fn timer_device(adapter: &Adapter, id: &btleplug::platform::PeripheralId) -> Option<Device> {
    let peripheral = adapter.peripheral(id).await.ok()?;
    let properties = peripheral.properties().await.ok()??;
    let name = properties.local_name?;
    protocol::is_timer_name(&name).then(|| Device {
        id: id.to_string(),
        name,
    })
}

/// Finds the peripheral a scan reported.
async fn find_peripheral(adapter: &Adapter, id: &str) -> Result<Peripheral, ConnectError> {
    let peripherals = adapter
        .peripherals()
        .await
        .map_err(|error| ConnectError::Failed(error.to_string()))?;
    peripherals
        .into_iter()
        .find(|peripheral| peripheral.id().to_string() == id)
        .ok_or(ConnectError::NotFound)
}

/// Connects and subscribes, returning the characteristic holding the time.
async fn open(peripheral: &Peripheral) -> btleplug::Result<(Characteristic, Notifications)> {
    peripheral.connect().await?;
    let mut attempts = DISCOVERY_ATTEMPTS;
    while let Err(error) = peripheral.discover_services().await {
        attempts -= 1;
        if attempts == 0 {
            return Err(error);
        }
        tokio::time::sleep(DISCOVERY_RETRY).await;
    }

    let mut characteristics = peripheral.characteristics();
    for _ in 0..CHARACTERISTIC_ATTEMPTS {
        if !characteristics.is_empty() {
            break;
        }
        tokio::time::sleep(CHARACTERISTIC_RETRY).await;
        characteristics = peripheral.characteristics();
    }
    let find = |uuid| {
        characteristics
            .iter()
            .find(|found| found.service_uuid == protocol::SERVICE && found.uuid == uuid)
            .cloned()
            .ok_or(btleplug::Error::NoSuchCharacteristic)
    };
    let state = find(protocol::STATE_CHARACTERISTIC)?;
    let time = find(protocol::TIME_CHARACTERISTIC)?;

    let notifications = peripheral.notifications().await?;
    peripheral.subscribe(&state).await?;
    Ok((time, notifications))
}

/// Makes the connection, then reports the timer until it ends.
async fn run_link(
    adapter: Option<Adapter>,
    id: String,
    events: flume::Sender<LinkEvent>,
    mut stop: oneshot::Receiver<()>,
    closed: oneshot::Sender<Result<(), String>>,
) {
    let fail = |error| {
        let _ = events.send(LinkEvent::Failed(error));
    };
    let Some(adapter) = adapter else {
        fail(ConnectError::NotFound);
        return;
    };
    let peripheral = match find_peripheral(&adapter, &id).await {
        Ok(peripheral) => peripheral,
        Err(error) => {
            fail(error);
            return;
        }
    };
    // Taken before connecting, so a drop right after it is not missed. The
    // notification stream cannot tell: on Windows it outlives the connection.
    let central = match adapter.events().await {
        Ok(central) => central,
        Err(error) => {
            fail(ConnectError::Failed(error.to_string()));
            return;
        }
    };

    let opened = tokio::select! {
        _ = &mut stop => None,
        result = tokio::time::timeout(CONNECT_TIMEOUT, open(&peripheral)) => Some(result),
    };
    let (time, notifications) = match opened {
        Some(Ok(Ok(opened))) => opened,
        outcome => {
            match outcome {
                Some(Ok(Err(error))) => fail(ConnectError::Failed(error.to_string())),
                Some(Err(_)) => fail(ConnectError::TimedOut),
                _ => {}
            }
            // Nothing may be connected yet, so a failure here means nothing.
            let _ = peripheral.disconnect().await;
            return;
        }
    };

    let _ = events.send(LinkEvent::Connected);
    let session = Session {
        peripheral,
        time,
        central,
        notifications,
    };
    session.follow(&events, stop, closed).await;
}

/// An established connection.
struct Session {
    peripheral: Peripheral,
    /// The characteristic holding the time.
    time: Characteristic,
    central: CentralEvents,
    notifications: Notifications,
}

impl Session {
    /// Reports the timer until the caller disconnects or the connection drops.
    async fn follow(
        mut self,
        events: &flume::Sender<LinkEvent>,
        mut stop: oneshot::Receiver<()>,
        closed: oneshot::Sender<Result<(), String>>,
    ) {
        let id = self.peripheral.id();
        loop {
            tokio::select! {
                _ = &mut stop => {
                    let result = self.peripheral.disconnect().await;
                    let _ = closed.send(result.map_err(|error| error.to_string()));
                    return;
                }
                event = self.central.next() => match event {
                    Some(CentralEvent::DeviceDisconnected(lost)) if lost == id => break,
                    Some(_) => {}
                    None => break,
                },
                notification = self.notifications.next() => {
                    let Some(notification) = notification else {
                        break;
                    };
                    if notification.uuid == protocol::STATE_CHARACTERISTIC {
                        let value = &notification.value;
                        let _ = events.send(read_update(&self.peripheral, &self.time, value).await);
                    }
                }
            }
        }
        // The link is already down, so this only tidies up the system's side.
        let _ = self.peripheral.disconnect().await;
        let _ = events.send(LinkEvent::Lost);
    }
}

/// Turns a state notification into a report, reading the time if the attempt
/// finished.
async fn read_update(peripheral: &Peripheral, time: &Characteristic, value: &[u8]) -> LinkEvent {
    match protocol::decode_update(value) {
        Update::State(state) => LinkEvent::State(state),
        Update::Incomplete => LinkEvent::Problem(Problem::IncompleteUpdate),
        Update::Unsupported(state) => LinkEvent::Problem(Problem::UnsupportedState(state)),
        Update::Finished => match peripheral.read(time).await {
            Ok(bytes) => protocol::decode_time(&bytes)
                .map_or(LinkEvent::Problem(Problem::IncompleteTime), |time_ms| {
                    LinkEvent::State(HardwareState::Finished(time_ms))
                }),
            Err(error) => LinkEvent::Problem(Problem::TimeUnreadable(error.to_string())),
        },
    }
}
