//! The 64 verbs and the 2-bit phase. One byte: `phase << 6 | code`.

use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    Instant = 0,
    Begin = 1,
    End = 2,
    Sample = 3,
}

impl Phase {
    pub fn mark(self) -> &'static str {
        match self {
            Phase::Instant => "",
            Phase::Begin => "(",
            Phase::End => ")",
            Phase::Sample => "*",
        }
    }
}

macro_rules! verbs {
    ($($code:literal $v:ident),* $(,)?) => {
        /// The 64 universal verbs. Discriminant = 6-bit code (block << 3 | index).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        #[allow(non_camel_case_types)]
        pub enum Verb { $($v = $code),* }

        impl Verb {
            pub const ALL: [Verb; 64] = [$(Verb::$v),*];
            pub fn name(self) -> &'static str {
                match self { $(Verb::$v => stringify!($v)),* }
            }
            pub fn from_name(s: &str) -> Option<Verb> {
                match s { $(stringify!($v) => Some(Verb::$v),)* _ => None }
            }
        }
    };
}

verbs! {
    0x00 POWER_CHANGE, 0x01 MODE_CHANGE, 0x02 GATE, 0x03 RESTORE,
    0x04 CALIBRATE, 0x05 ASSIGN_CLASS, 0x06 SUSPEND, 0x07 RESUME,
    0x08 GRANT, 0x09 REVOKE, 0x0A DELEGATE, 0x0B EXPIRE,
    0x0C CREATE, 0x0D BIND, 0x0E ATTEST, 0x0F VERIFY,
    0x10 OPEN, 0x11 CLOSE, 0x12 JOIN, 0x13 LEAVE,
    0x14 REMOVE, 0x15 COMMIT, 0x16 AMEND, 0x17 DISPUTE,
    0x18 SEND, 0x19 RECEIVE, 0x1A ACK, 0x1B INVOICE,
    0x1C PAY, 0x1D QUOTE, 0x1E AGREE, 0x1F COMPLETE,
    0x20 ASSIGN, 0x21 START, 0x22 PROGRESS, 0x23 FINISH,
    0x24 INSPECT, 0x25 REPORT, 0x26 EXPENSE, 0x27 TRAVEL,
    0x28 OBSERVE, 0x29 NOTE, 0x2A JOURNAL, 0x2B CAPTURE,
    0x2C MEASURE, 0x2D ESTIMATE, 0x2E COMPARE, 0x2F ALERT,
    0x30 SCHEDULE, 0x31 RESCHEDULE, 0x32 CANCEL, 0x33 REMIND,
    0x34 TRIGGER, 0x35 RECUR, 0x36 ATTEND, 0x37 DELEGATE_EVENT,
    0x38 QUERY, 0x39 RESPOND, 0x3A ACT, 0x3B DEFER,
    0x3C LEARN, 0x3D TEACH, 0x3E RECOMMEND, 0x3F AUTOMATE,
}

impl Verb {
    pub fn code(self) -> u8 {
        self as u8
    }
    pub fn from_code(c: u8) -> Option<Verb> {
        Verb::ALL.get(c as usize).copied()
    }
    pub fn block(self) -> u8 {
        self.code() >> 3
    }
    /// The eight verbs whose meaning the kernel itself fixes.
    pub fn is_bootstrap(self) -> bool {
        matches!(
            self,
            Verb::CREATE | Verb::BIND | Verb::GRANT | Verb::REVOKE | Verb::AMEND | Verb::OPEN | Verb::COMMIT | Verb::ACK
        )
    }
}

/// A type: verb plus phase, packed into one byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Type(u8);

impl Type {
    pub const fn new(phase: Phase, verb: Verb) -> Type {
        Type(((phase as u8) << 6) | verb as u8)
    }
    pub fn byte(self) -> u8 {
        self.0
    }
    pub fn from_byte(b: u8) -> Type {
        Type(b)
    }
    pub fn verb(self) -> Verb {
        Verb::ALL[(self.0 & 0x3F) as usize]
    }
    pub fn phase(self) -> Phase {
        match self.0 >> 6 {
            0 => Phase::Instant,
            1 => Phase::Begin,
            2 => Phase::End,
            _ => Phase::Sample,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.verb().name(), self.phase().mark())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_bytes() {
        for (i, v) in Verb::ALL.iter().enumerate() {
            assert_eq!(v.code() as usize, i);
            assert_eq!(Verb::from_name(v.name()), Some(*v));
        }
        assert_eq!(Type::new(Phase::Begin, Verb::ATTEND).byte(), 0x76);
        assert_eq!(Type::new(Phase::End, Verb::ATTEND).byte(), 0xB6);
        assert_eq!(Type::new(Phase::End, Verb::QUERY).byte(), 0xB8);
        assert_eq!(Type::new(Phase::Sample, Verb::MEASURE).byte(), 0xEC);
        assert_eq!(Type::new(Phase::Instant, Verb::FINISH).byte(), 0x23);
        assert_eq!(Verb::ATTEND.block(), 6);
        let t = Type::from_byte(0xBA);
        assert_eq!((t.verb(), t.phase()), (Verb::ACT, Phase::End));
        assert_eq!(Verb::ALL.iter().filter(|v| v.is_bootstrap()).count(), 8);
    }
}
