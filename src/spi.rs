//! SPI mock implementations.
//!
//! This mock supports the specification and checking of expectations to allow
//! automated testing of SPI based drivers. Mismatches between expected and
//! real SPI transactions will cause runtime assertions to assist with locating
//! faults.
//!
//! ## Usage
//!
//! ```
//! extern crate embedded_hal;
//! extern crate embedded_hal_mock;
//!
//! use embedded_hal::blocking::spi::{Transfer, Write};
//! use embedded_hal_mock::spi::{Mock as SpiMock, Transaction as SpiTransaction};
//! use embedded_hal::spi::FullDuplex;
//!
//! // Configure expectations
//! let expectations = [
//!     SpiTransaction::send(0x09),
//!     SpiTransaction::read(0x0A),
//!     SpiTransaction::send(0xFE),
//!     SpiTransaction::read(0xFF),
//!     SpiTransaction::write(vec![1, 2]),
//!     SpiTransaction::transfer(vec![3, 4], vec![5, 6]),
//! ];
//!
//! let mut spi: SpiMock<u8> = SpiMock::new(&expectations);
//! // FullDuplex transfers
//! spi.send(0x09);
//! assert_eq!(spi.read().unwrap(), 0x0A);
//! spi.send(0xFE);
//! assert_eq!(spi.read().unwrap(), 0xFF);
//!
//! // Writing
//! spi.write(&vec![1, 2]).unwrap();
//!
//! // Transferring
//! let mut buf = vec![3, 4];
//! spi.transfer(&mut buf).unwrap();
//! assert_eq!(buf, vec![5, 6]);
//!
//! // Finalise expectations
//! spi.done();
//! ```
use embedded_hal::blocking::spi;
use embedded_hal::spi::FullDuplex;
use nb;

use crate::common::Generic;
use crate::error::MockError;

/// SPI Transaction mode
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// Write transaction
    Write,
    /// Write and read transaction
    Transfer,
    /// Send transaction
    Send,
    /// After a send transaction in real HW a Read is available
    Read,
}

/// SPI transaction type
///
/// Models an SPI write or transfer (with response)
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction<W> {
    expected_mode: Mode,
    expected_data: Vec<W>,
    response: Vec<W>,
}

impl<W: Clone> Transaction<W> {
    /// Create a write transaction
    pub fn write(expected: Vec<W>) -> Self {
        Transaction {
            expected_mode: Mode::Write,
            expected_data: expected,
            response: Vec::new(),
        }
    }

    /// Create a transfer transaction
    pub fn transfer(expected: Vec<W>, response: Vec<W>) -> Self {
        Transaction {
            expected_mode: Mode::Transfer,
            expected_data: expected,
            response,
        }
    }

    /// Create a transfer transaction
    pub fn send(expected: W) -> Self {
        Transaction {
            expected_mode: Mode::Send,
            expected_data: [expected].to_vec(),
            response: Vec::new(),
        }
    }
    /// Create a transfer transaction
    pub fn read(response: W) -> Self {
        Transaction {
            expected_mode: Mode::Read,
            expected_data: Vec::new(),
            response: [response].to_vec(),
        }
    }
}

/// Mock SPI implementation
///
/// This supports the specification and checking of expectations to allow
/// automated testing of SPI based drivers. Mismatches between expected and
/// real SPI transactions will cause runtime assertions to assist with locating
/// faults.
///
/// See the usage section in the module level docs for an example.
pub type Mock<W> = Generic<Transaction<W>>;

macro_rules! spi {
    ($Word:ty) => {
        impl spi::Write<$Word> for Mock<$Word> {
            type Error = MockError;

            /// spi::Write implementation for Mock
            ///
            /// This will cause an assertion if the write call does not match the next expectation
            fn write(&mut self, buffer: &[$Word]) -> Result<(), Self::Error> {
                let w = self.next().expect("no expectation for spi::write call");
                assert_eq!(w.expected_mode, Mode::Write, "spi::write unexpected mode");
                assert_eq!(
                    &w.expected_data, &buffer,
                    "spi::write data does not match expectation"
                );
                Ok(())
            }
        }

        impl FullDuplex<$Word> for Mock<$Word> {
            type Error = MockError;
            /// spi::FullDuplex implementeation for Mock
            ///
            /// This will call the nonblocking read/write primitives.
            fn send(&mut self, buffer: $Word) -> nb::Result<(), Self::Error> {
                let data = self.next().expect("no expectation for spi::send call");
                assert_eq!(data.expected_mode, Mode::Send, "spi::send unexpected mode");
                assert_eq!(
                    data.expected_data[0], buffer,
                    "spi::send data does not match expectation"
                );
                Ok(())
            }

            /// spi::FullDuplex implementeation for Mock
            ///
            /// This will call the nonblocking read/write primitives.
            fn read(&mut self) -> nb::Result<$Word, Self::Error> {
                let w = self.next().expect("no expectation for spi::read call");
                assert_eq!(w.expected_mode, Mode::Read, "spi::Read unexpected mode");
                assert_eq!(
                    1,
                    w.response.len(),
                    "mismatched response length for spi::read"
                );
                let buffer: $Word = w.response[0];
                Ok(buffer)
            }
        }
        impl spi::Transfer<$Word> for Mock<$Word> {
            type Error = MockError;

            /// spi::Transfer implementation for Mock
            ///
            /// This writes the provided response to the buffer and will cause an assertion if the written data does not match the next expectation
            fn transfer<'w>(
                &mut self,
                buffer: &'w mut [$Word],
            ) -> Result<&'w [$Word], Self::Error> {
                let w = self.next().expect("no expectation for spi::transfer call");
                assert_eq!(
                    w.expected_mode,
                    Mode::Transfer,
                    "spi::transfer unexpected mode"
                );
                assert_eq!(
                    &w.expected_data, &buffer,
                    "spi::write data does not match expectation"
                );
                assert_eq!(
                    buffer.len(),
                    w.response.len(),
                    "mismatched response length for spi::transfer"
                );
                buffer.copy_from_slice(&w.response);
                Ok(buffer)
            }
        }
    };
}

spi!(u8);
spi!(u16);
spi!(u32);

#[cfg(test)]
mod test {
    use super::*;

    use embedded_hal::blocking::spi::{Transfer, Write};

    #[test]
    fn test_spi_mock_send() {
        let mut spi = Mock::new(&[Transaction::send(10u8)]);

        let _ = spi.send(10).unwrap();

        spi.done();
    }

    #[test]
    fn test_spi_mock_read() {
        let mut spi = Mock::new(&[Transaction::read(10u8)]);

        let ans = spi.read().unwrap();

        assert_eq!(ans, 10);

        spi.done();
    }

    #[test]
    fn test_spi_mock_multiple1() {
        let expectations = [
            Transaction::write(vec![1, 2u32]),
            Transaction::send(9),
            Transaction::read(10),
            Transaction::send(0xFE),
            Transaction::read(0xFF),
            Transaction::transfer(vec![3, 4], vec![5, 6]),
        ];
        let mut spi = Mock::new(&expectations);

        spi.write(&vec![1, 2]).unwrap();

        let _ = spi.send(0x09);
        assert_eq!(spi.read().unwrap(), 0x0a);
        let _ = spi.send(0xfe);
        assert_eq!(spi.read().unwrap(), 0xFF);
        let mut v = vec![3, 4];
        spi.transfer(&mut v).unwrap();

        assert_eq!(v, vec![5, 6]);

        spi.done();
    }

    #[test]
    fn test_spi_mock_write() {
        let expectations = [Transaction::write(vec![10, 12])];
        let mut spi = Mock::new(&expectations);

        spi.write(&vec![10u32, 12]).unwrap();

        spi.done();
    }

    #[test]
    fn test_spi_mock_transfer() {
        let expectations = [Transaction::transfer(vec![10, 12], vec![12, 13])];
        let mut spi = Mock::new(&expectations);

        let mut v = vec![10u16, 12];
        spi.transfer(&mut v).unwrap();

        assert_eq!(v, vec![12, 13]);

        spi.done();
    }

    #[test]
    fn test_spi_mock_multiple() {
        let expectations = [
            Transaction::write(vec![1, 2]),
            Transaction::transfer(vec![3, 4], vec![5, 6]),
        ];
        let mut spi = Mock::new(&expectations);

        spi.write(&vec![1u32, 2]).unwrap();

        let mut v = vec![3, 4];
        spi.transfer(&mut v).unwrap();

        assert_eq!(v, vec![5, 6]);

        spi.done();
    }

    #[test]
    #[should_panic]
    fn test_spi_mock_write_err() {
        let expectations = [Transaction::write(vec![10u8, 12])];
        let mut spi = Mock::new(&expectations);

        spi.write(&vec![10, 12, 12]).unwrap();

        spi.done();
    }

    #[test]
    #[should_panic]
    fn test_spi_mock_transfer_err() {
        let expectations = [Transaction::transfer(vec![10, 12], vec![12, 15])];
        let mut spi = Mock::new(&expectations);

        let mut v = vec![10u8, 12];
        spi.transfer(&mut v).unwrap();

        assert_eq!(v, vec![12, 13]);

        spi.done();
    }

    #[test]
    #[should_panic]
    fn test_spi_mock_transfer_response_err() {
        let expectations = [Transaction::transfer(vec![1, 2u32], vec![3, 4, 5])];
        let mut spi = Mock::new(&expectations);

        let mut v = vec![10, 12];
        spi.transfer(&mut v).unwrap();

        assert_eq!(v, vec![12, 13]);

        spi.done();
    }

    #[test]
    #[should_panic]
    fn test_spi_mock_mode_err() {
        let expectations = [Transaction::transfer(vec![10u8, 12], vec![])];
        let mut spi = Mock::new(&expectations);

        spi.write(&vec![10, 12, 12]).unwrap();

        spi.done();
    }

    #[test]
    #[should_panic]
    fn test_spi_mock_multiple_transaction_err() {
        let expectations = [
            Transaction::write(vec![10u8, 12]),
            Transaction::write(vec![10, 12]),
        ];
        let mut spi = Mock::new(&expectations);

        spi.write(&vec![10, 12, 12]).unwrap();

        spi.done();
    }
}
