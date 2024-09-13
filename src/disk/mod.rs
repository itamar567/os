use core::ops::Deref;

use spin::{Lazy, Mutex};

use self::{ata::Disk, fat16::Fat16};

mod ata;

mod fat16;

pub static FILESYSTEM: Lazy<Mutex<Fat16>> = Lazy::new(|| Mutex::new(Fat16::new(Disk)));
