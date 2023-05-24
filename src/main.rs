#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565, prelude::*,
};

use embedded_io::blocking::*;
use embedded_svc::{
    ipv4::Interface,
    wifi::{ClientConfiguration, Configuration, Wifi},
};

use esp32s3_hal::{
    clock::{ClockControl, CpuClock},
    gpio::IO,
    peripherals::Peripherals,
    prelude::*,
    timer::TimerGroup,
    Rtc,
    Rng,
    Delay,
    spi
};

use esp_wifi::{
    current_millis,
    initialize,
    wifi::{utils::create_network_interface, WifiMode},
    wifi_interface::WifiStack,
};

use esp_backtrace as _;
use esp_println::println;

use display_interface_spi::SPIInterfaceNoCS;
use mipidsi::{ColorOrder, Orientation};

use ui::{ build_ui, update_temperature };

use smoltcp::{
    iface::SocketStorage, 
    wire::{ IpAddress, Ipv4Address }
};
use esp_mbedtls::{set_debug, Mode, TlsVersion};
use esp_mbedtls::{Certificates, Session};
use minimq::{Minimq, QoS, Retain};


const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const CERT: &'static str = concat!(include_str!("../certs/AmazonRootCA1.pem"), "\0");
const CLIENT_CERT: &'static str = concat!(include_str!("../certs/device-certificate.pem.crt"), "\0");
const PRIVATE_KEY: &'static str = concat!(include_str!("../certs/private.pem.key"), "\0");

fn make_bits(bytes :&[u8]) -> u32 {
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | 0
}

#[entry]
fn main() -> ! {

    let peripherals = Peripherals::take();
    let mut system = peripherals.SYSTEM.split();
    let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock240MHz).freeze();
    let mut rtc = Rtc::new(peripherals.RTC_CNTL);

    // Disable the RTC and TIMG watchdog timers
    rtc.rwdt.disable();
    
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let sclk = io.pins.gpio7;
    let mosi = io.pins.gpio6;
    let mut backlight = io.pins.gpio45.into_push_pull_output();

    backlight.set_high().unwrap();

    let spi = spi::Spi::new_no_cs_no_miso(
        peripherals.SPI2,
        sclk,
        mosi,
        60u32.MHz(),
        spi::SpiMode::Mode0,
        &mut system.peripheral_clock_control,
        &clocks,
    );
    
    let di = SPIInterfaceNoCS::new(spi, io.pins.gpio4.into_push_pull_output());
    let reset = io.pins.gpio48.into_push_pull_output();
    let mut delay = Delay::new(&clocks);

    let mut display = mipidsi::Builder::ili9342c_rgb565(di)
        .with_display_size(320, 240)
        .with_orientation(Orientation::PortraitInverted(false))
        .with_color_order(ColorOrder::Bgr)
        .init(&mut delay, Some(reset))
        .unwrap();

    display.clear(Rgb565::WHITE).unwrap();

    build_ui(&mut display);

    let (wifi, _) = peripherals.RADIO.split();
    let mut socket_set_entries: [SocketStorage; 3] = Default::default();
    let (iface, device, mut controller, sockets) =
        create_network_interface(wifi, WifiMode::Sta, &mut socket_set_entries);
    let wifi_stack = WifiStack::new(iface, device, sockets, current_millis);

    let rngp = Rng::new(peripherals.RNG);
    let timer = TimerGroup::new(
        peripherals.TIMG1,
        &clocks,
        &mut system.peripheral_clock_control,
    );

    initialize(
        timer.timer0,
        rngp,
        system.radio_clock_control,
        &clocks,
    )
    .unwrap();

    println!("Call wifi_connect");
    let client_config = Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASSWORD.into(),
        ..Default::default()
    });

    controller.set_configuration(&client_config).unwrap();
    controller.start().unwrap();
    controller.connect().unwrap();

    println!("Wait to get connected");
    loop {
        let res = controller.is_connected();
        match res {
            Ok(connected) => {
                if connected {
                    break;
                }
            }
            Err(err) => {
                println!("{:?}", err);
                loop {}
            }
        }
    }

    println!("Wait to get an ip address");
    loop {
        wifi_stack.work();

        if wifi_stack.is_iface_up() {
            println!("Got ip {:?}", wifi_stack.get_ip_info());
            break;
        }
    }

    println!("We are connected!");

    println!("Making HTTP request");
    let mut rx_buffer = [0u8; 1536];
    let mut tx_buffer = [0u8; 1536];
    let mut socket = wifi_stack.get_socket(&mut rx_buffer, &mut tx_buffer);

    socket.work();

    socket
        .open(IpAddress::Ipv4(Ipv4Address::new(52, 28, 41, 87)), 8883)
        .unwrap();

    let certificates = Certificates {
        certs: Some(CERT),
        client_cert: Some(CLIENT_CERT),
        client_key: Some(PRIVATE_KEY),
        password: None,
    };

    let tls = Session::new(
        socket,
        "a3j3y1mdtdmkz5-ats.iot.eu-central-1.amazonaws.com",
        Mode::Client,
        TlsVersion::Tls1_2,
        certificates,
    )
    .unwrap();

    println!("Start tls connect");
    tls.connect().unwrap();

    println!("Did we even get here?");

    loop {

    }
}
