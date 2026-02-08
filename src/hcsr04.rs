use core::fmt::Error;

pub type Result<T> = core::result::Result<T, Error>;

// This is pretty much a rewrite of
// https://github.com/marcoradocchia/hc-sr04/tree/master
// for embedded systems, especially the Raspberry Pi Pico
// We avoid using the std library and instead use the
// embassy crate for async programming
use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    peripherals::{PIN_2, PIN_3 ,PIN_16, PIN_17},
};
use embassy_time::{Duration, Instant, Timer};
pub struct Unit {
    pub millimeters: f64,
    pub centimeters: f64,
    pub decimeters: f64,
    pub meters: f64,
}

// 343 m/s
// 0.0343 cm/microsecond
const SPEED_OF_SOUND: f64 = 0.0343;

pub struct HCSR04 {
    trigger: Output<'static>,
    echo: Input<'static>,
}

impl HCSR04 {
    pub fn new(trigger_pin: impl embassy_rp::gpio::Pin, echo_pin: impl embassy_rp::gpio::Pin) -> Result<Self> {
        let mut trigger = Output::new(trigger_pin, Level::Low);
        let echo = Input::new(echo_pin, Pull::None);
        trigger.set_low();

        Ok(Self { trigger, echo })
    }

    fn calculate_speed(&mut self, duration: Duration) -> Unit {
        if duration.as_micros() < 100 || duration.as_millis() > 18 {
            return Unit {
                millimeters: 4000.0,
                centimeters: 400.0,
                decimeters: 40.0,
                meters: 4.0,
            };
        }

        let distance = (SPEED_OF_SOUND * (duration.as_micros() as f64)) / 2.0;

        if distance < 2.0 {
            return Unit {
                millimeters: 0.0,
                centimeters: 0.0,
                decimeters: 0.0,
                meters: 0.0,
            };
        }

        if distance > 400.0 {
            return Unit {
                millimeters: 4000.0,
                centimeters: 400.0,
                decimeters: 40.0,
                meters: 4.0,
            };
        }

        Unit {
            millimeters: distance * 10.0,
            centimeters: distance,
            decimeters: distance / 10.0,
            meters: distance / 100.0,
        }
    }

    pub async fn measure(&mut self) -> Result<Unit> {
        Timer::after(Duration::from_millis(10)).await;
        self.trigger.set_high();
        Timer::after(Duration::from_micros(10)).await;
        self.trigger.set_low();

        self.echo.wait_for_high().await;
        let instant = Instant::now();
        self.echo.wait_for_low().await;

        Ok(self.calculate_speed(instant.elapsed()))
    }
}
