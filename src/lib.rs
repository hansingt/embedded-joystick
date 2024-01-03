//! TODO: Document this library!
use core::cell::RefCell;
use core::fmt::{Display, Formatter};
use core::marker::PhantomData;

use critical_section::Mutex;
use embedded_hal::adc::{Channel, OneShot};
use embedded_hal::digital::v2::InputPin;

/// TODO: Document this error
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

/// TODO: Document this struct!
pub struct Joystick<'a, 'b, V, H, S, ADCV, ADCH, ADCv, ADCh, WordV, WordH>
where
    V: Channel<ADCV>,
    H: Channel<ADCH>,
    S: InputPin,
    ADCv: OneShot<ADCV, WordV, V>,
    ADCh: OneShot<ADCH, WordH, H>,
{
    vertical_channel: V,
    vertical_adc: &'a Mutex<RefCell<ADCv>>,
    horizontal_channel: H,
    horizontal_adc: &'b Mutex<RefCell<ADCh>>,
    switch: S,
    __phantom_adc_v: PhantomData<ADCV>,
    __phantom_adc_h: PhantomData<ADCH>,
    __phantom_word_v: PhantomData<WordV>,
    __phantom_word_h: PhantomData<WordH>,
}

impl<'a, 'b, V, H, S, ADCV, ADCH, ADCv, ADCh, WordV, WordH>
    Joystick<'a, 'b, V, H, S, ADCV, ADCH, ADCv, ADCh, WordV, WordH>
where
    V: Channel<ADCV>,
    H: Channel<ADCH>,
    S: InputPin,
    ADCv: OneShot<ADCV, WordV, V>,
    ADCh: OneShot<ADCH, WordH, H>,
{
    pub fn new(
        vertical_channel: V,
        vertical_adc: &'a Mutex<RefCell<ADCv>>,
        horizontal_channel: H,
        horizontal_adc: &'b Mutex<RefCell<ADCh>>,
        switch: S,
    ) -> Self {
        Self {
            vertical_channel,
            vertical_adc,
            horizontal_channel,
            horizontal_adc,
            switch,
            __phantom_adc_v: Default::default(),
            __phantom_adc_h: Default::default(),
            __phantom_word_v: Default::default(),
            __phantom_word_h: Default::default(),
        }
    }

    pub fn get_vertical(&mut self) -> nb::Result<WordV, ADCv::Error> {
        critical_section::with(|cs| {
            self.vertical_adc
                .borrow(cs)
                .borrow_mut()
                .read(&mut self.vertical_channel)
        })
    }

    pub fn get_horizontal(&mut self) -> nb::Result<WordH, ADCh::Error> {
        critical_section::with(|cs| {
            self.horizontal_adc
                .borrow(cs)
                .borrow_mut()
                .read(&mut self.horizontal_channel)
        })
    }

    pub fn switch_pressed(&self) -> Result<bool, S::Error> {
        self.switch.is_high()
    }

    pub fn get_position(
        &mut self,
    ) -> nb::Result<(WordV, WordH), JoystickError<ADCv::Error, ADCh::Error>> {
        let v = match self.get_vertical() {
            Ok(v) => Ok(v),
            Err(e) => match e {
                nb::Error::WouldBlock => Err(nb::Error::WouldBlock),
                nb::Error::Other(e) => Err(nb::Error::Other(JoystickError::VerticalADCError(e))),
            },
        }?;
        let h = match self.get_horizontal() {
            Ok(v) => Ok(v),
            Err(e) => match e {
                nb::Error::WouldBlock => Err(nb::Error::WouldBlock),
                nb::Error::Other(e) => Err(nb::Error::Other(JoystickError::HorizontalADCError(e))),
            },
        }?;
        Ok((v, h))
    }
}

#[cfg(test)]
mod tests {
    use embedded_hal_mock::adc::{
        Mock as AdcMock, MockChan0, MockChan1, Transaction as AdcTransaction,
    };
    use embedded_hal_mock::pin::{
        Mock as PinMock, State as PinState, Transaction as PinTransaction,
    };

    use super::*;

    #[test]
    fn test_switch_pressed() {
        let adc_expectations: [AdcTransaction<u8>; 0] = [];
        let pin_expectations = [
            PinTransaction::get(PinState::Low),
            PinTransaction::get(PinState::High),
        ];
        let v_chan = MockChan0 {};
        let h_chan = MockChan1 {};
        let adc = Mutex::new(RefCell::new(AdcMock::new(&adc_expectations)));
        let switch = PinMock::new(&pin_expectations);
        let joystick = Joystick::new(v_chan, &adc, h_chan, &adc, switch);

        assert!(!joystick.switch_pressed().unwrap());
        assert!(joystick.switch_pressed().unwrap());
        critical_section::with(|cs| adc.borrow(cs).borrow_mut().done());
    }

    #[test]
    fn test_get_position() {
        let v_chan = MockChan0 {};
        let h_chan = MockChan1 {};
        let adc_expectations = [
            AdcTransaction::read(0, 128u8),
            AdcTransaction::read(1, 128u8),
        ];
        let adc = Mutex::new(RefCell::new(AdcMock::new(&adc_expectations)));
        let switch = PinMock::new(&[]);
        let mut joystick = Joystick::new(v_chan, &adc, h_chan, &adc, switch);

        let (v, h) = joystick.get_position().unwrap();

        assert!(v == 128 && h == 128);

        critical_section::with(|cs| adc.borrow(cs).borrow_mut().done());
    }
}
