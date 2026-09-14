mod hal;
mod photonic;
mod leo;
mod svetoch;

pub use hal::{PhiSensor, SensorError, SensorResult, SensorPool, SensorConfig};
pub use photonic::PhotonicChipDriver;
pub use leo::LEOApiDriver;
pub use svetoch::SvetochBluetoothDriver;

use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_target(true)
        .init();

    info!("Catedral OS — Hardware HAL v304.0");

    let mut pool = SensorPool::new();

    if let Ok(photonic) = PhotonicChipDriver::new("/dev/ttyUSB0", 115200) {
        pool.add_sensor(Box::new(photonic));
    }

    let leo_endpoint = std::env::var("LEO_ENDPOINT").unwrap_or_default();
    let leo_token = std::env::var("LEO_TOKEN").unwrap_or_default();
    if !leo_endpoint.is_empty() {
        pool.add_sensor(Box::new(LEOApiDriver::new(&leo_endpoint, &leo_token)));
    }

    let svetoch_addr = std::env::var("SVETOCH_ADDRESS").unwrap_or_default();
    if !svetoch_addr.is_empty() {
        pool.add_sensor(Box::new(SvetochBluetoothDriver::new(&svetoch_addr)));
    }

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        interval.tick().await;
        match pool.read_phi().await {
            Ok(phi) => info!("Phi: {:.6}", phi),
            Err(e) => tracing::warn!("Read failed: {}", e),
        }
    }
}
