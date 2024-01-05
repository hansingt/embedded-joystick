use core::cell::RefCell;
use core::fmt::{Display, Formatter};
use core::marker::PhantomData;
use std::fmt::Debug;

use critical_section::Mutex;
use embedded_hal::adc::{Channel, OneShot};
use embedded_hal::digital::v2::InputPin;

#[derive(Debug)]
pub enum JoystickError<VerticalError, HorizontalError> {
    VerticalADCError(VerticalError),
    HorizontalADCError(HorizontalError),
}

impl<VE: Display, HE: Display> Display for JoystickError<VE, HE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let e: &dyn Display = match self {
            JoystickError::VerticalADCError(e) => e,
            JoystickError::HorizontalADCError(e) => e,
        };
        write!(f, "{e}")
    }
}

struct Axis<'a, C, ADC, OS, Word>
where
    C: Channel<ADC>,
    OS: OneShot<ADC, Word, C>,
{
    channel: C,
    adc: &'a Mutex<RefCell<OS>>,
    _step: f32,
    _adc_marker: PhantomData<ADC>,
    _word_marker: PhantomData<Word>,
}

impl<'a, C, ADC, OS, Word> Axis<'a, C, ADC, OS, Word>
where
    C: Channel<ADC>,
    OS: OneShot<ADC, Word, C>,
    Word: Into<f32>,
{
    #[inline]
    fn new(adc: &'a Mutex<RefCell<OS>>, channel: C, resolution: usize) -> Self {
        let _step = 1. / (2u32.pow(resolution as u32) as f32);
        Self {
            channel,
            adc,
            _step,
            _adc_marker: PhantomData,
            _word_marker: PhantomData,
        }
    }

    fn read_raw(&mut self) -> nb::Result<Word, OS::Error> {
        critical_section::with(|cs| self.adc.borrow(cs).borrow_mut().read(&mut self.channel))
    }

    fn read(&mut self) -> nb::Result<f32, OS::Error> {
        match self.read_raw() {
            Ok(raw_value) => Ok(self._step * raw_value.into()),
            Err(error) => Err(error),
        }
    }
}

pub struct Joystick<'a, V, H, S, ADCV, ADCH, OSV, OSH, WordV, WordH>
where
    V: Channel<ADCV>,
    H: Channel<ADCH>,
    S: InputPin,
    OSV: OneShot<ADCV, WordV, V>,
    OSH: OneShot<ADCH, WordH, H>,
{
    vertical: Axis<'a, V, ADCV, OSV, WordV>,
    horizontal: Axis<'a, H, ADCH, OSH, WordH>,
    switch: S,
}

impl<'a, V, H, S, ADCV, ADCH, OSV, OSH, WordV, WordH>
    Joystick<'a, V, H, S, ADCV, ADCH, OSV, OSH, WordV, WordH>
where
    V: Channel<ADCV>,
    H: Channel<ADCH>,
    S: InputPin,
    OSV: OneShot<ADCV, WordV, V>,
    OSH: OneShot<ADCH, WordH, H>,
    WordV: Into<f32>,
    WordH: Into<f32>,
{
    pub fn new(
        vertical_channel: V,
        vertical_adc: &'a Mutex<RefCell<OSV>>,
        vertical_resolution: usize,
        horizontal_channel: H,
        horizontal_adc: &'a Mutex<RefCell<OSH>>,
        horizontal_resolution: usize,
        switch: S,
    ) -> Self {
        Self {
            vertical: Axis::new(vertical_adc, vertical_channel, vertical_resolution),
            horizontal: Axis::new(horizontal_adc, horizontal_channel, horizontal_resolution),
            switch,
        }
    }

    #[inline]
    pub fn get_vertical(&mut self) -> nb::Result<f32, OSV::Error> {
        self.vertical.read()
    }

    #[inline]
    pub fn get_horizontal(&mut self) -> nb::Result<f32, OSH::Error> {
        self.horizontal.read()
    }

    #[inline]
    pub fn switch_pressed(&self) -> Result<bool, S::Error> {
        self.switch.is_high()
    }

    pub fn get_position(
        &mut self,
    ) -> nb::Result<(f32, f32), JoystickError<OSV::Error, OSH::Error>> {
        let v = match self.get_vertical() {
            Ok(v) => v,
            Err(error) => {
                return match error {
                    nb::Error::WouldBlock => Err(nb::Error::WouldBlock),
                    nb::Error::Other(e) => {
                        Err(nb::Error::Other(JoystickError::VerticalADCError(e)))
                    }
                }
            }
        };
        let h = match self.get_horizontal() {
            Ok(h) => h,
            Err(error) => {
                return match error {
                    nb::Error::WouldBlock => Err(nb::Error::WouldBlock),
                    nb::Error::Other(e) => {
                        Err(nb::Error::Other(JoystickError::HorizontalADCError(e)))
                    }
                }
            }
        };
        Ok((v, h))
    }
}

#[cfg(test)]
mod tests {
    use embedded_hal_mock::adc::{
        Mock as AdcMock, MockAdc, MockChan0, MockChan1, Transaction as AdcTransaction,
    };
    use embedded_hal_mock::common::Generic;
    use embedded_hal_mock::pin::{
        Mock as PinMock, State as PinState, Transaction as PinTransaction,
    };

    use super::*;

    fn get_adc<Word: Eq + Clone + Debug>(
        v: Word,
        h: Word,
    ) -> (
        Mutex<RefCell<Generic<AdcTransaction<Word>>>>,
        MockChan0,
        MockChan1,
    ) {
        let v_chan = MockChan0 {};
        let h_chan = MockChan1 {};
        let adc_expectations = [
            AdcTransaction::read(MockChan0::channel(), v),
            AdcTransaction::read(MockChan1::channel(), h),
        ];
        let adc = Mutex::new(RefCell::new(AdcMock::new(&adc_expectations)));
        (adc, v_chan, h_chan)
    }

    fn get_switch(state: bool) -> PinMock {
        let pin_expectations = match state {
            true => [PinTransaction::get(PinState::High)],
            false => [PinTransaction::get(PinState::Low)],
        };
        PinMock::new(&pin_expectations)
    }

    fn get_joystick<'a, V, H, S, OSV, OSH, WordV: Into<f32>, WordH: Into<f32>>(
        adcv: &'a Mutex<RefCell<OSV>>,
        v: V,
        resx: usize,
        adch: &'a Mutex<RefCell<OSH>>,
        h: H,
        resy: usize,
        switch: S,
    ) -> Joystick<'a, V, H, S, MockAdc, MockAdc, OSV, OSH, WordV, WordH>
    where
        V: Channel<MockAdc>,
        H: Channel<MockAdc>,
        S: InputPin,
        OSV: OneShot<MockAdc, WordV, V>,
        OSH: OneShot<MockAdc, WordH, H>,
    {
        Joystick::new(v, adcv, resx, h, adch, resy, switch)
    }

    #[test]
    fn test_switch_pressed() {
        for state in [true, false] {
            let (adc, v, h) = get_adc(0u8, 0u8);
            let switch = get_switch(state);
            let joystick = get_joystick(&adc, v, 8, &adc, h, 8, switch);
            assert_eq!(joystick.switch_pressed().unwrap(), state)
        }
    }

    #[test]
    fn test_get_position() {
        for v in 0..=u8::MAX {
            for h in 0..=u8::MAX {
                let (adc, v_chan, h_chan) = get_adc(v, h);
                let switch = get_switch(false);
                let mut joystick = get_joystick(
                    &adc,
                    v_chan,
                    u8::BITS as usize,
                    &adc,
                    h_chan,
                    u8::BITS as usize,
                    switch,
                );
                let (pos_x, pos_y) = joystick.get_position().unwrap();
                assert_eq!(pos_x, v as f32 / 256.);
                assert_eq!(pos_y, h as f32 / 256.);
            }
        }
    }
}
