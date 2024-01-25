#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(maybe_uninit_as_bytes)]
#![feature(maybe_uninit_slice)]
use core::ptr::addr_of_mut;
use cortex_m::asm;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    gpio::{Level, Output, Speed},
    pac,
    time::Hertz,
    Peripherals,
};
use embassy_time::Duration;
use grounded::uninit::GroundedCell;
use panic_probe as _;
mod rtc;
use rtc::*;

const X25: crc::Crc<u8> = crc::Crc::<u8>::new(&crc::CRC_8_AUTOSAR);
#[derive(Copy, Clone)]
#[cfg(feature = "defmt")]
#[derive(defmt::Format)]
#[repr(C)]
struct Mac {
    foo: u32,
    bar: u8,
}
#[derive(PartialEq, Eq, Copy, Clone)]
#[repr(u32)]
enum Magic {
    Used = 0x5e1ec7ed,
    Primed = 0x7e117a1e,
}
#[derive(Copy, Clone)]
#[repr(C)]
struct Warm {
    magic: Magic,
    mac: Mac,
    crc: u8,
}
extern "C" {
    static mut WARM_DATA: GroundedCell<Warm>;
}

#[embassy_executor::main]
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
    }
    let mut p = embassy_stm32::init(config);
    //test_rtc(&mut p).await;
    init_rtc();
    update_wake_up_timer(1);
    wake();
    do_stuff(&mut p).await.unwrap();
    standby()
}
fn wake() {
    pac::RCC.csr().write(|v| {
        v.set_rmvf(true);
    })
}
fn standby() -> ! {
    #[cfg(feature = "defmt")]
    defmt::info!("STANDBY");
    pac::FLASH.optr().modify(|v| {
        v.set_n_rst_stdby(true);
        v.set_sram2_rst(false);
    });
    //PC13 WKUP
    pac::PWR.scr().write(|v| {
        v.set_cwuf(0, true);
        v.set_cwuf(1, true);
        v.set_cwuf(2, true);
        v.set_cc2hf(true);
        v.set_cwpvdf(true);
        v.set_cwrfbusyf(true);
    });
    // PA0 en_sensor pull high
    pac::PWR.pucr(0).modify(|v| v.set_p(0, true));
    pac::PWR.pdcr(2).modify(|v| v.set_p(13, true));
    pac::PWR
        .cr4()
        .modify(|v| v.set_wp(1, pac::pwr::vals::Wp::RISINGEDGE));
    pac::PWR.cr3().modify(|v| {
        v.set_rrs(true);
        v.set_eiwul(true);
        v.set_eulpen(true);
        v.set_ewup(0, false);
        v.set_ewup(1, true);
        v.set_ewup(2, false);
        v.set_apc(true);
    });
    while !pac::PWR.sr2().read().reglps() {}
    pac::PWR.cr1().modify(|w| {
        w.set_lpms(pac::pwr::vals::Lpms::STANDBY);
        w.set_lpr(pac::pwr::vals::Lpr::LOWPOWERMODE);
    });

    pac::RCC.ahb1enr().modify(|w| w.set_dma1en(false));
    pac::RCC.ahb1enr().modify(|w| w.set_dma2en(false));
    //pac::RCC.apb1enr1().modify(|w| w.set_rtcapben(false));
    unsafe {
        let mut p = cortex_m::Peripherals::steal();
        cortex_m::peripheral::SCB::set_sleepdeep(&mut p.SCB);
    }
    asm::dsb();
    asm::wfi();
    // asm::isb();
    // unsafe {
    //     let mut p = cortex_m::Peripherals::steal();
    //     cortex_m::peripheral::SCB::clear_sleepdeep(&mut p.SCB);
    // }
    cortex_m::peripheral::SCB::sys_reset()
}
async fn do_stuff(p: &mut Peripherals) -> Result<(), ()> {
    let mut led = Output::new(&mut p.PB5, Level::Low, Speed::Low);
    let warm = get_warm();
    #[cfg(feature = "defmt")]
    defmt::info!("do_stuff {}", warm.mac.bar);

    led.set_high();
    embassy_time::Timer::after(Duration::from_millis(1000)).await;
    led.set_low();
    embassy_time::Timer::after(Duration::from_millis(1000)).await;
    for _ in 0..warm.mac.bar {
        led.set_high();
        embassy_time::Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        embassy_time::Timer::after(Duration::from_millis(100)).await;
    }
    warm.mac.bar += 1;
    if warm.mac.bar > 5 {
        warm.mac.bar = 1;
    }
    warm.magic = Magic::Primed;
    warm.crc = X25.checksum(unsafe { any_as_u8_slice(&warm.mac) });
    defmt::info!("done stuff {}", warm.mac.bar);
    Ok(())
}
fn get_warm<'a>() -> &'a mut Warm {
    #[cfg(feature = "defmt")]
    defmt::info!("get_warm");
    unsafe {
        let warm_data_ptr = WARM_DATA.get();
        let magic_ptr = addr_of_mut!((*warm_data_ptr).magic);
        let mac_ptr = addr_of_mut!((*warm_data_ptr).mac);
        let crc_ptr = addr_of_mut!((*warm_data_ptr).crc);
        if magic_ptr.read_volatile() != Magic::Primed
            || crc_ptr.read_volatile()
                != X25.checksum(::core::slice::from_raw_parts(
                    mac_ptr as *const u8,
                    ::core::mem::size_of::<Mac>(),
                ))
        {
            cold_start(warm_data_ptr);
        }
        let warm = &mut *warm_data_ptr;
        warm.magic = Magic::Used;
        warm
    }
}
unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::core::slice::from_raw_parts((p as *const T) as *const u8, ::core::mem::size_of::<T>())
}

unsafe fn cold_start(warm_data_ptr: *mut Warm) {
    #[cfg(feature = "defmt")]
    defmt::info!("cold_start");
    let mac = Mac { foo: 0, bar: 1 };
    let warm = Warm {
        magic: Magic::Used,
        mac,
        crc: X25.checksum(any_as_u8_slice(&mac)),
    };
    update_wake_up_timer(1);
    (*warm_data_ptr) = warm;
}
