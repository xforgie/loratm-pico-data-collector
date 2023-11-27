use const_format::concatcp;
use defmt::{debug, error, info, unwrap};
use embassy_executor::Spawner;
use embassy_lora::iv::GenericSx126xInterfaceVariant;
use embassy_rp::gpio::{AnyPin, Input, Output, Pull};
use embassy_rp::i2c::{self};
use embassy_rp::peripherals::*;
use embassy_rp::spi::Spi;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Delay, Timer};
use embedded_graphics::geometry::Point;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::Drawable;
use embedded_sdmmc::{TimeSource, Timestamp, VolumeManager};
use ssd1306::prelude::{Brightness, DisplayConfig};
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306;

use crate::commands::*;
use crate::radio::Radio;

const INPUT_BUFFER_SIZE: usize = 4;
static INPUT_CHANNEL: Channel<CriticalSectionRawMutex, GatewayCommand, INPUT_BUFFER_SIZE> =
    Channel::<CriticalSectionRawMutex, GatewayCommand, INPUT_BUFFER_SIZE>::new();

const VERSION: &str = env!("CARGO_PKG_VERSION");
const HEADER: &str = concatcp!("RECEIVER v", VERSION);

// static INPUT_SIGNAL: Signal<CriticalSectionRawMutex, GatewayCommand> = Signal::new();

// Placeholder timestamping for writing files
#[derive(Debug)]
pub struct Clock;

impl TimeSource for Clock {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 53,
            zero_indexed_month: 11,
            zero_indexed_day: 148,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub struct Core0 {
    display: Ssd1306<
        ssd1306::prelude::I2CInterface<i2c::I2c<'static, I2C0, i2c::Async>>,
        DisplaySize128x64,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
    >,
    button0: AnyPin,
    button1: AnyPin,
    button2: AnyPin,
}

impl Core0 {
    pub fn new(
        display: Ssd1306<
            ssd1306::prelude::I2CInterface<i2c::I2c<'static, I2C0, i2c::Async>>,
            DisplaySize128x64,
            ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
        >,
        button0: AnyPin,
        button1: AnyPin,
        button2: AnyPin,
    ) -> Self {
        Self {
            display,
            button0,
            button1,
            button2,
        }
    }
}

pub struct Core1 {
    radio_spi: Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
    iv: GenericSx126xInterfaceVariant<
        Output<'static, embassy_rp::gpio::AnyPin>,
        Input<'static, embassy_rp::gpio::AnyPin>,
    >,
    sdmmc: embedded_sdmmc::SdCard<
        Spi<'static, embassy_rp::peripherals::SPI1, embassy_rp::spi::Async>,
        Output<'static, embassy_rp::gpio::AnyPin>,
        Delay,
    >,
}

impl Core1 {
    pub fn new(
        radio_spi: Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
        iv: GenericSx126xInterfaceVariant<
            Output<'static, embassy_rp::gpio::AnyPin>,
            Input<'static, embassy_rp::gpio::AnyPin>,
        >,
        sdmmc: embedded_sdmmc::SdCard<
            Spi<'static, embassy_rp::peripherals::SPI1, embassy_rp::spi::Async>,
            Output<'static, embassy_rp::gpio::AnyPin>,
            Delay,
        >,
    ) -> Self {
        Self { radio_spi, iv, sdmmc }
    }
}

#[embassy_executor::task]
pub async fn run_display(
    mut display: Ssd1306<
        ssd1306::prelude::I2CInterface<i2c::I2c<'static, I2C0, i2c::Async>>,
        DisplaySize128x64,
        ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    info!("Spawned task on Core0");
    info!("Initializing SSD1306");

    let x_bmp = [
        0b11111111, 0b11111111, 0b11000000, 0b00000011, 0b10100000, 0b00000101, 0b10010000, 0b00001001, 0b10001000,
        0b00010001, 0b10000100, 0b00100001, 0b10000010, 0b01000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001,
        0b10000010, 0b01000001, 0b10000100, 0b00100001, 0b10001000, 0b00010001, 0b10010000, 0b00001001, 0b10100000,
        0b00000101, 0b11000000, 0b00000011, 0b11111111, 0b11111111,
    ];

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    display.init().unwrap();
    display.set_brightness(Brightness::DIM).unwrap();
    display.clear_buffer();

    let raw_image = ImageRaw::<BinaryColor>::new(&x_bmp, 16);
    let image = Image::new(&raw_image, Point::zero());
    image.draw(&mut display).unwrap();

    Text::with_baseline(HEADER, Point::new(24, 4), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();

    info!("Display ready");

    let mut invert = false;
    let mut brightness = Brightness::NORMAL;

    loop {
        match INPUT_CHANNEL.receive().await {
            GatewayCommand::COMMAND0 => {
                display.set_invert(invert).unwrap();
                invert = !invert;
            }
            GatewayCommand::COMMAND1 => {
                match brightness {
                    Brightness::DIMMEST => brightness = Brightness::DIM,
                    Brightness::DIM => brightness = Brightness::NORMAL,
                    Brightness::NORMAL => brightness = Brightness::BRIGHT,
                    Brightness::BRIGHT => brightness = Brightness::BRIGHTEST,
                    _ => {}
                }
                display.set_brightness(brightness).unwrap();
            }
            GatewayCommand::COMMAND2 => {
                match brightness {
                    Brightness::BRIGHTEST => brightness = Brightness::BRIGHT,
                    Brightness::BRIGHT => brightness = Brightness::NORMAL,
                    Brightness::NORMAL => brightness = Brightness::DIM,
                    Brightness::DIM => brightness = Brightness::DIMMEST,
                    _ => {}
                }
                display.set_brightness(brightness).unwrap();
            }
        }
    }
}

#[embassy_executor::task(pool_size = 4)]
pub async fn read_input(button: AnyPin, command: &'static GatewayCommand) {
    debug!("Core1: Spawned input task");

    let mut button = Input::new(button, Pull::None);

    loop {
        button.wait_for_high().await;
        INPUT_CHANNEL.send(command.clone()).await;
        Timer::after_millis(100).await;
        button.wait_for_low().await;
        Timer::after_millis(100).await;
    }
}

#[embassy_executor::task]
pub async fn sd_card_test(
    sdmmc: embedded_sdmmc::SdCard<Spi<'static, SPI1, embassy_rp::spi::Async>, Output<'static, AnyPin>, Delay>,
) {
    debug!("Core1: Spawned SD Task");

    match sdmmc.num_bytes() {
        Ok(sd_size) => {
            info!("SD size: {}", sd_size);
            let volume_manager = VolumeManager::new(sdmmc, Clock {});
            match test_open_file(volume_manager) {
                Ok(_) => info!("Successfully wrote to file"),
                Err(e) => error!("Could not write to file: {}", e),
            }
        }
        Err(e) => error!("Could not determine size of SD: {}", e),
    }
}

fn test_open_file(
    mut volume_manager: VolumeManager<
        embedded_sdmmc::SdCard<Spi<'_, SPI1, embassy_rp::spi::Async>, Output<'_, AnyPin>, Delay>,
        Clock,
    >,
) -> Result<(), embedded_sdmmc::Error<embedded_sdmmc::SdCardError>> {
    let buffer = "Hello World!".as_bytes();

    let volume0 = volume_manager.open_volume(embedded_sdmmc::VolumeIdx(0))?;
    let root_dir = volume_manager.open_root_dir(volume0)?;
    let file = volume_manager.open_file_in_dir(root_dir, "file.txt", embedded_sdmmc::Mode::ReadWriteCreateOrAppend)?;
    volume_manager.write(file, buffer)?;

    volume_manager.close_file(file)?;
    volume_manager.close_dir(root_dir)?;
    volume_manager.close_volume(volume0)?;

    Ok(())
}

#[embassy_executor::task]
async fn run_radio(
    radio_spi: Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
    iv: GenericSx126xInterfaceVariant<
        Output<'static, embassy_rp::gpio::AnyPin>,
        Input<'static, embassy_rp::gpio::AnyPin>,
    >,
) {
    let _radio = Radio::new(radio_spi, iv).await;
}

#[embassy_executor::task]
pub async fn core1_run(spawner: Spawner, core: Core1) {
    info!("Running on Core 1!");

    unwrap!(spawner.spawn(run_radio(core.radio_spi, core.iv)));
    unwrap!(spawner.spawn(sd_card_test(core.sdmmc)));
}

#[embassy_executor::task]
pub async fn core0_run(spawner: Spawner, core: Core0) {
    info!("Running on Core 0!");

    unwrap!(spawner.spawn(run_display(core.display)));
    unwrap!(spawner.spawn(read_input(core.button0, &GatewayCommand::COMMAND0)));
    unwrap!(spawner.spawn(read_input(core.button1, &GatewayCommand::COMMAND1)));
    unwrap!(spawner.spawn(read_input(core.button2, &GatewayCommand::COMMAND2)));
}
