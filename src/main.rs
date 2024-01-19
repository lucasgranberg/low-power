#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Speed},
    low_power::{stop_ready, Executor, StopMode},
    pac,
    rtc::{Rtc, RtcConfig},
    time::Hertz,
};
use embassy_time::Duration;
use panic_probe as _;

static RTC: static_cell::StaticCell<Rtc> = static_cell::StaticCell::new();
#[cortex_m_rt::entry]
fn main() -> ! {
    Executor::take().run(|spawner| {
        spawner.spawn(async_main(spawner)).unwrap();
    });
}

#[embassy_executor::task]
async fn async_main(_spawner: Spawner) {
    #[cfg(feature = "defmt")]
    defmt::info!("START");
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(32_000_000),
            mode: HseMode::Oscillator,
            prescaler: HsePrescaler::DIV1,
        });
        config.rcc.msi = None;
        config.rcc.mux = ClockSrc::PLL1_R;
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV2,
            mul: PllMul::MUL6,
            divp: None,
            divq: Some(PllQDiv::DIV2), // PLL1_Q clock (32 / 2 * 6 / 2), used for RNG
            divr: Some(PllRDiv::DIV2), // sysclk 48Mhz clock (32 / 2 * 6 / 2)
        });
        config.rcc.ls = LsConfig::default_lse();
        config.enable_debug_during_sleep = cfg!(debug_assertions);
        //config.enable_debug_during_sleep = true;
    }
    let p = embassy_stm32::init(config);
    let rtc_config: RtcConfig = RtcConfig {
        frequency: Hertz(256),
    };
    let rtc = Rtc::new(p.RTC, rtc_config);
    let rtc = RTC.init(rtc);
    embassy_stm32::low_power::stop_with_rtc(rtc);
    let mut _en_sensor = Output::new(p.PA0, Level::High, Speed::Low);
    let mut _sensor = Output::new(p.PB4, Level::Low, Speed::Low);
    let _scl = Output::new(p.PB7, Level::Low, Speed::Low);
    let _sda = Output::new(p.PB8, Level::Low, Speed::Low);
    let mut led = Output::new(p.PB5, Level::Low, Speed::Low);
    let mut button = ExtiInput::new(Input::new(p.PB6, embassy_stm32::gpio::Pull::Up), p.EXTI6);
    loop {
        #[cfg(feature = "defmt")]
        {
            defmt::info!("LOOP");
            defmt::info!("STOP1 READY {}", stop_ready(StopMode::Stop1));
            defmt::info!("STOP2 READY {}", stop_ready(StopMode::Stop2));
        }
        led.set_high();
        embassy_time::Timer::after(Duration::from_millis(100)).await;
        led.set_low();

        let cen = pac::TIM2.cr1().read().cen();
        pac::TIM2.cr1().modify(|w| w.set_cen(false));

        // let button_fut = button.wait_for_any_edge();
        let delay_fut = embassy_time::Timer::after(Duration::from_millis(10000));
        delay_fut.await;
        // match select(button_fut, delay_fut).await {
        //     embassy_futures::select::Either::First(_) => {
        //         #[cfg(feature = "defmt")]
        //         defmt::info!("BUTTON");
        //     }
        //     embassy_futures::select::Either::Second(_) => {
        //         #[cfg(feature = "defmt")]
        //         defmt::info!("TIMER");
        //     }
        // }
        pac::TIM2.cr1().modify(|w| w.set_cen(cen));
        pac::RCC.cr().modify(|w: &mut pac::rcc::regs::Cr| {
            w.set_hsebyppwr(false);
            w.set_hseon(true);
        });
        while !pac::RCC.cr().read().hserdy() {}
    }
}
