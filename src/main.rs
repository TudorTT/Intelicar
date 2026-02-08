
#![no_std]
#![no_main]



use embassy_executor::Spawner;
use embassy_net::{tcp::TcpSocket, StackResources};
use embassy_time::{Duration, Timer};

use embedded_io_async::Write;
use static_cell::StaticCell;
use cyw43::JoinOptions;

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};




use {defmt_rtt as _, panic_probe as _};

// GPIO
use embassy_rp::{gpio::{  Input, Level, Output, Pin, Pull }, pac::timer};


// PWM
use embassy_rp::pwm::{
    Config as ConfigPWM,
    SetDutyCycle,
    Pwm,
};

mod hcsr04;
use hcsr04::{HCSR04};

use defmt::*;



mod irqs;

const SOCK: usize = 4;
static RESOURCES: StaticCell<StackResources<SOCK>> = StaticCell::<StackResources<SOCK>>::new();
//THE CAR CONNECT TO WIFI
// WIFI NAME AND PASSWORD
const WIFI_NETWORK: &str = "------------";
const WIFI_PASSWORD: &str = "--------------";


#[repr(u8)]
enum Movement {
    Stop = 0,
    Forward,
    Reverse,
    Left,
    Right,
}

static MOVEMENT_COMMAND: AtomicU8 = AtomicU8::new(Movement::Stop as u8);


static OBSTACLE_DETECTED_F: AtomicBool = AtomicBool::new(false);
static OBSTACLE_DETECTED_B: AtomicBool = AtomicBool::new(false);
static OBSTACLE_DETECTED_R: AtomicBool = AtomicBool::new(false);
static OBSTACLE_DETECTED_L: AtomicBool = AtomicBool::new(false);
static MIN_DISTANCE: AtomicU8 = AtomicU8::new(255); 

const MAX_DISTANCE: u8 = 255;
const MIN_SAFE_DISTANCE_CM: f64 = 5.0;


static PULSE_COUNT_l: AtomicU8 = AtomicU8::new(0);
static PULSE_COUNT_r: AtomicU8 = AtomicU8::new(0);

#[embassy_executor::task]
async fn pulse_counter_l(mut sensor: Input<'static>) {
    loop {
        sensor.wait_for_falling_edge().await;
        PULSE_COUNT_l.fetch_add(1, Ordering::Relaxed);
    }
}

#[embassy_executor::task]
async fn pulse_reporter_l() {
    loop {
        Timer::after(Duration::from_secs(1)).await;
        let count = PULSE_COUNT_l.swap(0, Ordering::Relaxed);
        info!("Speed: {} pulses/sec", count);
    }
}
#[embassy_executor::task]
async fn pulse_counter_r(mut sensor: Input<'static>) {
    loop {
        sensor.wait_for_falling_edge().await;
        PULSE_COUNT_r.fetch_add(1, Ordering::Relaxed);
    }
}

#[embassy_executor::task]
async fn pulse_reporter_r() {
    loop {
        Timer::after(Duration::from_secs(1)).await;
        let count = PULSE_COUNT_r.swap(0, Ordering::Relaxed);
        info!("Speed: {} pulses/sec", count);
    }
}

#[embassy_executor::task]
pub async fn movement_task(
    mut in1: Output<'static>,
    mut in2: Output<'static>,
    mut in3: Output<'static>,
    mut in4: Output<'static>,
) -> ! {
    loop {
        match MOVEMENT_COMMAND.load(Ordering::Relaxed) {
            x if x == Movement::Forward as u8 => {
                if OBSTACLE_DETECTED_F.load(Ordering::Relaxed) {
                    stop(&mut in1, &mut in2, &mut in3, &mut in4).await;
                    MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed);

                } else {
                    in1.set_high();
                    in2.set_low();
                    in3.set_high();
                    in4.set_low();
                }
            },
            x if x == Movement::Reverse as u8 => {
                if OBSTACLE_DETECTED_B.load(Ordering::Relaxed) {
                    stop(&mut in1, &mut in2, &mut in3, &mut in4).await;
                    MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed);
                } else {
                    in1.set_low();
                    in2.set_high();
                    in3.set_low();
                    in4.set_high();
                }
            },
            x if x == Movement::Left as u8 => {
                if OBSTACLE_DETECTED_L.load(Ordering::Relaxed) || OBSTACLE_DETECTED_F.load(Ordering::Relaxed) {
                    stop(&mut in1, &mut in2, &mut in3, &mut in4).await;
                    MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed);
                } else {
                    in1.set_high();
                    in2.set_low();
                    in3.set_low();
                    in4.set_high();
                    Timer::after(Duration::from_millis(100)).await;
                    MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed);
                }
            },
            x if x == Movement::Right as u8 => {
                if OBSTACLE_DETECTED_R.load(Ordering::Relaxed ) || OBSTACLE_DETECTED_F.load(Ordering::Relaxed) {
                    stop(&mut in1, &mut in2, &mut in3, &mut in4).await;
                    MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed);
                } else {
                    in1.set_low();
                    in2.set_high();
                    in3.set_high();
                    in4.set_low();
                    Timer::after(Duration::from_millis(100)).await;
                    MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed);
                }
            },
            _ => {
                stop(&mut in1, &mut in2, &mut in3, &mut in4).await;
            }
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
pub async fn sensor_task(
    mut front: HCSR04,
    mut back: HCSR04,
    mut left: HCSR04,
    mut right: HCSR04,
) -> ! {
     // in cm

    loop {
        let distances = [
            front.measure().await.ok().map(|d| d.centimeters),
            back.measure().await.ok().map(|d| d.centimeters),
            left.measure().await.ok().map(|d| d.centimeters),
            right.measure().await.ok().map(|d| d.centimeters),
        ];

        if distances[0].unwrap_or(255.0) < MIN_SAFE_DISTANCE_CM {
            OBSTACLE_DETECTED_F.store(true, Ordering::Relaxed);
            info!("Obstacle detected in front! Distance: {} cm", distances[0].unwrap_or(255.0));
        } else {
            OBSTACLE_DETECTED_F.store(false, Ordering::Relaxed);
        }

        if distances[1].unwrap_or(255.0) < MIN_SAFE_DISTANCE_CM {
            OBSTACLE_DETECTED_B.store(true, Ordering::Relaxed);
            info!("Obstacle detected in back! Distance: {} cm", distances[1].unwrap_or(255.0));
        } else {
            OBSTACLE_DETECTED_B.store(false, Ordering::Relaxed);
        }

        if distances[2].unwrap_or(255.0) < MIN_SAFE_DISTANCE_CM {
            OBSTACLE_DETECTED_L.store(true, Ordering::Relaxed);
            info!("Obstacle detected in left! Distance: {} cm", distances[2].unwrap_or(255.0));
        } else {
            OBSTACLE_DETECTED_L.store(false, Ordering::Relaxed);
        }

        if distances[3].unwrap_or(255.0) < MIN_SAFE_DISTANCE_CM {
            OBSTACLE_DETECTED_R.store(true, Ordering::Relaxed);
            info!("Obstacle detected in right! Distance: {} cm", distances[3].unwrap_or(255.0));
        } else {
            OBSTACLE_DETECTED_R.store(false, Ordering::Relaxed);
        }

        let mut min = MAX_DISTANCE as f64;
        for d in distances.iter().flatten() {
            if *d < min {
                min = *d;
            }
        }

        MIN_DISTANCE.store(min as u8, Ordering::Relaxed);
        Timer::after(Duration::from_millis(50)).await;
    }
}


#[embassy_executor::task]
pub async fn buzzer_task(mut buzzer: Output<'static>) -> ! {
    loop {
        let dist = MIN_DISTANCE.load(Ordering::Relaxed);
        let (on, off) = match dist {
            0..=5 => (Duration::from_millis(100), Duration::from_millis(0)),
            6 => (Duration::from_millis(100), Duration::from_millis(25)),
            7 => (Duration::from_millis(100), Duration::from_millis(50)),
            8 => (Duration::from_millis(100), Duration::from_millis(100)),
            9 => (Duration::from_millis(100), Duration::from_millis(150)),
            10 => (Duration::from_millis(100), Duration::from_millis(200)),
            _ => (Duration::from_millis(0), Duration::from_millis(0)),
        };

        if on > Duration::from_millis(0) {
            buzzer.set_high();
            Timer::after(on).await;
            buzzer.set_low();
            Timer::after(off).await;
        } else {
            Timer::after(Duration::from_millis(100)).await;
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

   
    let sensorlightleft = Input::new(p.PIN_22, Pull::Up); 
    let sensorlightright = Input::new(p.PIN_21, Pull::Up);
    let mut gled= Output::new(p.PIN_7, Level::Low);
    let mut rled = Output::new(p.PIN_6, Level::Low);
    gled.set_low();
    rled.set_high();

    let mut  buzzer = Output::new(p.PIN_16, Level::Low); 
    let  sensorfront = HCSR04::new(p.PIN_15, p.PIN_14).unwrap();
    let  sensorback = HCSR04::new(p.PIN_8, p.PIN_9).unwrap();
    let sensorleft = HCSR04::new(p.PIN_11, p.PIN_10).unwrap();
    let sensorright = HCSR04::new(p.PIN_13, p.PIN_12).unwrap();


let mut config: ConfigPWM = Default::default();
  config.top = 0x8000;

let mut pwm = Pwm::new_output_ab( 
    p.PWM_SLICE5,
    p.PIN_26,   
    p.PIN_27,
    config.clone()
);
pwm.set_duty_cycle_percent(100);
    let seed = 0x0123_4567_89ab_cdef;
     let mut in1 = Output::new(p.PIN_20,Level::Low);
     let mut in2= Output::new(p.PIN_19,Level::Low);
     let mut in3 = Output::new(p.PIN_18,Level::Low);
     let mut in4= Output::new(p.PIN_17,Level::Low);
    

    // Init WiFi driver
    let (net_device, mut control) = embassy_lab_utils::init_wifi!(&spawner, p).await;

    // Default config for dynamic IP address
    let config = embassy_net::Config::dhcpv4(Default::default());

    // Init network stack
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );
    spawner.spawn(net_task(runner));

    loop {
        match control.join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes())).await {
            Ok(_) => {

                break;
            },
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }
    
    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    let ipv4_config = stack.config_v4().unwrap();
    info!("Connected! IP Address: {}", ipv4_config.address);    

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    // If we want to keep the connection open regardless of inactivity, we can set the timeout
    // to `None`
    socket.set_timeout(None);

    info!("Listening on TCP:1234...");
    if let Err(e) = socket.accept(1234).await {

        warn!("accept error: {:?}", e);
        
        //continue;
    }
    
    info!("Received connection from {:?}", socket.remote_endpoint());
    
    let mut buf = [0; 64];
    gled.set_high();
    rled.set_low();
    spawner.spawn(sensor_task(sensorfront,sensorback,sensorleft , sensorright )).unwrap();
    spawner.spawn(buzzer_task(buzzer)).unwrap();
    spawner.spawn(movement_task(in1, in2, in3, in4)).unwrap();
    spawner.spawn(pulse_counter_l(sensorlightleft)).unwrap();
    spawner.spawn(pulse_reporter_l()).unwrap();
    spawner.spawn(pulse_counter_r(sensorlightright)).unwrap();
    spawner.spawn(pulse_reporter_r()).unwrap();
loop {
    let n = match socket.read(&mut buf).await {
        Ok(0) => {
            warn!("read EOF");
            break;
        }
        Ok(n) => n,
        Err(e) => {
            warn!("read error: {:?}", e);
            break;
        }
    };

    if let Ok(command) = core::str::from_utf8(&buf[..n]) {
        let command = command.trim();
        info!("rxd command: {}", command);

        match command {
    "f" | "F" | "W" => MOVEMENT_COMMAND.store(Movement::Forward as u8, Ordering::Relaxed),
    "b" | "B" | "S"=> MOVEMENT_COMMAND.store(Movement::Reverse as u8, Ordering::Relaxed),
    "l" | "L" | "A"=> MOVEMENT_COMMAND.store(Movement::Left as u8, Ordering::Relaxed),
    "r" | "R" | "D"=> MOVEMENT_COMMAND.store(Movement::Right as u8, Ordering::Relaxed),
    "x" | "X" => MOVEMENT_COMMAND.store(Movement::Stop as u8, Ordering::Relaxed),
    "1" | "60" => { pwm.set_duty_cycle_percent(60).ok(); },
    "2" | "70" => { pwm.set_duty_cycle_percent(70).ok(); },
    "3" | "80" => { pwm.set_duty_cycle_percent(80).ok(); },
    "4" | "90" => { pwm.set_duty_cycle_percent(90).ok(); },
    "5" | "100"=> { pwm.set_duty_cycle_percent(100).ok(); },
    _ => warn!("Unknown command: {}", command),
}


       
        let mut response = [0u8; 64];
        let prefix = b"Executed: ";
        let newline = b"\n";

        let mut len = 0;
        response[..prefix.len()].copy_from_slice(prefix);
        len += prefix.len();

        let cmd_bytes = command.as_bytes();
        let copy_len = core::cmp::min(cmd_bytes.len(), response.len() - len - 1);
        response[len..len + copy_len].copy_from_slice(&cmd_bytes[..copy_len]);
        len += copy_len;

        response[len] = b'\n';
        len += 1;

        let _ = socket.write_all(&response[..len]).await;
    }
}

 }


async fn stop(
    in1: &mut Output<'_>, 
    in2: &mut Output<'_>, 
    in3: &mut Output<'_>, 
    in4: &mut Output<'_>
) {
    in1.set_low();
    in2.set_low();
    in3.set_low();
    in4.set_low();
    Timer::after(Duration::from_millis(100)).await;
}


/// This task runs the network stack, used for processing network events.
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}


