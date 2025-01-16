#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use embassy_executor::Spawner;
use embassy_net::Runner;
use embassy_net::StackResources;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::efuse::Efuse;
use esp_hal::{
    clock::CpuClock,
    rng::Trng,
    timer::timg::TimerGroup,
    twai::{self, TwaiMode},
};
use esp_println::println;
use esp_wifi::{
    init,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
        WifiState,
    },
    EspWifiController,
};
use heapless::String as HString;
mod mender_mcu_client;
use crate::mender_mcu_client::add_ons::inventory::mender_inventory::{
    MenderInventoryConfig, MENDER_INVENTORY_ADDON_INSTANCE,
};
use crate::mender_mcu_client::core::mender_client::{
    mender_client_activate, mender_client_init, MenderClientCallbacks, MenderClientConfig,
};
use crate::mender_mcu_client::core::mender_utils::{
    DeploymentStatus, KeyStore, KeyStoreItem, MenderError, MenderResult,
};
use mender_mcu_client::{
    add_ons::inventory::mender_inventory,
    core::mender_client,
    platform::scheduler::mender_scheduler::{
        mender_scheduler_work_activate, mender_scheduler_work_create,
        mender_scheduler_work_deactivate, mender_scheduler_work_set_period, MenderFuture,
    },
};

mod custom;
mod global_variables;

const WIFI_SSID: &'static str = "Your SSID";
const WIFI_PSK: &'static str = "Your PSK";

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

// Example usage:
fn network_connect_cb() -> MenderResult<()> {
    log_info!("network_connect_cb");
    // Implementation
    Ok(())
}

fn network_release_cb() -> MenderResult<()> {
    log_info!("network_release_cb");
    // Implementation
    Ok(())
}

fn authentication_success_cb() -> MenderResult<()> {
    log_info!("authentication_success_cb");
    // Implementation
    Ok(())
}

fn authentication_failure_cb() -> MenderResult<()> {
    log_info!("authentication_failure_cb");
    // Implementation
    Ok(())
}

fn deployment_status_cb(status: DeploymentStatus, message: Option<&str>) -> MenderResult<()> {
    log_info!("deployment_status_cb");
    // Implementation
    Ok(())
}

fn restart_cb() -> MenderResult<()> {
    log_info!("restart_cb");
    // Implementation
    Ok(())
}

fn mender_client_work_test() -> MenderFuture {
    Box::pin(async {
        match my_work_function().await {
            MenderError::Done => Ok(()),
            _ => Err("Work failed"),
        }
    })
}

async fn my_work_function() -> MenderError {
    println!("Doing some work...");
    MenderError::Done
}

// Make the config static
static INVENTORY_CONFIG: MenderInventoryConfig = MenderInventoryConfig {
    refresh_interval: 0,
};

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });
    esp_alloc::heap_allocator!(100 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timg1.timer0);
    let trng = &mut *mk_static!(Trng<'static>, Trng::new(peripherals.RNG, peripherals.ADC1));

    let init = &*mk_static!(
        EspWifiController<'static>,
        init(timg0.timer0, trng.rng, peripherals.RADIO_CLK).unwrap()
    );

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(init, wifi, WifiStaDevice).unwrap();
    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = (trng.rng.random() as u64) << 32 | trng.rng.random() as u64;

    // // Init network stack
    // let stack = &*mk_static!(
    //     Stack<WifiDevice<'_, WifiStaDevice>>,
    //     Stack::new(
    //         wifi_interface,
    //         config,
    //         mk_static!(StackResources<3>, StackResources::<3>::new()),
    //         seed
    //     )
    // );
    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    spawner
        .spawn(connection(controller))
        .expect("connection spawn");
    spawner.spawn(net_task(runner)).expect("net task spawn");

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Starting async main...");

    let mac_address = Efuse::mac_address();
    let mac_str = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5]
    );

    let identity = {
        let mut store = KeyStore::new();
        store.set_item("mac", &mac_str).unwrap();
        store
    };

    // let config = MenderClientConfig::new(
    //     identity,
    //     "artifact-1.0",
    //     "esp32c6",
    //     "https://hosted.mender.io",
    //     Some("eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJtZW5kZXIudGVuYW50IjoiNjVkODMyYjNkY2I2ODI1YmQ2OWJjZGRmIiwiaXNzIjoiTWVuZGVyIiwic3ViIjoiNjVkODMyYjNkY2I2ODI1YmQ2OWJjZGRmIn0.oPgY1QLpvMlNJzc9_ZVbrNlWpAvqtZXXHWilw6kVZD-0HZQNZGt4nXbvOFrekfbgU7zHfK9N6ovqWffa7MjqFjceEfbpagYASchFcuqRZPBGTc5MBUmF0YZWzvaw0pBYLK5sakUiEVoAvQJsSdy75NcipTlHneaB96y5WoPBdP7fkdRb0UIWBIHi4O5ZFwDYgaP5SJBj9i-akoIvqnTsZjGfATUuqpNIErnE4yPwn0Rf2CgIdrgl2daTZAwFB0lbHC_Xm2IT5LjbODdTvtnJyVfYoIpU0Bn34YoCl538sPbzIsyArIit8D3uQ8aeviUiyXt857dSbSBE6wHV0gsJMxjBQZApFaYIH4FEk7g2PEV5Q3Fo0-TcL6BXrE10u3DDOMZbspLrqozq_eVfWth6aa_5fNlKIoZeesuwd4QJlviwUSRnCBdN2W-Elu8bhKSfRRmLPX5RL6g_BMyrM-wvcV96kFobZy52IZuMIjAex3I3p7gCu4IxWGB1KrxnmJPi")
    // ).with_recommissioning(false);

    let config = MenderClientConfig::new(
        identity,
        "artifact-1.0",
        "esp32c6",
        "https://mender.bluleap.ai",
        None,
    )
    .with_recommissioning(false);

    // Creating an instance:
    let callbacks = MenderClientCallbacks::new(
        network_connect_cb,
        network_release_cb,
        authentication_success_cb,
        authentication_failure_cb,
        deployment_status_cb,
        restart_cb,
    );

    mender_client_init(&spawner, &config, &callbacks, trng, stack)
        .await
        .expect("Failed to init mender client");

    // In your main function or setup code:
    match mender_client::mender_client_register_addon(
        &MENDER_INVENTORY_ADDON_INSTANCE,
        Some(&INVENTORY_CONFIG), // Use the static config
        None,
    )
    .await
    {
        Ok(_) => {
            log_info!("Mender inventory add-on registered successfully");
        }
        Err(_) => {
            log_error!("Unable to register mender-inventory add-on");
            panic!("Failed to register mender-inventory add-on");
        }
    }

    // Create a work
    let mut work = mender_scheduler_work_create(mender_client_work_test, 5, "my_work")
        .await
        .expect("Failed to create work");

    let mut work2 = mender_scheduler_work_create(mender_client_work_test, 5, "my_work2")
        .await
        .expect("Failed to create work");

    // Change period if needed
    mender_scheduler_work_set_period(&mut work, 10)
        .await
        .expect("Failed to set period");
    mender_scheduler_work_set_period(&mut work2, 2)
        .await
        .expect("Failed to set period");

    // Activate the work
    mender_scheduler_work_activate(&mut work)
        .await
        .expect("Failed to activate work");
    mender_scheduler_work_activate(&mut work2)
        .await
        .expect("Failed to activate work");

    // Define the inventory items
    let inventory = [
        KeyStoreItem {
            name: HString::<32>::try_from("mender-mcu-client").unwrap(),
            value: HString::<32>::try_from("1.0.0").unwrap(), // Replace with actual version
        },
        KeyStoreItem {
            name: HString::<32>::try_from("latitude").unwrap(),
            value: HString::<32>::try_from("45.8325").unwrap(),
        },
        KeyStoreItem {
            name: HString::<32>::try_from("longitude").unwrap(),
            value: HString::<32>::try_from("6.864722").unwrap(),
        },
    ];

    let mut keystore = KeyStore::new();
    for item in &inventory {
        keystore.set_item(&item.name, &item.value).unwrap();
    }
    // Set the inventory
    match mender_inventory::mender_inventory_set(&keystore).await {
        Ok(_) => {
            log_info!("Mender inventory set successfully");
        }
        Err(_) => {
            log_error!("Unable to set mender inventory");
        }
    }

    match mender_client_activate().await {
        MenderError::Done => println!("Client activated successfully"),
        _ => panic!("Failed to activate client"),
    };

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn connection(
    mut controller: WifiController<'static>,
    //stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
) {
    log::info!("start connection task");
    log::info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: WIFI_SSID.try_into().expect("Wifi ssid parse"),
                password: WIFI_PSK.try_into().expect("Wifi psk parse"),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            log::info!("Starting wifi");
            controller.start_async().await.unwrap();
            log::info!("Wifi started!");
        }
        log::info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => {
                log::info!("Wifi connected!");

                // loop {
                //     if stack.is_link_up() {
                //         break;
                //     }
                //     Timer::after(Duration::from_millis(500)).await;
                // }
            }
            Err(e) => {
                log::info!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}
