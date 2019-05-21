#![no_std]
#![no_main]

#[macro_use]
extern crate lazy_static;

// pick a panicking behavior
extern crate panic_halt; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// extern crate panic_abort; // requires nightly
// extern crate panic_itm; // logs messages over ITM; requires ITM support
// extern crate panic_semihosting; // logs messages to the host stderr; requires a debugger

use core::sync::atomic::{AtomicU32, Ordering};
use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;
use stm32l4::stm32l4x6;
use stm32l4::stm32l4x6::interrupt;

lazy_static! {
  static ref MUTEX_GPIOA: Mutex<RefCell<Option<stm32l4x6::GPIOA>>> = Mutex::new(RefCell::new(None));
  static ref MUTEX_GPIOC: Mutex<RefCell<Option<stm32l4x6::GPIOC>>> = Mutex::new(RefCell::new(None));
  static ref MUTEX_EXTI: Mutex<RefCell<Option<stm32l4x6::EXTI>>> = Mutex::new(RefCell::new(None));
}

static DELAY: AtomicU32 = AtomicU32::new(2000000);


#[entry]
fn main() -> ! {
  // get handles to the hardware
    let peripherals = stm32l4x6::Peripherals::take().unwrap();
    let gpioa = &peripherals.GPIOA;
    let gpioc = &peripherals.GPIOC;
    let syscfg = &peripherals.SYSCFG;
    let rcc = &peripherals.RCC;

    // Enable clocks for GPIOA(LED), GPIOC(Button), SYSCFG(EXTI)
    rcc.ahb2enr.write(|w| w.gpioaen().set_bit().gpiocen().set_bit());
    rcc.apb2enr.write(|w| w.syscfgen().set_bit());
    
    // Set GPIO directions
    // PA5 => output => LED
    // PC13 => input;pull-down => Button
    gpioa.moder.write(|w| w.moder5().output());
    gpioc.moder.write(|w| w.moder13().input());
    gpioc.pupdr.write(|w| unsafe { w.pupdr13().bits(0b10) } ); 

    // Map PC13 to EXTI13, unmask and trigger on rising edge
    syscfg.exticr4.write(|w| unsafe { w.exti13().bits(0b0010) } );
    let exti = &peripherals.EXTI;
    exti.imr1.write(|w| w.mr13().set_bit());
    exti.rtsr1.write(|w| w.tr13().set_bit());

    // Share peripherals
    cortex_m::interrupt::free(|cs| {
      MUTEX_GPIOA.borrow(cs).replace(Some(peripherals.GPIOA));
      MUTEX_GPIOC.borrow(cs).replace(Some(peripherals.GPIOC));
      MUTEX_EXTI.borrow(cs).replace(Some(peripherals.EXTI))
    });
  
    // Enable interrupt for button
    // 13 is between 15 and 10
    let mut nvic = cortex_m::Peripherals::take().unwrap().NVIC;
    nvic.enable(stm32l4x6::Interrupt::EXTI15_10);

    loop {
        // Switch LED on
        cortex_m::interrupt::free(|cs| {   
          let gpioa = MUTEX_GPIOA.borrow(cs).borrow();
          gpioa.as_ref().unwrap().odr.write(|w| w.odr5().high())
        });
        // Delay
        cortex_m::asm::delay(DELAY.load(Ordering::Relaxed));
        // Switch LED off
        cortex_m::interrupt::free(|cs| {   
          let gpioa = MUTEX_GPIOA.borrow(cs).borrow();
          gpioa.as_ref().unwrap().odr.write(|w| w.odr5().low());
        });
        // Delay
        cortex_m::asm::delay(DELAY.load(Ordering::Relaxed));
    }
}

#[interrupt]
fn EXTI15_10() {
    cortex_m::interrupt::free(|cs| {
        let exti = MUTEX_EXTI.borrow(cs).borrow();
        exti.as_ref().unwrap().pr1.modify(|_, w| w.pr13().set_bit());
    });
    // Switch between different modes
    match DELAY.load(Ordering::Relaxed) {
      2000000 => DELAY.store(1000000, Ordering::Relaxed),
      1000000 => DELAY.store(500000, Ordering::Relaxed),
      500000  => DELAY.store(2000000, Ordering::Relaxed),
      _ => () 
    }
}
