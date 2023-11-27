#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use {defmt_rtt as _, panic_probe as _};

#[cortex_m_rt::entry]
fn main() -> ! {
    let _ = embassy_rp::init(Default::default());

    loop {}
}
