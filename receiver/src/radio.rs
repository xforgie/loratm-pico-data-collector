use embassy_lora::iv::GenericSx126xInterfaceVariant;
use embassy_rp::gpio::{AnyPin, Input, Output};
use embassy_rp::spi::Spi;
use embassy_time::Delay;
use lora_phy::mod_params::BoardType;
use lora_phy::sx1276_7_8_9::SX1276_7_8_9;
use lora_phy::LoRa;

const _LORA_FREQUENCY_IN_HZ: u32 = 915_000_000; // warning: set this appropriately for the region

pub struct Radio {
    _lora: LoRa<
        SX1276_7_8_9<
            Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
            GenericSx126xInterfaceVariant<Output<'static, AnyPin>, Input<'static, AnyPin>>,
        >,
        Delay,
    >,
}

impl Radio {
    pub async fn new(
        spi: Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
        iv: GenericSx126xInterfaceVariant<Output<'static, AnyPin>, Input<'static, embassy_rp::gpio::AnyPin>>,
    ) -> Self {
        let _lora: LoRa<
            SX1276_7_8_9<
                Spi<'_, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
                GenericSx126xInterfaceVariant<Output<'_, AnyPin>, Input<'_, AnyPin>>,
            >,
            Delay,
        > = LoRa::new(SX1276_7_8_9::new(BoardType::GenericSx1261, spi, iv), false, Delay)
            .await
            .unwrap();

        Self { _lora }
    }

    pub fn send(&self) {}

    pub fn receive(&self) {}
}
