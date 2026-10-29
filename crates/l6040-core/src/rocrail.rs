//! A turnout number as Rocrail writes it. Rocrail's turnout dialog has the fields Address,
//! Port and Gate, and accepts three ways of filling them in (formulas from Rocrail's
//! rocutils/impl/addr.c and rocdigs/impl/loconet.c):
//!
//! - MADA, decoder and port: Address = decoder 1.., Port = 1 to 4
//! - PADA, the turnout number: Address = 0, Port = the turnout number
//! - FADA, every output counted from 0: Address = 2 × (turnout − 1) + gate, Port = 0
//!
//! The turnout number is the one the Keyboard 6040, the Intellibox, the IB-Switch and
//! LocoNet use (1 to 2048), so it equals PADA.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RocrailAddresses {
    pub turnout: u16,
    /// MADA: the decoder (Address field) and its port 1 to 4 (Port field).
    pub mada_address: u16,
    pub mada_port: u16,
    /// PADA: Address 0, the turnout number in the Port field.
    pub pada_port: u16,
    /// FADA: Port 0, this number in the Address field (gate 0; gate 1 is one more).
    pub fada_address: u16,
}

/// The Rocrail fields for a turnout number 1 to 2048.
pub fn addresses(turnout: u16) -> RocrailAddresses {
    let index = turnout.saturating_sub(1);
    RocrailAddresses {
        turnout,
        mada_address: index / 4 + 1,
        mada_port: index % 4 + 1,
        pada_port: turnout,
        fada_address: index * 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rocrail's LocoNet driver sends OPC_SW_REQ for (address − 1) × 4 + (port − 1), which
    /// is the turnout number minus 1, from any of the three forms.
    fn loconet_from_mada(address: u16, port: u16) -> u16 {
        (address - 1) * 4 + (port - 1)
    }

    fn mada_from_fada(fada: u16) -> (u16, u16) {
        (fada / 8 + 1, (fada % 8) / 2 + 1)
    }

    #[test]
    fn turnout_5() {
        let a = addresses(5);
        assert_eq!((a.mada_address, a.mada_port), (2, 1));
        assert_eq!(a.pada_port, 5);
        assert_eq!(a.fada_address, 8);
    }

    #[test]
    fn every_form_reaches_the_same_loconet_turnout() {
        for turnout in 1..=2048 {
            let a = addresses(turnout);
            assert_eq!(loconet_from_mada(a.mada_address, a.mada_port) + 1, turnout);
            let (address, port) = mada_from_fada(a.fada_address);
            assert_eq!(loconet_from_mada(address, port) + 1, turnout);
            let (address, port) = mada_from_fada(a.fada_address + 1); // gate 1
            assert_eq!(loconet_from_mada(address, port) + 1, turnout);
        }
    }
}
