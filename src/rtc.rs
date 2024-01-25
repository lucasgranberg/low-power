use embassy_stm32::pac;
pub fn update_wake_up_timer(minutes: u16) {
    rtc_write(false, |rtc| {
        rtc.cr().modify(|w| w.set_wute(false));
        rtc.scr()
            .write(|w| w.set_cwutf(pac::rtc::vals::Calrf::CLEAR));
        if !rtc.icsr().read().initf() {
            while !rtc.icsr().read().wutwf() {
                #[cfg(feature = "defmt")]
                defmt::info!("wutwf");
            }
        }
        pac::RCC.apb1enr1().modify(|w| w.set_rtcapben(true));
        pac::RCC.cfgr().modify(|w| w.set_stopwuck(false));
        rtc.calr()
            .modify(|w| w.set_lpcal(pac::rtc::vals::Lpcal::CKAPRE));

        let (t, sel) = if minutes > 1080 {
            (
                (minutes - 1080) * 60,
                pac::rtc::vals::Wucksel::CLOCKSPAREWITHOFFSET,
            )
        } else {
            (minutes * 60, pac::rtc::vals::Wucksel::CLOCKSPARE)
        };
        rtc.wutr().modify(|w| {
            w.set_wut(t);
            w.set_wutoclr(0);
        });

        rtc.scr()
            .write(|w| w.set_cwutf(pac::rtc::vals::Calrf::CLEAR));
        rtc.cr().modify(|w| {
            // 1s - 18h
            w.set_wucksel(sel);
            w.set_wutie(true);
            w.set_wute(true);
        });
    });
}
pub fn init_rtc() {
    pac::RCC.apb1enr1().modify(|v| v.set_rtcapben(true));
    use pac::rtc::vals::*;
    #[cfg(feature = "defmt")]
    defmt::info!("init_rtc {}", pac::RTC.icsr().read().inits());
    rtc_write(true, |rtc| {
        rtc.cr().modify(|w| {
            //w.set_bypshad(true);
            w.set_bypshad(false);
            w.set_fmt(Fmt::TWENTYFOURHOUR);
            w.set_osel(Osel::DISABLED);
            w.set_pol(Pol::HIGH);
        });

        rtc.prer().modify(|w| {
            w.set_prediv_s(254);
            w.set_prediv_a(126);
        });

        // TODO: configuration for output pins
        rtc.cr().modify(|w| {
            w.set_out2en(false);
            w.set_tampalrm_type(TampalrmType::OPENDRAIN);
            w.set_tampalrm_pu(false);
        });
        rtc.icsr().modify(|w| {
            w.set_bin(Bin::BINARY);
        });
    });

    #[cfg(feature = "defmt")]
    defmt::info!("inited");
}

fn rtc_write<F, R>(init_mode: bool, f: F) -> R
where
    F: FnOnce(&pac::rtc::Rtc) -> R,
{
    use pac::rtc::vals::Key;
    let r = pac::RTC;
    pac::PWR.cr1().modify(|v| v.set_dbp(true));
    // Disable write protection.
    // This is safe, as we're only writin the correct and expected values.
    r.wpr().write(|w| w.set_key(Key::DEACTIVATE1));
    r.wpr().write(|w| w.set_key(Key::DEACTIVATE2));

    if init_mode && !r.icsr().read().initf() {
        r.icsr().modify(|w| w.set_init(true));
        // wait till init state entered
        // ~2 RTCCLK cycles
        if r.icsr().read().inits() {
            while !r.icsr().read().initf() {}
        }
    }

    let result = f(&r);

    if init_mode {
        r.icsr().modify(|w| w.set_init(false)); // Exits init mode
    }

    // Re-enable write protection.
    // This is safe, as the field accepts the full range of 8-bit values.
    r.wpr().write(|w| w.set_key(Key::ACTIVATE));
    pac::PWR.cr1().modify(|v| v.set_dbp(false));

    result
}
