#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::{info, unwrap};
use embassy_executor::Executor;
use embassy_lora::iv::GenericSx126xInterfaceVariant;
use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::i2c::{Config as Config_I2C, InterruptHandler};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::I2C0;
use embassy_rp::spi::{Config as Config_SPI, Spi};
use embassy_rp::{bind_interrupts, i2c};
use embassy_time::Delay;
use receiver::gateway::*;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::{I2CDisplayInterface, Ssd1306};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

static mut CORE1_STACK: Stack<32768> = Stack::new(); // 32KiB
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
    // I2C1_IRQ => InterruptHandler<I2C1>;
});

// This initializes peripherals and sets up comms before starting the executors on both cores
#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    info!("Receiver v{}", VERSION);

    // Setup SPI for SD Card comms
    let sd_spi = Spi::new(
        p.SPI1,
        p.PIN_10,
        p.PIN_11,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        Config_SPI::default(),
    );
    let sdmmc_cs = Output::new(p.PIN_13.degrade(), Level::High);
    let sdmmc = embedded_sdmmc::sdcard::SdCard::new(sd_spi, sdmmc_cs, Delay {});

    // Setup SPI for LoRa comms
    let radio_spi: Spi<'_, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async> = Spi::new(
        p.SPI0,
        p.PIN_2,
        p.PIN_3,
        p.PIN_4,
        p.DMA_CH2,
        p.DMA_CH3,
        Config_SPI::default(),
    );
    let nss = Output::new(p.PIN_5.degrade(), Level::High);
    let reset = Output::new(p.PIN_14.degrade(), Level::High);
    let dio1 = Input::new(p.PIN_15.degrade(), Pull::None);
    let busy = Input::new(p.PIN_8.degrade(), Pull::None);
    let iv: GenericSx126xInterfaceVariant<Output<'_, embassy_rp::gpio::AnyPin>, Input<'_, embassy_rp::gpio::AnyPin>> =
        GenericSx126xInterfaceVariant::new(nss, reset, dio1, busy, None, None).unwrap();
    // let _radio = Radio::new(radio_spi, iv); // TODO: Initialize after executors are active??

    // Setup I2C for SSD1306 display
    let i2c = i2c::I2c::new_async(p.I2C0, p.PIN_17, p.PIN_16, Irqs, Config_I2C::default());
    let interface = I2CDisplayInterface::new(i2c);
    let display: Ssd1306<
        ssd1306::prelude::I2CInterface<i2c::I2c<'_, I2C0, i2c::Async>>,
        DisplaySize128x64,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
    > = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();

    // Core0 will handle user input as well as displaying the UI via an SSD1306 and a 128x64 display
    let core0 = Core0::new(display, p.PIN_18.degrade(), p.PIN_19.degrade(), p.PIN_20.degrade());

    // Core1 will handle LoRa and SD comms
    let core1 = Core1::new(radio_spi, iv, sdmmc);

    // Paid for two cores, i'm gonna use em'.
    spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor1 = EXECUTOR1.init(Executor::new());
        executor1.run(|spawner| unwrap!(spawner.spawn(core1_run(spawner, core1))));
    });

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        unwrap!(spawner.spawn(core0_run(spawner, core0)));
    });
}
